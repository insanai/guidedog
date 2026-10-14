//! The error value every operation returns, rendered in the Elm style.

use std::fmt;
use std::path::{Path, PathBuf};

/// What kind of failure an [`Error`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// A tree was asked to hold a path Git refuses, or two files with one path.
    InvalidPath,
    /// A file could not be read, created or written.
    Io,
    /// A file changed between taking its ID and packing it.
    Changed,
    /// A receive-pack message is malformed, or a field cannot be sent.
    Protocol,
    /// An input is larger than the limits allow.
    TooLarge,
    /// A pack writer was used out of order (too many or too few objects).
    Pack,
}

impl ErrorKind {
    /// A short, stable code for the kind, such as `git.invalid-path`.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::InvalidPath => "git.invalid-path",
            ErrorKind::Io => "git.io",
            ErrorKind::Changed => "git.changed",
            ErrorKind::Protocol => "git.protocol",
            ErrorKind::TooLarge => "git.too-large",
            ErrorKind::Pack => "git.pack",
        }
    }

    /// The report's title, in capitals.
    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::InvalidPath => "INVALID GIT PATH",
            ErrorKind::Io => "FILE ERROR",
            ErrorKind::Changed => "FILE CHANGED",
            ErrorKind::Protocol => "GIT PROTOCOL ERROR",
            ErrorKind::TooLarge => "TOO LARGE",
            ErrorKind::Pack => "PACK ERROR",
        }
    }
}

/// A failure: its kind, the file it concerns (if any), a plain message and a hint.
///
/// `Display` renders it in the Elm style: a title line, the message and the hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// What kind of failure this is.
    pub kind: ErrorKind,
    /// The file the failure concerns, when there is one.
    pub path: Option<PathBuf>,
    /// What went wrong, as a sentence.
    pub message: String,
    /// How to fix it, as a sentence.
    pub hint: String,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>, hint: &str) -> Error {
        Error { kind, path: None, message: message.into(), hint: hint.to_string() }
    }

    pub(crate) fn unreadable(path: &Path, err: &std::io::Error) -> Error {
        Error {
            kind: ErrorKind::Io,
            path: Some(path.to_path_buf()),
            message: format!("{} cannot be read: {err}.", path.display()),
            hint: "Check that the file exists and that you may read it.".to_string(),
        }
    }

    pub(crate) fn uncreatable(path: &Path, err: &std::io::Error) -> Error {
        Error {
            kind: ErrorKind::Io,
            path: Some(path.to_path_buf()),
            message: format!("{} cannot be created: {err}.", path.display()),
            hint: "The file must not exist yet, and its folder must be writable.".to_string(),
        }
    }

    pub(crate) fn unwritable(path: &Path, what: &str) -> Error {
        Error {
            kind: ErrorKind::Io,
            path: Some(path.to_path_buf()),
            message: format!("The {what} cannot be written."),
            hint: "Check that the disk has room and the folder is writable.".to_string(),
        }
    }

    pub(crate) fn changed(path: &Path, how: &str) -> Error {
        Error {
            kind: ErrorKind::Changed,
            path: Some(path.to_path_buf()),
            message: format!("{} {how}.", path.display()),
            hint: "Wait until nothing writes to the site, then publish again.".to_string(),
        }
    }

    pub(crate) fn protocol(message: impl Into<String>) -> Error {
        Error::new(
            ErrorKind::Protocol,
            message,
            "Check that the remote is a Git server offering receive-pack.",
        )
    }
}

impl Error {
    /// The Elm-style report of this error, for [`gd_report::render_text`] or
    /// [`gd_report::render_json`]; it names the file when there is one.
    pub fn report(&self) -> gd_report::Report {
        let mut report =
            gd_report::Report::error(self.kind.code(), self.kind.title(), &self.message)
                .hint(&self.hint);
        if let Some(path) = &self.path {
            report.path = path.display().to_string();
        }
        report
    }
}

/// Renders the [`Error::report`] as plain text, 80 columns wide.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = gd_report::render_text(&self.report(), &gd_report::Style::default());
        f.write_str(text.trim_end())
    }
}

impl std::error::Error for Error {}
