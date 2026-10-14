//! Parse errors.

use gd_report::Report;
use std::fmt;

/// What kind of mistake a TOML source has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The text is not TOML at this point: a missing `=`, bracket or line break.
    Syntax,
    /// A key or table is defined twice, or extended where TOML forbids it.
    Duplicate,
    /// A string is unterminated or holds a bad escape or control character.
    String,
    /// A number is malformed or does not fit in 64 bits.
    Number,
    /// A date or time is malformed or out of range.
    Datetime,
    /// Arrays or inline tables nest deeper than [`crate::MAX_DEPTH`].
    Depth,
}

impl ErrorKind {
    /// The stable diagnostic code, such as `"toml.duplicate"`.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "toml.syntax",
            ErrorKind::Duplicate => "toml.duplicate",
            ErrorKind::String => "toml.string",
            ErrorKind::Number => "toml.number",
            ErrorKind::Datetime => "toml.datetime",
            ErrorKind::Depth => "toml.depth",
        }
    }

    /// The report title, such as `"DUPLICATE KEY"`.
    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "TOML SYNTAX ERROR",
            ErrorKind::Duplicate => "DUPLICATE KEY",
            ErrorKind::String => "BAD STRING",
            ErrorKind::Number => "BAD NUMBER",
            ErrorKind::Datetime => "BAD DATE-TIME",
            ErrorKind::Depth => "TOO DEEPLY NESTED",
        }
    }
}

/// Where and why a source is not TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The kind of mistake.
    pub kind: ErrorKind,
    /// What is wrong, as a sentence.
    pub message: String,
    /// How to fix it, as a sentence.
    pub hint: String,
    /// Byte offset where the problem starts.
    pub offset: usize,
    /// Byte offset where the problem ends (at least `offset`).
    pub end: usize,
    /// 1-based line of `offset`.
    pub line: usize,
    /// 1-based column of `offset`, counting Unicode scalars.
    pub column: usize,
}

impl Error {
    /// Builds an Elm-style report naming the file `path`, with the line of `source` (the
    /// text given to [`crate::parse`]) and carets under the problem.
    pub fn report(&self, path: &str, source: &str) -> Report {
        let report = Report::error(self.kind.code(), self.kind.title(), &self.message);
        report.hint(&self.hint).at(path, source, self.offset, self.end)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for Error {}
