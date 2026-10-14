//! Structured errors and diagnostics, rendered in the Elm style: a title bar with the
//! location, a plain explanation, a source excerpt with a caret when there is one, and a
//! hint saying how to fix it.

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;

/// How serious a [`Diagnostic`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    /// The compile failed.
    #[default]
    Error,
    /// The compile went on.
    Warning,
}

impl Severity {
    /// The lowercase word Typst prints: `"error"` or `"warning"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One problem Typst reported.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagnostic {
    /// The file, root-relative (`/book.typ`), a package path, or as the program printed it.
    pub file: String,
    /// 1-based; 0 when the compiler gave no position.
    pub line: usize,
    /// 1-based, in characters; 0 when the compiler gave no position.
    pub column: usize,
    /// The byte range in the file's text, when the embedded compiler knew it.
    pub span: Option<std::ops::Range<usize>>,
    /// Error or warning.
    pub severity: Severity,
    /// What Typst said.
    pub message: String,
    /// Typst's suggestions for fixing it.
    pub hints: Vec<String>,
    /// The position is in the generated text compiled by `compile_text`.
    pub stdin: bool,
}

impl Diagnostic {
    /// Renders the diagnostic in the Elm style with an excerpt of `source`, the text of
    /// [`Diagnostic::file`] when the caller has it, and a caret under the span. Returns
    /// an owned report; never fails (a position outside `source` drops the excerpt).
    pub fn report(&self, source: Option<&str>) -> String {
        let mut out = String::with_capacity(160 + self.message.len());
        // Writing into a String cannot fail.
        let _ = self.write_report(&mut out, source);
        out
    }

    /// Writes the report [`Diagnostic::report`] returns into `out`, so a caller reporting
    /// many diagnostics reuses one buffer. Fails only when `out` does.
    pub fn write_report(&self, out: &mut impl fmt::Write, source: Option<&str>) -> fmt::Result {
        let title = match self.severity {
            Severity::Error => "TYPST ERROR",
            Severity::Warning => "TYPST WARNING",
        };
        write_bar(out, title, &Location(self))?;
        write!(out, "\n\n{}", self.message)?;
        if let Some(text) = source {
            self.write_excerpt(out, text)?;
        }
        if self.hints.is_empty() {
            write!(out, "\nHint: {}", self.default_hint())?;
        }
        for hint in &self.hints {
            write!(out, "\nHint: {hint}")?;
        }
        Ok(())
    }

    fn default_hint(&self) -> &'static str {
        match self.severity {
            Severity::Error => {
                "Correct the Typst source at this position; see \
                https://typst.app/docs for the syntax and functions it uses."
            }
            Severity::Warning => {
                "The output was still written; fix the source to silence \
                this warning."
            }
        }
    }

    /// Writes `\n\n12| line` and a caret line, or nothing when the position is not in
    /// `text`.
    fn write_excerpt(&self, out: &mut impl fmt::Write, text: &str) -> fmt::Result {
        if self.line == 0 {
            return Ok(());
        }
        let Some(line_text) = text.split('\n').nth(self.line - 1) else { return Ok(()) };
        let line_text = line_text.trim_end_matches('\r');
        let start = crate::offset_of(text, self.line, self.column.max(1));
        let line_start = crate::offset_of(text, self.line, 1);
        let width = match &self.span {
            Some(span) if span.end > span.start && span.start == start => {
                let end = span.end.min(line_start + line_text.len());
                text.get(start..end).map_or(1, |s| s.chars().count().max(1))
            }
            _ => 1,
        };
        let number = self.line;
        let indent = number.ilog10() as usize + 1 + 2 + self.column.max(1) - 1;
        write!(out, "\n\n{number}| {line_text}\n{:indent$}{:^<width$}", "", "")
    }
}

/// A diagnostic's `file:line:column`, `file`, or nothing.
struct Location<'a>(&'a Diagnostic);

impl fmt::Display for Location<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.0;
        match (d.file.is_empty(), d.line) {
            (true, _) => Ok(()),
            (false, 0) => f.write_str(&d.file),
            (false, line) => write!(f, "{}:{}:{}", d.file, line, d.column),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_report(f, None)
    }
}

