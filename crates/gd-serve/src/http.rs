//! The small piece of HTTP/1.1 the preview needs: read a bounded request head, parse
//! its request line, map the target to a file confined to the site root, and write a
//! `Connection: close` response.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

/// The two methods the preview answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    Get,
    Head,
}

/// A parsed request line: the method, the raw path (query and fragment removed) and
/// the percent-decoded path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub method: Method,
    pub raw_path: String,
    pub path: String,
}

/// Why a request cannot be answered with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The request line is not `METHOD /target [HTTP/x.y]`.
    Malformed,
    /// The head did not end within the request size limit.
    TooLarge,
    /// A method other than GET or HEAD.
    Method,
    /// A target with `..`, a backslash, a NUL byte or invalid UTF-8.
    BadPath,
}

impl Refusal {
    /// The status line and plain-text body that explain the refusal.
    pub fn status_and_body(self) -> (&'static str, &'static str) {
        match self {
            Refusal::Malformed => ("400 Bad Request", "Malformed request.\n"),
            Refusal::TooLarge => {
                ("431 Request Header Fields Too Large", "The request is too large.\n")
            }
            Refusal::Method => ("405 Method Not Allowed", "Only GET and HEAD are supported.\n"),
            Refusal::BadPath => ("400 Bad Request", "Invalid path.\n"),
        }
    }
}

/// Reads the request head into `buf` (cleared first) until the blank line that ends
/// it, end of stream, or `limit` bytes. Fails with `Ok(Err(TooLarge))` past the limit
/// and with an I/O error (timeouts included) when the client does not send.
pub(crate) fn read_head(
    stream: &mut impl Read,
    limit: usize,
    buf: &mut Vec<u8>,
) -> io::Result<Result<(), Refusal>> {
    buf.clear();
    let mut chunk = [0u8; 2048];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(if buf.contains(&b'\n') { Ok(()) } else { Err(Refusal::Malformed) });
        }
        buf.extend_from_slice(&chunk[..n]);
        if head_ended(buf) {
            return Ok(Ok(()));
        }
        if buf.len() >= limit {
            return Ok(Err(Refusal::TooLarge));
        }
    }
}

fn head_ended(buf: &[u8]) -> bool {
    buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.windows(2).any(|w| w == b"\n\n")
}

/// Parses the first line of `head`. The target must be origin-form (`/path?query`).
pub(crate) fn parse_request(head: &[u8]) -> Result<Request, Refusal> {
    let end = head.iter().position(|&b| b == b'\n').unwrap_or(head.len());
    let line = std::str::from_utf8(&head[..end]).map_err(|_| Refusal::Malformed)?;
    let mut fields = line.split_ascii_whitespace();
    let (Some(method), Some(target)) = (fields.next(), fields.next()) else {
        return Err(Refusal::Malformed);
    };
    let method = match method {
        "GET" => Method::Get,
        "HEAD" => Method::Head,
        _ => return Err(Refusal::Method),
    };
    let raw_path = target.split(['?', '#']).next().unwrap_or_default();
    if !raw_path.starts_with('/') {
        return Err(Refusal::Malformed);
    }
    let decoded = percent_decode(raw_path);
    let path = String::from_utf8(decoded).map_err(|_| Refusal::BadPath)?;
    Ok(Request { method, raw_path: raw_path.to_string(), path })
}

