//! Errors: serde_json's failures turned into located, Elm-style diagnostics.

use gd_report::Report;
use std::fmt;

/// What kind of mistake a JSON text (or a conversion) has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The text is not JSON at this point: a missing comma, colon or bracket.
    Syntax,
    /// The text ends before the value is complete.
    Eof,
    /// A string holds a bad escape or raw control character.
    String,
    /// A number is malformed or too large.
    Number,
    /// An object has the same key twice.
    Duplicate,
    /// Arrays and objects nest deeper than the limit allows.
    Depth,
    /// A value does not have the shape a program expects ([`crate::from_value`]).
    Data,
}

impl ErrorKind {
    /// The stable diagnostic code, such as `"json.syntax"`.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "json.syntax",
            ErrorKind::Eof => "json.eof",
            ErrorKind::String => "json.string",
            ErrorKind::Number => "json.number",
            ErrorKind::Duplicate => "json.duplicate",
            ErrorKind::Depth => "json.depth",
            ErrorKind::Data => "json.data",
        }
    }

    /// The report title, such as `"JSON SYNTAX ERROR"`.
    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "JSON SYNTAX ERROR",
            ErrorKind::Eof => "JSON ENDS EARLY",
            ErrorKind::String => "BAD JSON STRING",
            ErrorKind::Number => "BAD JSON NUMBER",
            ErrorKind::Duplicate => "DUPLICATE KEY",
            ErrorKind::Depth => "TOO DEEPLY NESTED",
            ErrorKind::Data => "UNEXPECTED JSON DATA",
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
    /// 1-based line of `offset`; 0 when the error has no position ([`ErrorKind::Data`]
    /// from a conversion).
    pub line: usize,
    /// 1-based column of `offset`, counting Unicode scalars; 0 when there is no position.
    pub column: usize,
}

impl Error {
    /// Builds an Elm-style report naming the file `path`, with the line of `source` (the
    /// text given to the parser) and carets under the problem.
    pub fn report(&self, path: &str, source: &str) -> Report {
        let report = Report::error(self.kind.code(), self.kind.title(), &self.message);
        report.hint(&self.hint).at(path, source, self.offset, self.end)
    }

    /// An error without a position, from [`crate::from_value`] or [`crate::to_value`].
    pub(crate) fn from_data(err: &serde_json::Error) -> Error {
        Error {
            kind: ErrorKind::Data,
            message: sentence(&err.to_string()),
            hint: "Check the field names and value types against what the program expects."
                .to_string(),
            offset: 0,
            end: 0,
            line: 0,
            column: 0,
        }
    }

    /// Locates a parse failure of `err` in `text`, of which serde_json read the part after
    /// the first `skipped` bytes (a byte order mark). `refusal` is set when the parser's
    /// own checks (duplicate keys, depth) stopped serde_json.
    pub(crate) fn from_parse(
        err: &serde_json::Error,
        text: &str,
        skipped: usize,
        refusal: Option<Refusal>,
    ) -> Error {
        let what = serde_message(err);
        let key = refusal.as_ref().and_then(|r| r.key.clone());
        let (kind, message, hint) = match refusal {
            Some(r) => (r.kind, r.message, r.hint),
            None => {
                let (kind, message, hint) = describe(&what);
                (kind, message.unwrap_or_else(|| sentence(&what)), hint)
            }
        };
        let offset = if err.is_eof() {
            text.trim_end().len()
        } else {
            skipped + offset_of(&text[skipped..], err.line(), err.column())
        };
        let offset = gd_report::floor_boundary(text, offset.min(text.len()));
        let end = gd_report::floor_boundary(text, offset + 1);
        let (line, column) = gd_report::line_column(text, offset);
        let error = Error { kind, message, hint: hint.to_string(), offset, end, line, column };
        match key {
            Some(key) => error.span_key(text, &key),
            None => error,
        }
    }

    /// Widens the span backwards over `"key"` when the text just before `end` holds it,
    /// so a duplicate key is underlined whole.
    fn span_key(mut self, text: &str, key: &str) -> Error {
        let quoted = serde_json::to_string(key).unwrap_or_default();
        if text[..self.end].ends_with(&quoted) {
            self.offset = self.end - quoted.len();
            (self.line, self.column) = gd_report::line_column(text, self.offset);
        }
        self
    }
}

/// What the parser's own checks report instead of serde_json's message.
#[derive(Debug, Clone)]
pub(crate) struct Refusal {
    pub kind: ErrorKind,
    pub message: String,
    pub hint: &'static str,
    /// The repeated key, to underline it whole.
    pub key: Option<String>,
}

