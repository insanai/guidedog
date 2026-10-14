//! HTTP and HTTPS through the `curl` executable.
//!
//! curl is run directly with an argument vector (never through a shell), with `-q` first
//! so no `.curlrc` changes its behaviour. Only http and https are allowed, also for
//! redirects. The body arrives on stdout; the status and final URL arrive on stderr after
//! a marker, written by `--write-out`. Bodies are bounded while they are read: past the
//! limit the pipe is closed and curl stops.

use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::{Error, ErrorKind, Fetched, Header, MAX_HEADER_BYTES, MAX_HEADERS, Options};
use crate::{Request, Response};

const MARKER: &str = "@@gd-fetch@@ ";
const WRITE_OUT: &str = "%{stderr}\n@@gd-fetch@@ %{http_code} %{url_effective}\n";
const MAX_STDERR: u64 = 1024 * 1024;
const CURLE_OPERATION_TIMEDOUT: i32 = 28;

/// What a request body is read from.
enum Input<'a> {
    None,
    Bytes(&'a [u8]),
    File(File),
}

/// What one run of curl produced.
#[derive(Default)]
struct Run {
    stdout: Vec<u8>,
    overflow: bool,
    memory: bool,
    cancelled: bool,
    exit: Option<i32>,
    status: u16,
    url: String,
    message: String,
}

pub(crate) fn get_url(url: &str, options: &Options) -> Result<Fetched, Error> {
    let mut args = base_args(options);
    args.push("--location".into());
    push_url(&mut args, url);
    let run = run(options, &args, Input::None, options.limit())?;
    if let Some(mut e) = failure(&run, url, options.limit()) {
        e.status = run.status;
        return Err(e);
    }
    if !(200..=299).contains(&run.status) {
        let detail = format!("The server answered with HTTP status {}.", run.status);
        let mut e = Error::new(ErrorKind::Status, url, detail);
        e.status = run.status;
        return Err(e);
    }
    let final_url = if run.url.is_empty() { url.to_owned() } else { run.url };
    Ok(Fetched { data: run.stdout, url: final_url, status: run.status })
}

pub(crate) fn send_url(r: &Request, options: &Options) -> Response {
    let input = match (&r.body_path, r.body.is_empty()) {
        (Some(path), _) => match File::open(path) {
            Ok(f) => Input::File(f),
            Err(e) => {
                let shown = path.display().to_string();
                return Response::failure(ErrorKind::Missing, &shown, e.to_string());
            }
        },
        (None, false) => Input::Bytes(&r.body),
        (None, true) => Input::None,
    };
    let method = if r.method.is_empty() { "GET" } else { r.method.as_str() };
    let mut args = base_args(options);
    args.extend(["--dump-header".into(), "-".into(), "--header".into(), "Expect:".into()]);
    for h in &r.headers {
        // "Name:" would remove the header in curl; "Name;" sends it empty.
        let line = if h.value.is_empty() {
            format!("{};", h.name)
        } else {
            format!("{}: {}", h.name, h.value)
        };
        args.extend(["--header".into(), line.into()]);
    }
    let input = if method == "GET" {
        input
    } else {
        args.extend(["--data-binary".into(), "@-".into()]);
        if method != "POST" {
            args.extend(["--request".into(), method.into()]);
        }
        match input {
            Input::None => Input::Bytes(&[]),
            other => other,
        }
    };
    push_url(&mut args, &r.url);
    let room = options.limit().saturating_add(2 * MAX_HEADER_BYTES);
    match run(options, &args, input, room) {
        Ok(run) => response_of(run, &r.url, options.limit()),
        Err(e) => Response { error: Some(e), ..Response::default() },
    }
}

fn base_args(options: &Options) -> Vec<OsString> {
    const FIXED: [&str; 11] = [
        "-q",
        "--silent",
        "--show-error",
        "--globoff",
        "--proto",
        "=http,https",
        "--proto-redir",
        "=http,https",
        "--write-out",
        WRITE_OUT,
        "--user-agent",
    ];
    // The fixed flags, the agent, a timeout, and room for a request's headers and URL.
    let mut args = Vec::with_capacity(FIXED.len() + 16);
    args.extend(FIXED.iter().map(OsString::from));
    args.push(options.agent().into());
    if let Some(t) = options.timeout {
        let ms = t.as_millis().max(1);
        args.extend(["--max-time".into(), format!("{}.{:03}", ms / 1000, ms % 1000).into()]);
    }
    args
}

