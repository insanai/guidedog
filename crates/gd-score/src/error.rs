use std::fmt;

/// An index into a reader's retained source files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

impl FileId {
    /// Unplaced source sentinel; file zero is the root input file.
    pub const NONE: Self = Self(u32::MAX);
}

impl Default for FileId {
    fn default() -> Self {
        Self::NONE
    }
}

/// A half-open byte range in a source file. The default is an unplaced diagnostic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// The source file that owns the range.
    pub file: FileId,
    /// First byte, inclusive.
    pub start: usize,
    /// Last byte, exclusive.
    pub end: usize,
}

impl Span {
    /// Returns whether this span carries no source-file attachment.
    pub fn is_unplaced(self) -> bool {
        self.file == FileId::NONE
    }
}

/// Whether processing can continue after a diagnostic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Severity {
    /// The input cannot be rendered faithfully.
    #[default]
    Error,
    /// The drawing remains usable.
    Warning,
}

/// Stable problem categories shared by readers and engravers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Malformed LilyPond notation.
    Syntax,
    /// No note, rest or spacer exists.
    Empty,
    /// A command is not known to the profile.
    UnknownCommand,
    /// A known construct is outside the implemented profile.
    Unsupported,
    /// A pitch or accidental is invalid.
    Pitch,
    /// A duration is invalid.
    Duration,
    /// A written bar check is misplaced.
    BarCheck,
    /// Lyrics could not all be attached.
    Lyrics,
    /// An include was refused or could not be read.
    Include,
    /// The reader exceeded a declared bound.
    ReadLimit,
    /// An exact time does not fit the rational representation.
    TimeOverflow,
    /// A programmatically constructed score violates an invariant.
    Score,
    /// A single measure exceeds the line width.
    Overfull,
    /// A required font or glyph is unavailable.
    Font,
    /// Engraving exceeded a declared bound.
    EngraveLimit,
}

impl ErrorKind {
    /// Returns the diagnostic's stable code without allocating.
    pub fn code(self) -> &'static str {
        match self {
            Self::Syntax => "lilypond.syntax",
            Self::Empty => "lilypond.empty",
            Self::UnknownCommand => "lilypond.unknown-command",
            Self::Unsupported => "lilypond.unsupported",
            Self::Pitch => "lilypond.pitch",
            Self::Duration => "lilypond.duration",
            Self::BarCheck => "lilypond.bar-check",
            Self::Lyrics => "lilypond.lyrics",
            Self::Include => "lilypond.include",
            Self::ReadLimit => "lilypond.limit",
            Self::TimeOverflow => "score.time-overflow",
            Self::Score => "score.invariant",
            Self::Overfull => "engrave.overfull",
            Self::Font => "engrave.font",
            Self::EngraveLimit => "engrave.limit",
        }
    }
}

/// A structured diagnostic. Adapters place its span in a retained source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// Stable classification.
    pub kind: ErrorKind,
    /// Whether the drawing remains valid.
    pub severity: Severity,
    /// Original notation location, or the default when unplaced.
    pub span: Span,
    /// One concrete explanation.
    pub message: String,
    /// A corrective action.
    pub hint: String,
}

impl Error {
    /// Copies an explanation into an unplaced error; performs no input processing.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            severity: Severity::Error,
            span: Span::default(),
            message: message.into(),
            hint: String::new(),
        }
    }

    /// Attaches a source range, consuming and returning the diagnostic.
    pub fn at(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    /// Attaches a fallback source range only when this diagnostic is still unplaced.
    pub fn or_at(mut self, span: Span) -> Self {
        if self.span.is_unplaced() {
            self.span = span;
        }
        self
    }

    /// Adds a corrective action, consuming and returning the diagnostic.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Marks a diagnostic as recoverable, consuming and returning it.
    pub fn warning(mut self) -> Self {
        self.severity = Severity::Warning;
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.code(), self.message)
    }
}

impl std::error::Error for Error {}
