//! Errors: what went wrong, where, and how to fix it.

use std::fmt;

use crate::text::{self, WrapOptions};

/// What kind of failure an [`Error`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorKind {
    /// The template does not parse (Jinja's TemplateSyntaxError).
    #[default]
    Syntax,
    /// An undefined value was used (UndefinedError).
    Undefined,
    /// An operation got the wrong kind of value (TypeError).
    Type,
    /// A template could not be loaded (TemplateNotFound).
    NotFound,
    /// A recursion, nesting, iteration, output or stack limit was reached.
    Limit,
    /// A value too large to count or to allocate was asked for (MemoryError).
    Memory,
    /// Any other failure while rendering.
    Runtime,
}

impl ErrorKind {
    /// The report title: "SYNTAX ERROR", "UNDEFINED VALUE", ...
    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "SYNTAX ERROR",
            ErrorKind::Undefined => "UNDEFINED VALUE",
            ErrorKind::Type => "TYPE ERROR",
            ErrorKind::NotFound => "TEMPLATE NOT FOUND",
            ErrorKind::Limit => "LIMIT REACHED",
            ErrorKind::Memory => "OUT OF MEMORY",
            ErrorKind::Runtime => "RENDER ERROR",
        }
    }
}

/// A failure to parse or render. `excerpt` is the template line the error points at;
/// `hint`, when not empty, suggests a fix. `Display` renders an Elm-style report:
///
/// ```text
/// -- UNDEFINED VALUE -------------------------------------------------- page.html:3
///
/// 'usr' is undefined
///
/// 3| <h1>{{ usr.name }}</h1>
///
/// Hint: Did you mean 'user'?
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Error {
    /// What kind of failure this is.
    pub kind: ErrorKind,
    /// The template's name; empty when there is none.
    pub template: String,
    /// The 1-based line, or 0 when unknown.
    pub line: usize,
    /// The plain explanation.
    pub message: String,
    /// The text of the line the error points at.
    pub excerpt: String,
    /// How to fix it; empty when there is no suggestion.
    pub hint: String,
}

/// MEMORY_HINT_RENDER is the hint of a Memory error raised while rendering.
pub(crate) const MEMORY_HINT_RENDER: &str = "Build less at once here: a smaller repetition, \
    padding, join, or loop; or give the render more memory.";

/// MEMORY_MESSAGE is the message of a Memory error.
pub(crate) const MEMORY_MESSAGE: &str = "This template needs more memory than it may use.";

impl Error {
    /// Makes an error of kind with message; the render that returns it fills in where.
    pub fn new(kind: ErrorKind, line: usize, message: impl Into<String>) -> Error {
        Error { kind, line, message: message.into(), ..Error::default() }
    }

    /// Returns the error with `hint` attached, unless it already has one.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Error {
        if self.hint.is_empty() {
            self.hint = hint.into();
        }
        self
    }
}

/// Attaches hints to boxed errors, as the renderer passes them around.
pub(crate) trait Hinted {
    /// The error with hint attached, unless it already has one.
    fn hinted(self, hint: impl Into<String>) -> Self;
}

impl Hinted for Box<Error> {
    fn hinted(mut self, hint: impl Into<String>) -> Self {
        if self.hint.is_empty() {
            self.hint = hint.into();
        }
        self
    }
}

impl From<Box<Error>> for Error {
    fn from(err: Box<Error>) -> Error {
        *err
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.kind.title();
        let mut place = if self.template.is_empty() {
            "<template>".to_string()
        } else {
            self.template.clone()
        };
        if self.line > 0 {
            place = format!("{place}:{}", self.line);
        }
        let dashes = (80usize.saturating_sub(title.len() + place.len() + 5)).max(3);
        write!(
            f,
            "-- {title} {} {place}\n\n{}\n",
            "-".repeat(dashes),
            wrap_report(&self.message)
        )?;
        if !self.excerpt.is_empty() {
            write!(f, "\n{}| {}\n", self.line, self.excerpt)?;
        }
        if !self.hint.is_empty() {
            write!(f, "\n{}\n", wrap_report(&format!("Hint: {}", self.hint)))?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// Wraps prose at 80 columns, keeping each input line separate.
fn wrap_report(message: &str) -> String {
    let options = WrapOptions { width: 80, break_long_words: false, break_on_hyphens: false };
    let mut out = String::with_capacity(message.len() + 8);
    for (i, line) in text::splitlines(message, false).iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for (j, piece) in text::wrap(line, &options).iter().enumerate() {
            if j > 0 {
                out.push('\n');
            }
            out.push_str(piece);
        }
    }
    out
}

/// Returns line number `line` (1-based) of source, without its newline.
pub(crate) fn source_line(source: &str, line: usize) -> &str {
    if line == 0 {
        return "";
    }
    source.lines().nth(line - 1).map_or("", |l| l.trim_end_matches('\r'))
}

/// The Levenshtein distance between two short names.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() > 64 || b.len() > 64 {
        return 99;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Picks the candidate nearest to name, if it is close enough to be a typo.
pub(crate) fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> &'a str {
    let limit = if name.len() <= 3 { 1 } else { 2 };
    let (mut best, mut best_d) = ("", 3);
    for c in candidates {
        if c == name {
            continue;
        }
        let d = edit_distance(name, c);
        if d <= limit && d < best_d {
            best = c;
            best_d = d;
        }
    }
    best
}
