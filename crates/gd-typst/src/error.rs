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
    /// The Elm-style report of this diagnostic, with an excerpt of `source` (the text of
    /// [`Diagnostic::file`], when the caller has it) and carets under the span. A
    /// position outside `source` drops the excerpt. Copies what it needs.
    pub fn report(&self, source: Option<&str>) -> gd_report::Report {
        let (severity, code, title) = match self.severity {
            Severity::Error => (gd_report::Severity::Error, "typst.error", "TYPST ERROR"),
            Severity::Warning => (gd_report::Severity::Warning, "typst.warning", "TYPST"),
        };
        let mut report = gd_report::Report::error(code, title, &self.message);
        report.severity = severity;
        report.path.clone_from(&self.file);
        match source {
            Some(text) => self.place(&mut report, text),
            None => (report.line, report.column) = (self.line, self.column),
        }
        if self.hints.is_empty() {
            report.hints.push(self.default_hint().to_owned());
        }
        report.hints.extend(self.hints.iter().cloned());
        report
    }

    /// Renders [`Diagnostic::report`] as plain text, 80 columns wide.
    pub fn render(&self, source: Option<&str>) -> String {
        gd_report::render_text(&self.report(source), &gd_report::Style::default())
    }

    /// Positions `report` in `text`: at the span the embedded compiler gave when it
    /// starts at the reported position, else at the line and column.
    fn place(&self, report: &mut gd_report::Report, text: &str) {
        if self.line == 0 {
            return;
        }
        let start = crate::offset_of(text, self.line, self.column.max(1));
        match &self.span {
            Some(span) if span.end > span.start && span.start == start => {
                gd_report::set_span(report, text, span.start, span.end);
            }
            _ => gd_report::set_line_column(report, text, self.line, self.column.max(1)),
        }
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
}

/// Renders [`Diagnostic::report`] without an excerpt.
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.render(None).trim_end())
    }
}

/// Why Typst did not run, or its artifact was not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The source has errors: Typst wrote no artifact.
    Compile {
        /// The typst program's exit code; `None` for the embedded compiler, or when a
        /// signal stopped the program.
        exit_code: Option<i32>,
        /// Every error and warning Typst reported, in its order.
        diagnostics: Vec<Diagnostic>,
        /// The compiler's output (or the diagnostics as lines), bounded.
        detail: String,
    },
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
            Error::Compile { .. } => "typst.compile",
            Error::Launch { .. } => "typst.launch",
            Error::Io { .. } => "typst.io",
            Error::OutsideRoot { .. } => "typst.outside-root",
            Error::InvalidRequest => "typst.invalid-request",
            Error::Internal => "typst.internal",
            Error::NotEmbedded => "typst.not-embedded",
            Error::OutputTooLarge { .. } => "typst.output-too-large",
        }
    }

    /// The title of the report.
    pub fn title(&self) -> &'static str {
        match self {
            Error::Compile { .. } => "TYPST COMPILE FAILED",
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
            Error::Compile { exit_code, diagnostics, .. } => {
                compile_message(*exit_code, diagnostics)
            }
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
            Error::Compile { .. } => {
                "Fix the errors Typst reported; each diagnostic shows the source position."
            }
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

impl Error {
    /// The Elm-style report of this error, placed at the file it concerns.
    pub fn report(&self) -> gd_report::Report {
        let mut report = gd_report::Report::error(self.code(), self.title(), &self.message());
        report.hints.push(self.hint().to_owned());
        match self {
            Error::Io { path, .. } => report.path = path.display().to_string(),
            Error::OutsideRoot { source, .. } => report.path = source.display().to_string(),
            _ => {}
        }
        report
    }
}

/// Explains a failed compile by its first error.
fn compile_message(exit_code: Option<i32>, diagnostics: &[Diagnostic]) -> Cow<'static, str> {
    let errors = diagnostics.iter().filter(|d| d.severity == Severity::Error);
    let count = errors.clone().count();
    match (errors.clone().next(), exit_code) {
        (Some(first), _) if count == 1 => {
            Cow::Owned(format!("Typst reported an error: {}", first.message))
        }
        (Some(first), _) => {
            Cow::Owned(format!("Typst reported {count} errors; the first: {}", first.message))
        }
        (None, Some(code)) => Cow::Owned(format!("The typst program failed (exit code {code}).")),
        (None, None) => "Typst could not compile the document.".into(),
    }
}

/// Renders [`Error::report`] as plain text, 80 columns wide.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = gd_report::render_text(&self.report(), &gd_report::Style::default());
        f.write_str(text.trim_end())
    }
}

impl std::error::Error for Error {}
