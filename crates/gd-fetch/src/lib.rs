//! Fetch reads remote and local resources and sends HTTP requests.
//!
//! HTTP and HTTPS go through the `curl` executable (run directly, never through a shell);
//! `file://` URLs and plain paths are read from the disk. Guidedog uses [`get`] for
//! remote intersphinx inventories, and the Pages publisher uses [`send`] for GitHub's API
//! and Git's smart HTTP.
//!
//! ```no_run
//! let options = gd_fetch::Options::default();
//! match gd_fetch::get("https://docs.python.org/3/objects.inv", &options) {
//!     Ok(f) => println!("{} bytes from {}", f.data.len(), f.url),
//!     Err(e) => eprintln!("{e}"),
//! }
//! ```
//!
//! [`get`] follows redirects, only to http and https, and limits the body to
//! [`Options::max_bytes`]. Bodies are asked for as they are, without transfer
//! compression, so a server's malformed `Content-Encoding` does not stop the transfer.
//! [`send`] makes one request and never follows a redirect, so headers such as
//! `Authorization` reach only the URL the caller named.

mod curl;
mod error;
pub mod testserver;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub use error::{Error, ErrorKind};

/// The default for [`Options::timeout`]: 30 seconds for the whole transfer.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The default for [`Options::max_bytes`]: 64 MiB.
pub const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;
/// The most headers a response may have.
pub const MAX_HEADERS: usize = 128;
/// The most bytes a response's headers may take in all.
pub const MAX_HEADER_BYTES: usize = 64 * 1024;

/// The default for [`Options::user_agent`].
pub const DEFAULT_USER_AGENT: &str = "Guidedog";

/// Options bound one [`get`] or [`send`]. `Options::default()` is the defaults; change
/// fields with struct-update syntax.
#[derive(Debug, Clone)]
pub struct Options {
    /// The whole transfer's limit; `None` is no limit.
    pub timeout: Option<Duration>,
    /// The most body bytes read ([`DEFAULT_MAX_BYTES`] by default).
    pub max_bytes: usize,
    /// The `User-Agent` ([`DEFAULT_USER_AGENT`] by default).
    pub user_agent: Cow<'static, str>,
    /// When set, the transfer stops (with [`ErrorKind::Cancelled`]) once it becomes true.
    pub cancel: Option<Arc<AtomicBool>>,
    /// The curl executable to run; `None` looks `curl` up on `PATH`.
    pub curl: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            timeout: Some(DEFAULT_TIMEOUT),
            max_bytes: DEFAULT_MAX_BYTES,
            user_agent: Cow::Borrowed(DEFAULT_USER_AGENT),
            cancel: None,
            curl: None,
        }
    }
}

/// What [`get`] read. The caller owns all of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fetched {
    /// The body or the file's contents.
    pub data: Vec<u8>,
    /// Where the data came from, after redirects (the path for a file).
    pub url: String,
    /// The HTTP status; `None` for files.
    pub status: Option<u16>,
}

/// One HTTP header. A response's names are in lowercase.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    /// The header's name.
    pub name: String,
    /// The header's value, trimmed.
    pub value: String,
}

impl Header {
    /// Makes a header from borrowed text; it copies both.
    pub fn new(name: &str, value: &str) -> Header {
        Header { name: name.to_owned(), value: value.to_owned() }
    }
}

/// The HTTP methods [`send`] makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Method {
    /// `GET`, which has no body.
    #[default]
    Get,
    /// `HEAD`, which has no body.
    Head,
    /// `POST`.
    Post,
    /// `PUT`.
    Put,
    /// `PATCH`.
    Patch,
    /// `DELETE`.
    Delete,
}

impl Method {
    /// The method's name as HTTP spells it, such as `"POST"`.
    pub fn name(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }

    /// Whether requests with this method never carry a body.
    fn is_bodyless(self) -> bool {
        matches!(self, Method::Get | Method::Head)
    }
}

/// A request's body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Body {
    /// No body.
    #[default]
    None,
    /// Bytes in memory.
    Bytes(Vec<u8>),
    /// The contents of a file, read as they are sent.
    File(PathBuf),
}

/// One HTTP request for [`send`].
#[derive(Debug, Clone, Default)]
pub struct Request {
    /// The method.
    pub method: Method,
    /// An http or https URL.
    pub url: String,
    /// Headers to send as they are.
    pub headers: Vec<Header>,
    /// The body; GET and HEAD requests have none.
    pub body: Body,
}