/// Decodes `%XX` escapes; a `%` not followed by two hex digits stays as it is.
pub(crate) fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let (Some(hi), Some(lo)) =
                (hex_value(bytes.get(i + 1)), hex_value(bytes.get(i + 2)))
        {
            out.push(hi * 16 + lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn hex_value(byte: Option<&u8>) -> Option<u8> {
    let c = *byte?;
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// What a decoded path names inside the site.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Target {
    /// A regular file inside the site root, as its canonical path.
    File(PathBuf),
    /// A directory requested without its trailing slash: redirect to `raw_path/`.
    Directory,
    /// Nothing, or something outside the site root (through a symbolic link).
    Missing,
}

/// Maps the decoded `path` (with leading `/`) to the file it names below `root`, a
/// canonical directory. `..` segments, backslashes and NUL bytes are refused before
/// the file system is touched; symbolic links that lead outside `root` are `Missing`.
pub(crate) fn resolve(root: &Path, path: &str) -> Result<Target, Refusal> {
    let relative = relative_path(path)?;
    let Ok(canonical) = root.join(&relative).canonicalize() else {
        return Ok(Target::Missing);
    };
    if !canonical.starts_with(root) {
        return Ok(Target::Missing);
    }
    match canonical.metadata() {
        Ok(m) if m.is_file() => Ok(Target::File(canonical)),
        Ok(m) if m.is_dir() => Ok(Target::Directory),
        _ => Ok(Target::Missing),
    }
}

/// The site-relative file path of a decoded request path: only plain segments, with
/// `index.html` added for a directory path (empty or ending in `/`).
pub(crate) fn relative_path(path: &str) -> Result<PathBuf, Refusal> {
    if path.contains(['\\', '\0']) {
        return Err(Refusal::BadPath);
    }
    let mut relative = PathBuf::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => return Err(Refusal::BadPath),
            _ => relative.push(segment),
        }
    }
    // Only plain names may remain: no root, prefix or parent component.
    if !relative.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(Refusal::BadPath);
    }
    if path.ends_with('/') || relative.as_os_str().is_empty() {
        relative.push("index.html");
    }
    Ok(relative)
}

/// The `Content-Type` for a file, by its extension (ASCII case ignored).
pub(crate) fn content_type(path: &Path) -> &'static str {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "otf" => "font/otf",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

/// The page sent with a 404.
pub(crate) const NOT_FOUND_PAGE: &str =
    "<!DOCTYPE html><title>Not found</title><p>This page does not exist.</p>\n";

/// Writes the status line and the shared headers, plus `extra` header lines (each
/// ending in `\r\n`). Preview responses are never cached and always close.
pub(crate) fn write_head(
    out: &mut impl Write,
    status: &str,
    kind: &str,
    length: u64,
    extra: &str,
) -> io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {length}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n{extra}\
         Connection: close\r\n\r\n"
    )
}

/// Sends a small in-memory response; HEAD gets only the head.
pub(crate) fn send_text(
    out: &mut impl Write,
    method: Method,
    status: &str,
    kind: &str,
    body: &str,
    extra: &str,
) -> io::Result<()> {
    write_head(out, status, kind, body.len() as u64, extra)?;
    if method == Method::Get {
        out.write_all(body.as_bytes())?;
    }
    out.flush()
}

/// Sends a file, streamed rather than read whole; HEAD gets only the head.
pub(crate) fn send_file(out: &mut impl Write, method: Method, path: &Path) -> io::Result<()> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    write_head(out, "200 OK", content_type(path), length, "")?;
    if method == Method::Get {
        io::copy(&mut (&mut file).take(length), out)?;
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::{Method, Refusal, parse_request, percent_decode, relative_path};
    use std::path::PathBuf;

    #[test]
    fn decodes_percent_escapes_and_keeps_stray_percents() {
        assert_eq!(percent_decode("/a%20b%2Fc"), b"/a b/c");
        assert_eq!(percent_decode("/100%"), b"/100%");
        assert_eq!(percent_decode("/%zz%4"), b"/%zz%4");
    }

    #[test]
    fn maps_paths_to_plain_relative_files() {
        assert_eq!(relative_path("/"), Ok(PathBuf::from("index.html")));
        assert_eq!(relative_path("/guide/"), Ok(PathBuf::from("guide/index.html")));
        assert_eq!(relative_path("//./a.css"), Ok(PathBuf::from("a.css")));
        assert_eq!(relative_path("/a/../b"), Err(Refusal::BadPath));
        assert_eq!(relative_path("/a\\b"), Err(Refusal::BadPath));
    }

    #[test]
    fn parses_request_lines() {
        let request = parse_request(b"HEAD /a%20b?q=1 HTTP/1.1\r\n\r\n").expect("request");
        assert_eq!(request.method, Method::Head);
        assert_eq!(request.raw_path, "/a%20b");
        assert_eq!(request.path, "/a b");
        assert_eq!(parse_request(b"DELETE / HTTP/1.1\r\n"), Err(Refusal::Method));
        assert_eq!(parse_request(b"GET\r\n"), Err(Refusal::Malformed));
        assert_eq!(parse_request(b"GET /%ff HTTP/1.1\r\n"), Err(Refusal::BadPath));
    }
}
