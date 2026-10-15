//! HTTP and HTTPS through the `ureq` client, with `rustls` for TLS.
//!
//! Each call builds its own agent: the user agent, the timeout for the whole transfer,
//! the proxy from the environment (see [`crate::proxy`]) and how many redirects to follow
//! (none for [`crate::send`]). ureq speaks only http and https, so redirects cannot lead
//! anywhere else. Bodies are read in chunks and bounded while they arrive; the cancel flag
//! is checked between chunks, and while waiting for an answer the transfer runs on a
//! worker thread so that a cancel does not wait for the server.

use std::borrow::Cow;
use std::fs::File;
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::{Body, Request, Response, proxy};
use crate::{Error, ErrorKind, Fetched, Header, MAX_HEADER_BYTES, MAX_HEADERS, Options};

/// The most redirects [`get_url`] follows before it gives up.
const MAX_REDIRECTS: u32 = 20;
/// How often a waiting call looks at the cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(10);
/// The size of one read of a response body.
const CHUNK: usize = 64 * 1024;

pub(crate) fn get_url(url: &str, options: &Options) -> Result<Fetched, Error> {
    match &options.cancel {
        None => get_now(url, options),
        Some(cancel) => {
            let (owned_url, owned) = (url.to_owned(), options.clone());
            cancellable(cancel, url, move || get_now(&owned_url, &owned))?
        }
    }
}

pub(crate) fn send_url(request: &Request, options: &Options) -> Response {
    match &options.cancel {
        None => send_now(request, options),
        Some(cancel) => {
            let (owned_request, owned) = (request.clone(), options.clone());
            cancellable(cancel, &request.url, move || send_now(&owned_request, &owned))
                .unwrap_or_else(|e| Response { error: Some(e), ..Response::default() })
        }
    }
}

/// Runs `work` on a worker thread and waits for it, giving up with
/// [`ErrorKind::Cancelled`] once `cancel` becomes true. A worker left behind ends when its
/// transfer does (it reads the same flag between chunks, and the timeout bounds the rest).
fn cancellable<T, F>(cancel: &Arc<AtomicBool>, location: &str, work: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    if cancel.load(Ordering::Acquire) {
        return Err(cancelled(location));
    }
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after a cancel; then the result is not wanted.
        let _ = sender.send(work());
    });
    loop {
        match receiver.recv_timeout(CANCEL_POLL) {
            Ok(result) => return Ok(result),
            Err(RecvTimeoutError::Timeout) if cancel.load(Ordering::Acquire) => {
                return Err(cancelled(location));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let detail = "The transfer stopped without an answer.";
                return Err(Error::new(ErrorKind::Network, location, detail));
            }
        }
    }
}

fn cancelled(location: &str) -> Error {
    Error::new(ErrorKind::Cancelled, location, "The transfer was cancelled.")
}

fn get_now(url: &str, options: &Options) -> Result<Fetched, Error> {
    let agent = agent_for(url, options, MAX_REDIRECTS)?;
    // Ask for the body as it is: a server's malformed compression cannot stop the read.
    let request = agent.get(url).set("Accept-Encoding", "identity");
    let response = answer_of(request.call(), url)?;
    let status = response.status();
    let final_url = response.get_url().to_owned();
    if !(200..=299).contains(&status) {
        let detail = format!("The server answered with HTTP status {status}.");
        return Err(Error { status: Some(status), ..Error::new(ErrorKind::Status, url, detail) });
    }
    let (data, failed) = read_body(response.into_reader(), url, options);
    match failed {
        Some(e) => Err(Error { status: Some(status), ..e }),
        None => Ok(Fetched { data, url: final_url, status: Some(status) }),
    }
}

fn send_now(r: &Request, options: &Options) -> Response {
    let agent = match agent_for(&r.url, options, 0) {
        Ok(agent) => agent,
        Err(e) => return Response { error: Some(e), ..Response::default() },
    };
    let mut request = agent.request(r.method.name(), &r.url);
    for h in &r.headers {
        request = request.set(&h.name, &h.value);
    }
    if !r.headers.iter().any(|h| h.name.eq_ignore_ascii_case("accept-encoding")) {
        request = request.set("Accept-Encoding", "identity");
    }
    let response = match dispatch(request, r) {
        Ok(response) => response,
        Err(failed) => return Response { error: Some(failed), ..Response::default() },
    };
    response_of(response, &r.url, options)
}

/// Sends `request` with the body `r` names and returns the answer, whatever its status.
/// Fails when a body file cannot be opened or no answer arrives.
fn dispatch(request: ureq::Request, r: &Request) -> Result<ureq::Response, Error> {
    let answer = match &r.body {
        Body::File(path) => {
            let missing = |e: io::Error| {
                Error::new(ErrorKind::Missing, &path.display().to_string(), e.to_string())
            };
            let file = File::open(path).map_err(missing)?;
            let length = file.metadata().map_err(missing)?.len();
            // A known length is sent as Content-Length rather than in chunks.
            request.set("Content-Length", &length.to_string()).send(file.take(length))
        }
        Body::Bytes(bytes) => request.send_bytes(bytes),
        Body::None if r.method.is_bodyless() => request.call(),
        Body::None => request.send_bytes(&[]),
    };
    answer_of(answer, &r.url)
}