fn push_url(args: &mut Vec<OsString>, url: &str) {
    args.extend(["--url".into(), url.into()]);
}

/// The failure a finished run shows, apart from its HTTP status.
fn failure(run: &Run, location: &str, max: usize) -> Option<Error> {
    let (kind, detail): (ErrorKind, Cow<'static, str>) = if run.memory {
        (ErrorKind::Memory, "Memory ran out while reading the response.".into())
    } else if run.overflow {
        (
            ErrorKind::TooLarge,
            format!("The response is larger than the limit of {max} bytes.").into(),
        )
    } else if run.cancelled {
        (ErrorKind::Cancelled, "The transfer was cancelled.".into())
    } else if run.exit == Some(CURLE_OPERATION_TIMEDOUT) {
        (ErrorKind::Timeout, "The transfer took longer than the timeout.".into())
    } else if run.exit != Some(0) {
        let detail = match (run.exit, run.message.is_empty()) {
            (_, false) => run.message.clone().into(),
            (Some(code), true) => format!("curl failed with exit code {code}.").into(),
            (None, true) => "curl was stopped by a signal.".into(),
        };
        (ErrorKind::Network, detail)
    } else {
        return None;
    };
    Some(Error::new(kind, location, detail))
}

fn response_of(run: Run, url: &str, max: usize) -> Response {
    let mut response = Response { status: run.status, ..Response::default() };
    let failed = failure(&run, url, max);
    let (headers, start) = match split_head(&run.stdout) {
        Ok(found) => found,
        Err(()) => {
            let detail = "The response's headers are larger than the limit.";
            let mut e = Error::new(ErrorKind::TooLarge, url, detail);
            e.status = run.status;
            response.error = Some(failed.unwrap_or(e));
            return response;
        }
    };
    response.headers = headers;
    let mut stdout = run.stdout;
    response.body = stdout.split_off(start);
    let (kind, detail): (ErrorKind, Cow<'static, str>) = match run.status {
        _ if failed.is_some() => (ErrorKind::Network, Cow::Borrowed("")),
        _ if response.body.len() > max => (
            ErrorKind::TooLarge,
            format!("The response is larger than the limit of {max} bytes.").into(),
        ),
        200..=299 => return response,
        300..=399 => (ErrorKind::Redirect, "The server answered with a redirect.".into()),
        s => (ErrorKind::Status, format!("The server answered with HTTP status {s}.").into()),
    };
    let mut e = failed.unwrap_or_else(|| Error::new(kind, url, detail));
    e.status = run.status;
    response.error = Some(e);
    response
}

/// Splits curl's `--dump-header -` output: the headers of the final response (interim 1xx
/// responses are skipped), and where the body starts. `Err` when the headers are too many.
fn split_head(out: &[u8]) -> Result<(Vec<Header>, usize), ()> {
    let mut pos = 0;
    loop {
        if !out[pos..].starts_with(b"HTTP/") {
            return Ok((Vec::new(), pos));
        }
        let Some((end, next)) = block_end(&out[pos..]) else {
            return Ok((Vec::new(), pos));
        };
        let block = String::from_utf8_lossy(&out[pos..pos + end]);
        let mut lines = block.lines();
        let status = lines.next().and_then(|l| l.split(' ').nth(1)).unwrap_or("");
        if status.starts_with('1') {
            pos += next;
            continue;
        }
        if end > MAX_HEADER_BYTES {
            return Err(());
        }
        let headers: Vec<Header> = lines.filter_map(header_of).collect();
        if headers.len() > MAX_HEADERS {
            return Err(());
        }
        return Ok((headers, pos + next));
    }
}

/// The length of the header block at the start of `s` and where what follows it starts.
fn block_end(s: &[u8]) -> Option<(usize, usize)> {
    let crlf = s.windows(4).position(|w| w == b"\r\n\r\n").map(|i| (i, i + 4));
    let lf = s.windows(2).position(|w| w == b"\n\n").map(|i| (i, i + 2));
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (a, b) => a.or(b),
    }
}

fn header_of(line: &str) -> Option<Header> {
    let (name, value) = line.split_once(':')?;
    Some(Header { name: name.trim().to_ascii_lowercase(), value: value.trim().to_owned() })
}