/// Why Typst did not run, or its artifact was not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The typst program could not be started.
    Launch {
        /// The program that was run.
        program: String,
        /// What the operating system said.
        message: String,
    },
    /// A file the compile needs could not be written or read.
    Io {
        /// The file.
        path: PathBuf,
        /// What the operating system said.
        message: String,
    },
    /// The source file is not below the Typst root.
    OutsideRoot {
        /// The source given.
        source: PathBuf,
        /// The root given.
        root: PathBuf,
    },
    /// The embedded compiler could not use the request (a path or date it cannot hold).
    InvalidRequest,
    /// The embedded compiler failed internally (it panicked).
    Internal,
    /// This build has no embedded compiler.
    NotEmbedded,
    /// The caller's `reserve` refused the output's size.
    OutputTooLarge {
        /// The output's size in bytes.
        bytes: usize,
    },
}

impl Error {
    /// A short, stable code for the error kind.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Launch { .. } => "typst-launch",
            Error::Io { .. } => "typst-io",
            Error::OutsideRoot { .. } => "typst-outside-root",
            Error::InvalidRequest => "typst-invalid-request",
            Error::Internal => "typst-internal",
            Error::NotEmbedded => "typst-not-embedded",
            Error::OutputTooLarge { .. } => "typst-output-too-large",
        }
    }

    /// The title of the report.
    pub fn title(&self) -> &'static str {
        match self {
            Error::Launch { .. } => "TYPST DID NOT RUN",
            Error::Io { .. } => "TYPST FILE ERROR",
            Error::OutsideRoot { .. } => "SOURCE OUTSIDE ROOT",
            Error::InvalidRequest => "INVALID TYPST REQUEST",
            Error::Internal => "TYPST INTERNAL FAILURE",
            Error::NotEmbedded => "NO EMBEDDED TYPST",
            Error::OutputTooLarge { .. } => "TYPST OUTPUT TOO LARGE",
        }
    }

    /// The plain explanation; borrowed for the kinds whose text is fixed.
    pub fn message(&self) -> Cow<'static, str> {
        match self {
            Error::Launch { program, message } => Cow::Owned(format!(
                "The Typst program `{program}` could not be started: {message}."
            )),
            Error::Io { path, message } => Cow::Owned(format!("`{}`: {message}.", path.display())),
            Error::OutsideRoot { source, root } => Cow::Owned(format!(
                "The source `{}` is outside the Typst root `{}`.",
                source.display(),
                root.display()
            )),
            Error::InvalidRequest => "The embedded Typst compiler rejected the request.".into(),
            Error::Internal => "The Typst compiler failed internally.".into(),
            Error::NotEmbedded => "This build has no embedded Typst compiler.".into(),
            Error::OutputTooLarge { bytes } => Cow::Owned(format!(
                "The output ({bytes} bytes) is larger than the memory budget allows."
            )),
        }
    }

    /// How to fix it.
    pub fn hint(&self) -> &'static str {
        match self {
            Error::Launch { .. } => {
                "Install Typst 0.15.1 (https://github.com/typst/typst/\
                releases) and put it on PATH, or use a build with the embedded compiler."
            }
            Error::Io { .. } => "Check that the directory exists and is writable.",
            Error::OutsideRoot { .. } => {
                "Move the source below the root, or pass a root that contains it."
            }
            Error::InvalidRequest => {
                "Use a root and source path in valid UTF-8, and a \
                SOURCE_DATE_EPOCH within the years Typst supports."
            }
            Error::Internal => {
                "This is a bug in Typst; report it with the document that \
                triggers it, and try the installed typst program meanwhile."
            }
            Error::NotEmbedded => {
                "Build with the `embedded` feature, or locate an \
                installed `typst` program instead."
            }
            Error::OutputTooLarge { .. } => {
                "Raise the output budget, or split the document into smaller ones."
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, .. } => write_bar(f, self.title(), &path.display())?,
            Error::OutsideRoot { source, .. } => write_bar(f, self.title(), &source.display())?,
            _ => write_bar(f, self.title(), &"")?,
        }
        write!(f, "\n\n{}\n\nHint: {}", self.message(), self.hint())
    }
}

impl std::error::Error for Error {}

/// Writes `-- TITLE ------------------------------ location`, 80 columns wide when it
/// fits; the location is measured without being formatted into a buffer.
fn write_bar(out: &mut impl fmt::Write, title: &str, location: &dyn fmt::Display) -> fmt::Result {
    let mut counted = Count(0);
    // Counting cannot fail.
    let _ = fmt::Write::write_fmt(&mut counted, format_args!("{location}"));
    let dashes = 80usize.saturating_sub(title.chars().count() + counted.0 + 5).max(3);
    write!(out, "-- {title} {:-<dashes$}", "")?;
    if counted.0 > 0 {
        write!(out, " {location}")?;
    }
    Ok(())
}

/// A sink that counts characters.
struct Count(usize);

impl fmt::Write for Count {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0 += s.chars().count();
        Ok(())
    }
}
