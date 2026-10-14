//! Structured errors, rendered in the style of Elm's compiler.

use std::fmt;

/// What kind of problem an [`Error`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The PO text breaks the format.
    Syntax,
    /// A message is defined twice.
    Duplicate,
    /// The catalog is not UTF-8.
    Charset,
    /// Bytes that are not valid UTF-8.
    Encoding,
    /// A Plural-Forms header that cannot be used.
    Plural,
    /// A truncated or inconsistent MO file.
    MoFormat,
    /// The file could not be read or written.
    Io,
    /// There was not enough memory to hold the catalog.
    Memory,
    /// The file is larger than [`crate::LoadOptions::max_bytes`].
    TooLarge,
}

impl ErrorKind {
    /// Returns the report title of this kind, e.g. `PO SYNTAX ERROR`.
    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "PO SYNTAX ERROR",
            ErrorKind::Duplicate => "DUPLICATE MESSAGE",
            ErrorKind::Charset => "UNSUPPORTED CHARSET",
            ErrorKind::Encoding => "INVALID UTF-8",
            ErrorKind::Plural => "BAD PLURAL-FORMS",
            ErrorKind::MoFormat => "BAD MO FILE",
            ErrorKind::Io => "CANNOT ACCESS FILE",
            ErrorKind::Memory => "OUT OF MEMORY",
            ErrorKind::TooLarge => "CATALOG TOO LARGE",
        }
    }
}

/// A problem explained the way Elm's compiler explains one: where, what, and what to do.
///
/// The error owns its strings, so it outlives the input it was made from. `Display`
/// renders the report:
///
/// ```text
/// -- PO SYNTAX ERROR ------------------------------------------------------ de.po:2
///
/// This string has no closing quote.
///
/// 2| msgstr "b
///           ^
///
/// Hint: End the string with " on the same line; continue on the next line with ...
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// What went wrong.
    pub kind: ErrorKind,
    /// The file (or the name given to a parser); empty when there is none.
    pub file: String,
    /// The 1-based line of the problem; 0 when it has no line.
    pub line: usize,
    /// The 1-based byte column of the problem in `excerpt`; 0 for none.
    pub column: usize,
    /// The offending line; empty for none.
    pub excerpt: String,
    /// A plain explanation.
    pub message: String,
    /// How to fix it.
    pub hint: String,
}

impl Error {
    /// Builds an error from borrowed parts; every string is copied.
    pub(crate) fn new(kind: ErrorKind, file: &str, message: &str, hint: &str) -> Error {
        Error {
            kind,
            file: file.to_string(),
            line: 0,
            column: 0,
            excerpt: String::new(),
            message: message.to_string(),
            hint: hint.to_string(),
        }
    }

    /// Sets the line, column and excerpt.
    pub(crate) fn at(mut self, line: usize, column: usize, excerpt: &str) -> Error {
        self.line = line;
        self.column = column;
        self.excerpt = excerpt.to_string();
        self
    }

    /// Returns the report title, e.g. `PO SYNTAX ERROR`.
    pub fn title(&self) -> &'static str {
        self.kind.title()
    }
}

/// The error of an allocation that was refused (`try_reserve`).
pub(crate) fn memory_error(file: &str) -> Error {
    Error::new(
        ErrorKind::Memory,
        file,
        "There was not enough memory to hold the catalog.",
        "Free memory and run the command again, or split the catalog into smaller .po \
         files, one per text domain.",
    )
}

/// The error of a file that could not be read.
pub(crate) fn read_error(path: &str, err: &std::io::Error) -> Error {
    Error::new(
        ErrorKind::Io,
        path,
        &format!("Cannot read {path}: {err}."),
        "Check the path and the file's permissions.",
    )
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.title();
        let mut place =
            if self.file.is_empty() { "<catalog>".to_string() } else { self.file.clone() };
        if self.line > 0 {
            place = format!("{place}:{}", self.line);
        }
        let dashes = 80usize.saturating_sub(title.len() + place.len() + 5).max(3);
        writeln!(f, "-- {title} {} {place}\n\n{}", "-".repeat(dashes), self.message)?;
        if !self.excerpt.is_empty() {
            let gutter = format!("{}| ", self.line);
            writeln!(f, "\n{gutter}{}", self.excerpt.replace('\t', " "))?;
            if self.column > 0 {
                let pad = gutter.len() + column_width(&self.excerpt, self.column - 1);
                writeln!(f, "{}^", " ".repeat(pad))?;
            }
        }
        if !self.hint.is_empty() {
            writeln!(f, "\nHint: {}", self.hint)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// Counts the characters of the first `n` bytes of `s`, for the caret.
fn column_width(s: &str, n: usize) -> usize {
    s.char_indices().take_while(|&(i, _)| i < n).count()
}