/// Runs curl with `args`, feeding `input` to its stdin and reading at most `limit` bytes
/// of its stdout.
fn run(options: &Options, args: &[OsString], input: Input, limit: usize) -> Result<Run, Error> {
    let program = options.curl.as_deref().unwrap_or(Path::new("curl"));
    let stdin = if matches!(input, Input::None) { Stdio::null() } else { Stdio::piped() };
    let spawned = Command::new(program)
        .args(args)
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = spawned.map_err(|e| {
        let detail = format!("`{}` could not be run: {e}.", program.display());
        Error::new(ErrorKind::CurlMissing, &program.display().to_string(), detail)
    })?;
    // Scoped threads borrow the request body instead of copying it.
    let mut run = thread::scope(|scope| {
        if let Some(stdin) = child.stdin.take() {
            scope.spawn(move || feed(stdin, input));
        }
        let errors = child.stderr.take().map(|mut e| {
            scope.spawn(move || {
                let mut text = Vec::new();
                let _ = (&mut e).take(MAX_STDERR).read_to_end(&mut text);
                text
            })
        });
        let reader = child.stdout.take().map(|o| scope.spawn(move || drain(o, limit)));
        let (exit, cancelled) = wait(&mut child, options.cancel.as_ref());
        let mut run = Run { exit, cancelled, ..Run::default() };
        if let Some(Ok((data, overflow, memory))) = reader.map(|r| r.join()) {
            (run.stdout, run.overflow, run.memory) = (data, overflow, memory);
        }
        let text = errors.and_then(|e| e.join().ok()).unwrap_or_default();
        (run, text)
    });
    let (ref mut out, ref text) = run;
    read_stderr(out, &String::from_utf8_lossy(text));
    Ok(run.0)
}

fn feed(mut stdin: std::process::ChildStdin, input: Input) {
    // A write fails when curl stops early; the run's exit code reports why.
    let _ = match input {
        Input::None => Ok(()),
        Input::Bytes(b) => stdin.write_all(b),
        Input::File(mut f) => io::copy(&mut f, &mut stdin).map(|_| ()),
    };
}

/// Reads `out` to its end, or until it would pass `limit` bytes (then it closes the pipe,
/// which stops curl). Returns the bytes, whether they overflowed, and whether memory ran out.
fn drain(mut out: impl Read, limit: usize) -> (Vec<u8>, bool, bool) {
    let mut data = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = match out.read(&mut chunk) {
            Ok(0) => return (data, false, false),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return (data, false, false),
        };
        if n > limit - data.len() {
            return (data, true, false);
        }
        if data.try_reserve(n).is_err() {
            return (Vec::new(), false, true);
        }
        data.extend_from_slice(&chunk[..n]);
    }
}

/// Waits for curl, killing it when `cancel` becomes true. Returns its exit code (`None`
/// when a signal stopped it) and whether it was cancelled.
fn wait(child: &mut Child, cancel: Option<&Arc<AtomicBool>>) -> (Option<i32>, bool) {
    let Some(cancel) = cancel else {
        return (child.wait().ok().and_then(|s| s.code()), false);
    };
    loop {
        if cancel.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return (None, true);
        }
        match child.try_wait() {
            Ok(Some(status)) => return (status.code(), false),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => return (None, false),
        }
    }
}

/// Takes the status and final URL from the marker line, and curl's message from the rest.
fn read_stderr(run: &mut Run, text: &str) {
    run.message.clear();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(MARKER) {
            let (code, url) = rest.split_once(' ').unwrap_or((rest, ""));
            run.status = code.parse().unwrap_or(0);
            run.url.clear();
            run.url.push_str(url);
        } else if !line.trim().is_empty() {
            if !run.message.is_empty() {
                run.message.push(' ');
            }
            run.message.push_str(clean(line.trim()));
        }
    }
}

/// curl's message without its `curl: (N) ` prefix.
fn clean(line: &str) -> &str {
    let rest = line.strip_prefix("curl: ").unwrap_or(line);
    match rest.strip_prefix('(').and_then(|r| r.split_once(") ")) {
        Some((code, text)) if code.bytes().all(|c| c.is_ascii_digit()) => text,
        _ => rest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interim_responses_are_skipped() {
        let out = b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 201 Created\r\nX-A: b\r\n\r\nbody";
        let (headers, start) = split_head(out).unwrap_or_default();
        assert_eq!(headers, vec![Header::new("x-a", "b")]);
        assert_eq!(&out[start..], b"body");
    }

    #[test]
    fn curl_messages_lose_their_prefix() {
        assert_eq!(clean("curl: (6) Could not resolve host: x"), "Could not resolve host: x");
        assert_eq!(clean("something else"), "something else");
    }
}
