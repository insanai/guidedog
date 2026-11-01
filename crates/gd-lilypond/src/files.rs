use crate::{Error, FileId, Severity};

/// Retained input text. The name is resolved by the adapter, not by this library.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// Display and dependency identity.
    pub name: String,
    /// Original UTF-8 text, before tokenization.
    pub text: String,
}

/// Sources indexed by FileId; file zero is the directive's supplied source.
#[derive(Clone, Debug, Default)]
pub struct SourceFiles {
    /// Files in first-read order, including files that caused an error.
    pub files: Vec<SourceFile>,
}

impl SourceFiles {
    /// Borrows one file, returning None for an unknown identifier.
    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }
}

/// A failed parse or interpretation that keeps enough source to place its diagnostic.
#[derive(Clone, Debug)]
pub struct Failed {
    /// The first fatal problem.
    pub error: Error,
    /// Every file read before the failure.
    pub files: SourceFiles,
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for Failed {}

/// Borrows a shared diagnostic and retained source to create an owned report.
/// Missing file identifiers produce an unplaced report rather than a panic.
pub fn error_report(error: &Error, files: &SourceFiles) -> gd_report::Report {
    let title = match error.kind {
        crate::ErrorKind::UnknownCommand => "UNKNOWN MUSIC COMMAND",
        crate::ErrorKind::Unsupported => "UNSUPPORTED LILYPOND",
        crate::ErrorKind::BarCheck => "BAR CHECK FAILED",
        crate::ErrorKind::Lyrics => "UNUSED LYRICS",
        crate::ErrorKind::ReadLimit | crate::ErrorKind::EngraveLimit => "MUSIC LIMIT REACHED",
        _ => "MUSIC NOT ENGRAVED",
    };
    let mut report = gd_report::Report::error(error.kind.code(), title, &error.message);
    if error.severity == Severity::Warning {
        report.severity = gd_report::Severity::Warning;
    }
    if !error.hint.is_empty() {
        report.hints.push(error.hint.clone());
    }
    if let Some(file) = files.file(error.span.file) {
        report = report.at(&file.name, &file.text, error.span.start, error.span.end);
    }
    report
}
