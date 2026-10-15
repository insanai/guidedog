//! Problems: what went wrong, where, how to fix it, and the exit status a command returns.

use gd_report::{Excerpt, Report};
use std::fmt;
use std::path::Path;

/// A failure with directions: its code (`"eds.date"`), title, the path it concerns, what
/// went wrong, a hint that says what to do, and the exit status a command returns for it.
///
/// Problems about a catalog have codes in the catalog's prefix (`eds.metadata`); problems
/// before a catalog is known use `discussion.*`, and command-line mistakes `cli.usage`.
///
/// The fields live in a boxed [`ProblemData`], so `Result<T, Problem>` stays one pointer
/// wide; read and write them through `Deref` (`p.code`, `p.exit = 2`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Problem(Box<ProblemData>);

/// The fields of a [`Problem`]. Build one with struct-update syntax and convert it with
/// `.into()`, or start from [`Problem::new`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProblemData {
    /// A stable identifier such as `"gds.transition"`.
    pub code: String,
    /// A short sentence-case title such as `"That lifecycle move is not allowed"`.
    pub title: String,
    /// The file or value the problem concerns; may be empty.
    pub path: String,
    /// The 1-based line in `path`, or 0 when the problem has no line.
    pub line: usize,
    /// The 1-based column on `line`, or 0.
    pub column: usize,
    /// The source line to show with a caret, when there is one.
    pub excerpt: Option<Excerpt>,
    /// The plain explanation.
    pub message: String,
    /// How to fix it.
    pub hint: String,
    /// A close name for a misspelt one, shown as "Did you mean ...?".
    pub suggestion: Option<String>,
    /// The exit status of a command that stops on this problem: 2 usage, 3 a budget,
    /// 4 the file system or another program, 1 anything else.
    pub exit: i32,
}

/// A result whose error is a [`Problem`].
pub type Result<T, E = Problem> = std::result::Result<T, E>;

impl From<ProblemData> for Problem {
    fn from(data: ProblemData) -> Problem {
        Problem(Box::new(data))
    }
}

impl std::ops::Deref for Problem {
    type Target = ProblemData;

    fn deref(&self) -> &ProblemData {
        &self.0
    }
}

impl std::ops::DerefMut for Problem {
    fn deref_mut(&mut self) -> &mut ProblemData {
        &mut self.0
    }
}

impl Problem {
    /// Makes a problem with exit status 1; change it with [`Problem::exit`].
    pub fn new(
        code: impl Into<String>,
        title: &str,
        path: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Problem {
        Problem::from(ProblemData {
            code: code.into(),
            title: title.to_string(),
            path: path.into(),
            message: message.into(),
            hint: hint.into(),
            exit: 1,
            ..ProblemData::default()
        })
    }

    /// Replaces the path and returns the problem, for chaining.
    pub fn with_path(mut self, path: impl Into<String>) -> Problem {
        self.path = path.into();
        self
    }

    /// Replaces the title and returns the problem, for chaining.
    pub fn with_title(mut self, title: &str) -> Problem {
        self.title = title.to_string();
        self
    }

    /// Sets the line (with no excerpt) and returns the problem, for chaining.
    pub fn with_line(mut self, line: usize) -> Problem {
        self.line = line;
        self
    }

    /// Sets the exit status and returns the problem, for chaining.
    pub fn exit(mut self, exit: i32) -> Problem {
        self.exit = exit;
        self
    }

    /// Adds a "Did you mean" suggestion when there is one, for chaining.
    pub fn suggest(mut self, suggestion: Option<&str>) -> Problem {
        self.suggestion = suggestion.map(str::to_string);
        self
    }

    /// Positions the problem at 1-based `line` of `source`, the text of `path`, so its
    /// report shows the line with a caret. A line past the end leaves only the number.
    pub fn at_line(mut self, source: &str, line: usize) -> Problem {
        self.line = line;
        let mut report = Report::default();
        gd_report::set_line_column(&mut report, source, line, first_column(source, line));
        if report.line == line {
            self.column = report.column;
            self.excerpt = report.excerpt;
        }
        self
    }

    /// The Elm-style report of the problem, for `gd_report::render_text` or `render_json`.
    pub fn report(&self) -> Report {
        let mut report = Report::error(&self.code, &self.title.to_uppercase(), &self.message);
        report.path = self.path.clone();
        report.line = self.line;
        report.column = self.column;
        report.excerpt = self.excerpt.clone();
        if !self.hint.is_empty() {
            report.hints.push(self.hint.clone());
        }
        if let Some(name) = &self.suggestion {
            report.hints.push(format!("Did you mean `{name}`?"));
        }
        report
    }
}

/// The column of the first non-blank character of `line`, so the caret marks the setting.
fn first_column(source: &str, line: usize) -> usize {
    let text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    text.chars().take_while(|c| c.is_whitespace()).count() + 1
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.path.is_empty() {
            write!(f, "{}", self.path)?;
            if self.line > 0 {
                write!(f, ":{}", self.line)?;
            }
            write!(f, ": ")?;
        }
        write!(f, "{} [{}]: {}", self.title, self.code, self.message)
    }
}

impl std::error::Error for Problem {}

/// `path` as a problem shows it.
pub(crate) fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// A problem of the file system at `path`, in the namespace `ns` (a prefix or
/// `"discussion"`).
pub(crate) fn io_problem(ns: &str, path: &Path, err: &std::io::Error) -> Problem {
    Problem::new(
        format!("{ns}.io"),
        "Cannot access this path",
        shown(path),
        err.to_string(),
        "Check the path and its permissions, then retry.",
    )
    .exit(4)
}

/// The problem of a prefix that is not 2 to 16 lowercase ASCII letters.
pub(crate) fn prefix_problem(path: &str, prefix: &str) -> Problem {
    Problem::new(
        "discussion.prefix",
        "Invalid discussion prefix",
        path,
        format!("{prefix:?} is not a prefix: a prefix is 2 to 16 lowercase ASCII letters."),
        "Pass --prefix with a short lowercase name, such as gds, sds, or eds.",
    )
    .exit(2)
}

/// A command-line mistake: what was received and how to write it.
pub(crate) fn usage(title: &str, value: &str, hint: impl Into<String>) -> Problem {
    Problem::new("cli.usage", title, "", format!("Received {value}."), hint).exit(2)
}
