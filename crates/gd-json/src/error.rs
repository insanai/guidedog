//! Parse errors.

use gd_report::Report;
use std::fmt;

/// What kind of mistake a JSON text has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The text is not JSON at this point: a missing comma, colon or bracket.
    Syntax,
    /// A string is unterminated or holds a bad escape or raw control character.
    String,
    /// A number is malformed.
    Number,
    /// An object has the same key twice.
    Duplicate,
    /// Arrays and objects nest deeper than the limit allows.
    Depth,
}

impl ErrorKind {
    /// The stable diagnostic code, such as `"json.syntax"`.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "json.syntax",
            ErrorKind::String => "json.string",
            ErrorKind::Number => "json.number",
            ErrorKind::Duplicate => "json.duplicate",
            ErrorKind::Depth => "json.depth",
        }
    }

    /// The report title, such as `"JSON SYNTAX ERROR"`.
    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "JSON SYNTAX ERROR",
            ErrorKind::String => "BAD JSON STRING",
            ErrorKind::Number => "BAD JSON NUMBER",
            ErrorKind::Duplicate => "DUPLICATE KEY",
            ErrorKind::Depth => "TOO DEEPLY NESTED",
        }
    }
}

/// Where and why a text is not acceptable JSON.
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
    /// text given to the parser) and carets under the problem.
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