/// What [`send`] received, or why it could not.
///
/// A response whose status is not 2xx has an error of kind [`ErrorKind::Status`] and keeps
/// its body and headers, which often explain it; a redirect has [`ErrorKind::Redirect`]
/// and its `location` header. The caller owns all of it.
#[derive(Debug, Clone, Default)]
pub struct Response {
    /// The HTTP status; `None` when none was received.
    pub status: Option<u16>,
    /// The body as received.
    pub body: Vec<u8>,
    /// The final response's headers, names in lowercase.
    pub headers: Vec<Header>,
    /// Why the request failed, or `None` for a 2xx answer.
    pub error: Option<Error>,
}

impl Response {
    /// The value of the header `name` (in lowercase), or `None` when the response has none.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|h| h.name == name).map(|h| h.value.as_str())
    }

    /// Whether the request succeeded with a 2xx status.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    fn failure(kind: ErrorKind, location: &str, detail: impl Into<Cow<'static, str>>) -> Response {
        Response { error: Some(Error::new(kind, location, detail)), ..Response::default() }
    }
}

/// Reads a URL (`http://`, `https://`, `file://`) or a path and returns what it read.
///
/// It blocks until the transfer ends; threads may call it at once. It fails with an
/// [`Error`], never a panic: a file that is missing, a transfer that fails, times out,
/// answers with a status other than 2xx, or grows beyond [`Options::max_bytes`].
pub fn get(location: &str, options: &Options) -> Result<Fetched, Error> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return curl::get_url(location, options);
    }
    let path = location.strip_prefix("file://").unwrap_or(location);
    if path.contains("://") {
        return Err(Error::new(
            ErrorKind::Unsupported,
            location,
            "Only http://, https:// and file:// URLs, and paths, can be read.",
        ));
    }
    get_file(path, options.max_bytes)
}

/// Makes the HTTP request `request` and returns the response.
///
/// It never follows a redirect and never sends a URL or header that holds a line break,
/// nor a body with GET or HEAD ([`ErrorKind::Unsupported`]). Failure is in
/// [`Response::error`], never a panic. It blocks until the transfer ends; threads may
/// call it at once.
pub fn send(request: &Request, options: &Options) -> Response {
    if let Err(detail) = check_request(request) {
        return Response::failure(ErrorKind::Unsupported, &request.url, detail);
    }
    curl::send_url(request, options)
}

fn check_request(r: &Request) -> Result<(), &'static str> {
    if !r.url.starts_with("http://") && !r.url.starts_with("https://") {
        return Err("Only http and https URLs can be sent to.");
    }
    if r.method.is_bodyless() && r.body != Body::None {
        return Err("A GET or HEAD request has no body.");
    }
    if !is_one_line(&r.url) {
        return Err("The URL is not one line.");
    }
    if r.headers.iter().any(|h| !is_token(&h.name) || !is_one_line(&h.value)) {
        return Err("A header name or value is not one line.");
    }
    Ok(())
}

/// Whether `s` is an HTTP token, such as a method or a header name.
fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.!#$%&'*+^`|~".contains(&c))
}

fn is_one_line(s: &str) -> bool {
    !s.bytes().any(|c| c == b'\r' || c == b'\n' || c == 0)
}

fn get_file(path: &str, max: usize) -> Result<Fetched, Error> {
    let missing = |e: std::io::Error| Error::new(ErrorKind::Missing, path, e.to_string());
    let file = File::open(path).map_err(missing)?;
    let size = file.metadata().map_or(0, |m| usize::try_from(m.len()).unwrap_or(usize::MAX));
    let mut data = Vec::new();
    if size <= max && data.try_reserve_exact(size).is_err() {
        return Err(Error::new(ErrorKind::Memory, path, "Memory ran out reading the file."));
    }
    // A file can grow after it is opened: bound the bytes actually read.
    let mut reader = file.take(max as u64 + 1);
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(missing(e)),
        };
        if n > max.saturating_sub(data.len()) {
            let detail = format!("The file is larger than the limit of {max} bytes.");
            return Err(Error::new(ErrorKind::TooLarge, path, detail));
        }
        if data.try_reserve(n).is_err() {
            return Err(Error::new(ErrorKind::Memory, path, "Memory ran out reading the file."));
        }
        data.extend_from_slice(&chunk[..n]);
    }
    Ok(Fetched { data, url: path.to_owned(), status: None })
}