/// Turns a finished exchange into a [`Response`]: headers, the bounded body, and an error
/// for a status other than 2xx.
fn response_of(response: ureq::Response, url: &str, options: &Options) -> Response {
    let status = response.status();
    let mut out = Response { status: Some(status), ..Response::default() };
    let Some(headers) = headers_of(&response) else {
        let detail = "The response's headers are larger than the limit.";
        out.error =
            Some(Error { status: Some(status), ..Error::new(ErrorKind::TooLarge, url, detail) });
        return out;
    };
    out.headers = headers;
    let (body, failed) = read_body(response.into_reader(), url, options);
    out.body = body;
    let failed = failed.or_else(|| match status {
        200..=299 => None,
        300..=399 => {
            Some(Error::new(ErrorKind::Redirect, url, "The server answered with a redirect."))
        }
        s => {
            let detail = format!("The server answered with HTTP status {s}.");
            Some(Error::new(ErrorKind::Status, url, detail))
        }
    });
    out.error = failed.map(|e| Error { status: Some(status), ..e });
    out
}

/// The agent for one call to `url`: user agent, timeout, proxy and redirect count.
fn agent_for(url: &str, options: &Options, redirects: u32) -> Result<ureq::Agent, Error> {
    let mut builder = ureq::AgentBuilder::new()
        .try_proxy_from_env(false)
        .redirects(redirects)
        .user_agent(&options.user_agent);
    if let Some(timeout) = options.timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(found) = proxy::proxy_for(url, |name| std::env::var(name).ok()) {
        let proxy = ureq::Proxy::new(found.value.trim_end_matches('/')).map_err(|e| {
            let detail =
                format!("The proxy `{}` in {} cannot be used: {e}.", found.value, found.variable);
            Error::new(ErrorKind::Network, url, detail)
        })?;
        builder = builder.proxy(proxy);
    }
    Ok(builder.build())
}

/// ureq reports a status of 400 or more as an error; here any answer is a response, and
/// only a request that got no answer fails.
fn answer_of(
    answer: Result<ureq::Response, ureq::Error>,
    url: &str,
) -> Result<ureq::Response, Error> {
    match answer {
        Ok(response) | Err(ureq::Error::Status(_, response)) => Ok(response),
        Err(ureq::Error::Transport(transport)) => Err(transport_error(&transport, url)),
    }
}

/// The response's headers, names in lowercase, in the order received; `None` when they
/// are more than [`MAX_HEADERS`] or larger than [`MAX_HEADER_BYTES`].
fn headers_of(response: &ureq::Response) -> Option<Vec<Header>> {
    let names = response.headers_names();
    if names.len() > MAX_HEADERS {
        return None;
    }
    let mut headers: Vec<Header> = Vec::with_capacity(names.len());
    let mut bytes = 0;
    for name in names {
        // A repeated name: take its next value.
        let seen = headers.iter().filter(|h| h.name == name).count();
        let Some(value) = response.all(&name).get(seen).map(|v| v.trim().to_owned()) else {
            continue;
        };
        bytes += name.len() + value.len() + 4;
        headers.push(Header { name, value });
    }
    (bytes <= MAX_HEADER_BYTES).then_some(headers)
}

/// Reads `body` to its end in chunks, stopping with an error past [`Options::max_bytes`],
/// on a cancel, a timeout or a failed read. The bytes read so far are kept (none when
/// memory ran out).
fn read_body(mut body: impl Read, url: &str, options: &Options) -> (Vec<u8>, Option<Error>) {
    let max = options.max_bytes;
    let mut data = Vec::new();
    let mut chunk = vec![0u8; CHUNK];
    loop {
        if options.cancel.as_ref().is_some_and(|c| c.load(Ordering::Acquire)) {
            return (data, Some(cancelled(url)));
        }
        let n = match body.read(&mut chunk) {
            Ok(0) => return (data, None),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return (data, Some(io_error(&e, url))),
        };
        if n > max.saturating_sub(data.len()) {
            let detail = format!("The response is larger than the limit of {max} bytes.");
            return (data, Some(Error::new(ErrorKind::TooLarge, url, detail)));
        }
        if data.try_reserve(n).is_err() {
            let detail = "Memory ran out while reading the response.";
            return (Vec::new(), Some(Error::new(ErrorKind::Memory, url, detail)));
        }
        data.extend_from_slice(&chunk[..n]);
    }
}

fn io_error(e: &io::Error, url: &str) -> Error {
    if is_timeout(e) {
        return Error::new(ErrorKind::Timeout, url, "The transfer took longer than the timeout.");
    }
    Error::new(ErrorKind::Network, url, format!("Reading the response failed: {e}."))
}

fn is_timeout(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock)
}

/// The [`Error`] for a request that got no answer.
fn transport_error(transport: &ureq::Transport, url: &str) -> Error {
    let mut source = std::error::Error::source(transport);
    while let Some(cause) = source {
        if cause.downcast_ref::<io::Error>().is_some_and(is_timeout) {
            let detail = "The transfer took longer than the timeout.";
            return Error::new(ErrorKind::Timeout, url, detail);
        }
        source = cause.source();
    }
    let message = transport.message().unwrap_or("");
    if transport.kind() == ureq::ErrorKind::BadHeader && message.contains("header fields") {
        let detail = "The response's headers are larger than the limit.";
        return Error::new(ErrorKind::TooLarge, url, detail);
    }
    Error::new(ErrorKind::Network, url, describe(transport))
}

/// A sentence from ureq's kind, message and cause, such as
/// "Connection Failed: Connect error: Connection refused (os error 111)."
fn describe(transport: &ureq::Transport) -> Cow<'static, str> {
    let mut text = transport.kind().to_string();
    if let Some(message) = transport.message() {
        text.push_str(": ");
        text.push_str(message);
    }
    if let Some(cause) = std::error::Error::source(transport) {
        text.push_str(": ");
        text.push_str(&cause.to_string());
    }
    text.push('.');
    text.into()
}
