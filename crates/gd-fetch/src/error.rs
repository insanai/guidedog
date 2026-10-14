//! Fetch errors: a kind, the location, the HTTP status, a detail, and an Elm-style report.

use std::borrow::Cow;
use std::fmt::{self, Write as _};

/// Why a [`crate::get`] or [`crate::send`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Not http, https or a file; or a request `send` cannot make.
    Unsupported,
    /// No such file, or it could not be read.
    Missing,
    /// The connection or transfer failed.
    Network,
    /// The transfer took longer than the timeout.
    Timeout,
    /// The server answered with a status other than 2xx.
    Status,
    /// The body or the headers were larger than the limit.
    TooLarge,
    /// `send` was answered with a redirect (3xx), which it does not follow.
    Redirect,
    /// `Options::cancel` was set during the transfer.
    Cancelled,
    /// Memory for a complete result could not be reserved.
    Memory,
    /// The `curl` executable could not be run.
    CurlMissing,
}

/// A fetch failure, with what a report needs. `Display` renders an Elm-style report with a
/// title, the location, an explanation and a hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// What kind of failure it is.
    pub kind: ErrorKind,
    /// The URL or path the failure concerns.
    pub location: String,
    /// The HTTP status, or 0 when none was received.
    pub status: u16,
    /// What went wrong, in a sentence (curl's own message for network failures).
    pub detail: Cow<'static, str>,
}

impl Error {
    /// Makes an error of `kind` for `location`, copying the location; a `&'static str`
    /// detail is borrowed, a `String` moved.
    pub fn new(kind: ErrorKind, location: &str, detail: impl Into<Cow<'static, str>>) -> Error {
        Error { kind, location: location.to_owned(), status: 0, detail: detail.into() }
    }

    /// The report's title, in capitals.
    pub fn title(&self) -> &'static str {
        match self.kind {
            ErrorKind::Unsupported => "UNSUPPORTED REQUEST",
            ErrorKind::Missing => "FILE NOT FOUND",
            ErrorKind::Network => "NETWORK FAILURE",
            ErrorKind::Timeout => "TRANSFER TIMED OUT",
            ErrorKind::Status => "HTTP ERROR STATUS",
            ErrorKind::TooLarge => "RESPONSE TOO LARGE",
            ErrorKind::Redirect => "UNEXPECTED REDIRECT",
            ErrorKind::Cancelled => "TRANSFER CANCELLED",
            ErrorKind::Memory => "OUT OF MEMORY",
            ErrorKind::CurlMissing => "CURL NOT FOUND",
        }
    }

    /// How to fix the failure, in a sentence.
    pub fn hint(&self) -> &'static str {
        match self.kind {
            ErrorKind::Unsupported => {
                "Use an http:// or https:// URL (or a local path for reads), and keep \
                 methods, URLs and headers on one line."
            }
            ErrorKind::Missing => "Check that the path is spelled right and the file is readable.",
            ErrorKind::Network => {
                "Check the network connection and the host name; behind a proxy, set \
                 HTTPS_PROXY."
            }
            ErrorKind::Timeout => "Try again later, or raise the timeout.",
            ErrorKind::Status => status_hint(self.status),
            ErrorKind::TooLarge => "Raise the size limit if the resource is meant to be large.",
            ErrorKind::Redirect => "Send the request to the URL in the location header.",
            ErrorKind::Cancelled => "The transfer was cancelled on request; run it again.",
            ErrorKind::Memory => "Free memory, or lower the size limit.",
            ErrorKind::CurlMissing => {
                "Install curl (for example `apt install curl` or `brew install curl`) and \
                 put it on PATH."
            }
        }
    }

    /// Writes the plain explanation into `f`.
    fn write_explanation(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self.kind {
            ErrorKind::Status => {
                write!(f, "The server answered with HTTP status {}.", self.status)
            }
            _ if self.detail.is_empty() => {
                write!(f, "The transfer of `{}` failed.", self.location)
            }
            _ => f.write_str(&self.detail),
        }
    }
}

fn status_hint(status: u16) -> &'static str {
    match status {
        401 | 403 => "Check the credentials or token, and that they grant access.",
        404 | 410 => "Check the URL; the resource may have moved or been removed.",
        429 => "The server limits requests; wait a while and try again.",
        500..=599 => "The server failed; try again later.",
        _ => "Check the URL and the request the server refused.",
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.title();
        let used = 3 + title.len() + 1 + 1 + self.location.len();
        let dashes = 80usize.saturating_sub(used).max(3);
        write!(f, "-- {title} ")?;
        for _ in 0..dashes {
            f.write_char('-')?;
        }
        writeln!(f, " {}\n", self.location)?;
        self.write_explanation(f)?;
        writeln!(f, "\n")?;
        write!(f, "Hint: {}", self.hint())
    }
}

impl std::error::Error for Error {}
