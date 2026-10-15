//! `Problem`, the failure of a host step (files, publication, Typst, budgets), and the
//! texts the problems share.

use std::fmt;
use std::path::Path;

use gd_report::Report;

/// A host failure: a file that cannot be read, an output that exists, a budget reached,
/// a Typst program that is missing. It carries the exit status the command ends with.
/// Turn it into a report with [`Problem::report`] to render it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Problem {
    /// A stable code such as `"host.io"`.
    pub code: &'static str,
    /// A short upper-case title such as `"CANNOT ACCESS THIS PATH"`.
    pub title: &'static str,
    /// The path the problem concerns, as given; may be empty.
    pub path: String,
    /// The plain explanation.
    pub message: String,
    /// How to fix it.
    pub hint: String,
    /// The process exit status: 2 usage or file type, 3 limits and memory, 4 I/O and
    /// Typst, 130 cancelled.
    pub exit: i32,
}

impl Problem {
    /// Makes a problem from its parts; `path` is copied.
    pub fn new(
        code: &'static str,
        title: &'static str,
        path: &str,
        message: impl Into<String>,
        hint: impl Into<String>,
        exit: i32,
    ) -> Problem {
        Problem {
            code,
            title,
            path: path.to_string(),
            message: message.into(),
            hint: hint.into(),
            exit,
        }
    }

    /// The problem as an error report without a position, for `gd_report::render_text`
    /// and `render_json`. Copies the problem's text.
    pub fn report(&self) -> Report {
        let mut report = Report::error(self.code, self.title, &self.message).hint(&self.hint);
        report.path = crate::display_path(&self.path);
        report
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}: {} Hint: {}", self.title, self.message, self.hint)
        } else {
            write!(f, "{} ({}): {} Hint: {}", self.title, self.path, self.message, self.hint)
        }
    }
}

impl std::error::Error for Problem {}

/// Explains an operating-system error on `path` (`host.io`, exit 4).
pub fn io_problem(path: &Path, error: &std::io::Error) -> Problem {
    Problem::new(
        "host.io",
        "CANNOT ACCESS THIS PATH",
        &path.display().to_string(),
        error.to_string(),
        "Check that the path exists and that you may read or write it.",
        4,
    )
}

/// The problem, with exit status 130, of a command stopped by Ctrl+C.
pub fn interrupted_problem() -> Problem {
    Problem::new(
        "host.cancelled",
        "CANCELLED",
        "",
        "The command was interrupted.",
        "Run it again when you are ready; published files were left unchanged.",
        130,
    )
}

/// Explains an allocation the system refused although the budget allowed it.
pub(crate) fn memory_problem(what: &str, path: &str, bytes: usize) -> Problem {
    Problem::new(
        "host.memory",
        "OUT OF MEMORY",
        path,
        format!(
            "{what} needs {} of memory, which the budget allows but the system could not \
             provide.",
            human_bytes(bytes)
        ),
        "Free memory, build with fewer -j threads, or lower --budget.",
        3,
    )
}

/// Explains an input larger than its per-source limit.
pub(crate) fn too_large(path: &str, limit: usize) -> Problem {
    Problem::new(
        "host.limit.source",
        "SOURCE IS TOO LARGE",
        path,
        format!("The input exceeds the per-source limit of {}.", human_bytes(limit)),
        "Split the document into smaller files joined with include directives.",
        3,
    )
}

/// Names a size in bytes, KiB, or MiB. MiB keep a tenth, rounded up, so a size just
/// over a budget never reads as the budget itself.
pub(crate) fn human_bytes(n: usize) -> String {
    const MIB: usize = 1 << 20;
    if n >= MIB {
        let mut whole = n >> 20;
        let mut tenths = ((n % MIB) * 10).div_ceil(MIB);
        if tenths == 10 {
            whole += 1;
            tenths = 0;
        }
        if tenths == 0 {
            return format!("{whole} MiB");
        }
        return format!("{whole}.{tenths} MiB");
    }
    if n >= 1 << 10 {
        return format!("{} KiB", n >> 10);
    }
    format!("{n} bytes")
}