/// serde_json's message without its " at line L column C" suffix.
fn serde_message(err: &serde_json::Error) -> String {
    let full = err.to_string();
    match full.rfind(" at line ") {
        Some(i) => full[..i].to_string(),
        None => full,
    }
}

/// The byte offset of serde_json's position: `column` is the 1-based byte column of the
/// character it stopped at (0 at the start of a line).
fn offset_of(text: &str, line: usize, column: usize) -> usize {
    let line_start: usize =
        text.split_inclusive('\n').take(line.saturating_sub(1)).map(str::len).sum();
    line_start + column.saturating_sub(1)
}

/// Capitalizes serde_json's message and ends it with a full stop.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => "The JSON text is not valid.".to_string(),
    }
}

const VALUE_HINT: &str = "A value is a \"string\", a number, true, false, null, an array \
                          [ ... ] or an object { ... }; JSON has no comments.";
const SEPARATOR_HINT: &str =
    "Check for a missing comma or quote near here, and close every [ and { you open.";
const EOF_HINT: &str = "The document ends early: close the open brackets and quotes.";
const ESCAPE_HINT: &str = "Valid escapes are \\\" \\\\ \\/ \\b \\f \\n \\r \\t and \\uXXXX; \
                           write a surrogate pair as two \\u escapes.";
const NUMBER_HINT: &str = "Write numbers like 0, -12, 3.5 or 1e9: no leading zeros, no \
                           leading +, and digits after a point or exponent.";

/// The kind, a better message (when there is one) and the hint for serde_json's message.
fn describe(what: &str) -> (ErrorKind, Option<String>, &'static str) {
    let (kind, message, hint): (ErrorKind, &str, &str) = match what {
        "EOF while parsing a list" => (ErrorKind::Eof, "The text ends inside an array.", EOF_HINT),
        "EOF while parsing an object" => {
            (ErrorKind::Eof, "The text ends inside an object.", EOF_HINT)
        }
        "EOF while parsing a string" => {
            (ErrorKind::Eof, "The text ends inside a string.", "Close the string with \".")
        }
        "EOF while parsing a value" => {
            (ErrorKind::Eof, "The text ends where a value should be.", EOF_HINT)
        }
        "expected `:`" => (
            ErrorKind::Syntax,
            "Expected a colon after this object key.",
            "Write each member as \"key\": value, separated by commas.",
        ),
        "expected `,` or `]`" => {
            (ErrorKind::Syntax, "Expected , or ] in this array.", SEPARATOR_HINT)
        }
        "expected `,` or `}`" => {
            (ErrorKind::Syntax, "Expected , or } in this object.", SEPARATOR_HINT)
        }
        "expected ident" | "expected value" => {
            (ErrorKind::Syntax, "Expected a value here.", VALUE_HINT)
        }
        "key must be a string" | "expected `\"`" => (
            ErrorKind::Syntax,
            "Object keys must be strings in double quotes.",
            "Quote the key: {\"name\": 1}, not {name: 1}.",
        ),
        "trailing comma" => (
            ErrorKind::Syntax,
            "There is a comma before the closing bracket.",
            "JSON has no trailing commas; remove the last comma.",
        ),
        "trailing characters" => (
            ErrorKind::Syntax,
            "There is more text after the JSON value.",
            "A JSON text holds one value; wrap several in an array [ ... ].",
        ),
        "invalid number" => (ErrorKind::Number, "This number is malformed.", NUMBER_HINT),
        "number out of range" => (
            ErrorKind::Number,
            "This number is too large for a 64-bit float.",
            "Write very large numbers as strings.",
        ),
        "recursion limit exceeded" => (
            ErrorKind::Depth,
            "Arrays and objects nest too deeply.",
            "Flatten the data; JSON nesting is limited to keep the parser's stack bounded.",
        ),
        _ if what.starts_with("control character") => (
            ErrorKind::String,
            "A string holds a raw control character, such as a line break.",
            "Escape it: write \\n for a line break and \\t for a tab.",
        ),
        _ if what.contains("escape") || what.contains("unicode code point") => {
            (ErrorKind::String, "A string holds an invalid escape.", ESCAPE_HINT)
        }
        _ => return (ErrorKind::Syntax, None, SEPARATOR_HINT),
    };
    (kind, Some(message.to_string()), hint)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            return f.write_str(&self.message);
        }
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for Error {}
