//! Diagnostics: the structured warnings and errors gd-rustdoc returns. They carry the
//! same parts as a `gd-report` report (code, title, message, hint, place, excerpt), so a
//! host can turn them into one field by field; `Display` lays them out the Elm way for
//! callers that only want text.

use std::fmt;

/// How serious a [`Diagnostic`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    /// Something was left out or could not be linked; the pages are still written.
    #[default]
    Warning,
    /// Nothing could be documented.
    Error,
}

/// A place in a source file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Position {
    /// The 1-based line.
    pub line: usize,
    /// The 1-based column, in characters.
    pub column: usize,
    /// The text of that line, without its line break.
    pub excerpt: String,
}

/// One problem found while loading or documenting a crate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagnostic {
    /// How serious it is.
    pub severity: Severity,
    /// A stable code such as `rust.autodoc.parse`, for filtering.
    pub code: &'static str,
    /// A short upper-case title such as `RUST SOURCE DOES NOT PARSE`.
    pub title: &'static str,
    /// The plain explanation.
    pub message: String,
    /// How to fix it.
    pub hint: String,
    /// The file it concerns, as the loader named it.
    pub file: Option<String>,
    /// Where in that file.
    pub position: Option<Position>,
}

impl Diagnostic {
    /// Starts a warning with a code, title, message and hint; it has no place yet.
    pub fn warning(code: &'static str, title: &'static str, message: String, hint: &str) -> Self {
        Diagnostic { code, title, message, hint: hint.to_string(), ..Diagnostic::default() }
    }

    /// Places the diagnostic in `file` at byte `offset` of its `source`, filling the line,
    /// column and excerpt. Offsets past the end are clamped.
    pub fn at(mut self, file: &str, source: &str, offset: usize) -> Self {
        let mut offset = offset.min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }
        let line_start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
        let line_end = source[offset..].find('\n').map_or(source.len(), |i| offset + i);
        self.file = Some(file.to_string());
        self.position = Some(Position {
            line: source[..offset].matches('\n').count() + 1,
            column: source[line_start..offset].chars().count() + 1,
            excerpt: source[line_start..line_end].trim_end_matches('\r').to_string(),
        });
        self
    }

    /// Names the file without a place in it.
    pub fn in_file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self
    }
}

/// The rule under a title is padded to this width, as in `gd-report`.
const RULE_WIDTH: usize = 80;

impl fmt::Display for Diagnostic {
    /// Lays the diagnostic out the Elm way: a title rule naming the place, the
    /// explanation, the source line with a caret, and the hint.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place = match (&self.file, &self.position) {
            (None, _) => String::new(),
            (Some(file), None) => file.clone(),
            (Some(file), Some(p)) => format!("{}:{}:{}", file, p.line, p.column),
        };
        let used = self.title.len() + place.len() + 5;
        let dashes = "-".repeat(RULE_WIDTH.saturating_sub(used).max(3));
        writeln!(f, "-- {} {} {}", self.title, dashes, place)?;
        writeln!(f)?;
        writeln!(f, "{}", self.message)?;
        if let Some(p) = self.position.as_ref().filter(|p| !p.excerpt.is_empty()) {
            let gutter = format!("{}| ", p.line);
            writeln!(f)?;
            writeln!(f, "{}{}", gutter, p.excerpt)?;
            let pad = gutter.len() + p.column.saturating_sub(1);
            writeln!(f, "{}^", " ".repeat(pad))?;
        }
        if !self.hint.is_empty() {
            writeln!(f)?;
            writeln!(f, "Hint: {}", self.hint)?;
        }
        Ok(())
    }
}

/// A crate that could not be loaded at all: its root file is missing, unreadable or too
/// large. Problems inside the crate are warnings on [`Crate`](crate::Crate) instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// What went wrong, with `severity` set to [`Severity::Error`]. Boxed, so a
    /// `Result` carrying the error stays small.
    pub diagnostic: Box<Diagnostic>,
}

impl Error {
    /// Makes an error from its parts; it has no place.
    pub(crate) fn new(
        code: &'static str,
        title: &'static str,
        message: String,
        hint: &str,
    ) -> Self {
        Error::from(Diagnostic::warning(code, title, message, hint))
    }
}

impl From<Diagnostic> for Error {
    /// Makes an error of a diagnostic, raising its severity.
    fn from(mut diagnostic: Diagnostic) -> Self {
        diagnostic.severity = Severity::Error;
        Error { diagnostic: Box::new(diagnostic) }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.diagnostic.fmt(f)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_has_title_place_caret_and_hint() {
        let source = "mod a;\nmod bad;\n";
        let d = Diagnostic::warning(
            "rust.autodoc.module",
            "MODULE FILE NOT FOUND",
            "No file holds `bad`.".to_string(),
            "Create src/bad.rs.",
        )
        .at("src/lib.rs", source, 11);
        let text = d.to_string();
        assert!(text.starts_with("-- MODULE FILE NOT FOUND ---"), "{text}");
        assert!(text.contains("src/lib.rs:2:5"), "{text}");
        assert!(text.contains("2| mod bad;\n       ^\n"), "{text}");
        assert!(text.ends_with("Hint: Create src/bad.rs.\n"), "{text}");
    }
}
