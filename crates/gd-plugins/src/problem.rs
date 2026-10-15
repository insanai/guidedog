//! `Problem`: a failure with directions, and the constructors the SDK shares.

use std::fmt;

use gd_report::Report;

/// Exit status of a publication the host accepted but has not confirmed live yet.
pub const EXIT_ACCEPTED: i32 = 6;
/// Exit status of an outcome that is not known; inspect it with `status` or `recover`.
pub const EXIT_UNKNOWN: i32 = 7;

/// A failure: a stable code (`"pages.credential"`), a title, the path or name it
/// concerns, what went wrong, a hint that says what to do, and the exit status a
/// command returns for it (always nonzero for a real problem).
///
/// Exit statuses beyond 0 are 1 (failed, nothing published), 2 (usage or
/// configuration), 3 (memory or a limit), 4 (files or processes), [`EXIT_ACCEPTED`],
/// [`EXIT_UNKNOWN`] and 130 (cancelled).
///
/// The fields live in a [`ProblemData`] box, so `Result<T, Problem>` stays one pointer
/// wide; read and write them directly (`p.code`, `p.exit = 2`) through `Deref`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Problem(Box<ProblemData>);

/// The fields of a [`Problem`]. Build one with struct-update syntax and convert it with
/// `.into()`, or start from [`Problem::new`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProblemData {
    /// A stable identifier such as `"plugins.timeout"`.
    pub code: String,
    /// A short sentence naming what failed.
    pub title: String,
    /// The path or name the problem concerns; may be empty.
    pub path: String,
    /// The plain explanation.
    pub message: String,
    /// What to do about it.
    pub hint: String,
    /// The exit status a command returns for this problem.
    pub exit: i32,
}

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
    /// Starts a problem with exit status 1 and no path or hint; chain [`Problem::hint`],
    /// [`Problem::path`] and [`Problem::exit`] to complete it.
    pub fn new(code: &str, title: &str, message: &str) -> Problem {
        Problem::from(ProblemData {
            code: code.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            exit: 1,
            ..ProblemData::default()
        })
    }

    /// Sets the hint and returns the problem, for chaining.
    pub fn hint(mut self, hint: &str) -> Problem {
        self.hint = hint.to_string();
        self
    }

    /// Sets the path the problem concerns and returns it, for chaining.
    pub fn path(mut self, path: &str) -> Problem {
        self.path = path.to_string();
        self
    }

    /// Sets the exit status and returns the problem, for chaining.
    pub fn exit(mut self, exit: i32) -> Problem {
        self.exit = exit;
        self
    }

    /// `plugins.memory` (exit 3): memory, or a buffer driven by input size, was refused.
    pub fn memory() -> Problem {
        Problem::new(
            "plugins.memory",
            "Out of memory",
            "The system refused memory, so the work stopped before anything it had not \
             prepared whole was used.",
        )
        .hint("Close other programs to free memory, then run the same command again.")
        .exit(3)
    }

    /// Converts the problem into an Elm-style report for `gd_report::render_text`.
    pub fn report(&self) -> Report {
        let mut report = Report::error(&self.code, &self.title.to_uppercase(), &self.message);
        report.path = self.path.clone();
        if !self.hint.is_empty() {
            report = report.hint(&self.hint);
        }
        report
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.title, self.message)?;
        if !self.path.is_empty() {
            write!(f, " ({})", self.path)?;
        }
        if !self.hint.is_empty() {
            write!(f, " Hint: {}", self.hint)?;
        }
        Ok(())
    }
}

impl std::error::Error for Problem {}

/// `plugins.contract` (exit 2): a plugin or its caller broke the interface.
pub(crate) fn contract(id: &str, hint: &str) -> Problem {
    Problem::new(
        "plugins.contract",
        "The plugin does not meet its interface",
        &format!("Plugin {id:?} cannot be used with this operation."),
    )
    .hint(hint)
    .path(id)
    .exit(2)
}

/// `plugins.protocol` (exit 7): the other program did not follow the protocol.
pub(crate) fn protocol(message: &str) -> Problem {
    Problem::new("plugins.protocol", "The plugin program did not follow the protocol", message)
        .hint("Install a plugin for this Guidedog, or report the response to its authors.")
        .exit(EXIT_UNKNOWN)
}

/// `plugins.io` (exit 4): a path could not be read or written.
pub(crate) fn io(path: &str, err: &std::io::Error) -> Problem {
    Problem::new("plugins.io", "Cannot access this path", &err.to_string())
        .hint("Check that the path exists and that you may read and write it, then retry.")
        .path(path)
        .exit(4)
}

/// `plugins.cancelled` (exit 130) for a build plugin.
pub(crate) fn build_cancelled(id: &str) -> Problem {
    Problem::new(
        "plugins.cancelled",
        "The plugin build was cancelled",
        "The host stopped this build before publishing its output.",
    )
    .hint("Run guidedog build again when you are ready to continue.")
    .path(id)
    .exit(130)
}

/// Holds a problem a plugin returned to the contract: it needs a title, message, hint
/// and a nonzero exit status. The message names the plugin and the stage, the path
/// defaults to `path`, and the publication statuses (6, 7) become 4: a build failure is
/// never an uncertain remote outcome.
pub(crate) fn from_plugin(p: Problem, id: &str, stage: &str, path: &str) -> Problem {
    if p.title.is_empty() || p.message.is_empty() || p.hint.is_empty() || p.exit == 0 {
        return contract(
            id,
            "Return a Problem with a title, explanation, correction hint, and nonzero exit \
             status.",
        );
    }
    let mut out = p;
    if out.path.is_empty() {
        out.path = path.to_string();
    }
    out.message = format!("Plugin {id} stopped during {stage}. {}", out.message);
    if out.exit == EXIT_UNKNOWN || out.exit == EXIT_ACCEPTED {
        out.exit = 4;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_problems_need_directions_and_are_never_uncertain() {
        let bare = Problem::from(ProblemData { exit: 2, ..ProblemData::default() });
        assert_eq!(from_plugin(bare, "badge", "load", "").code, "plugins.contract");
        let p = Problem::new("badge.x", "Bad", "It broke.").hint("Fix it.").exit(EXIT_UNKNOWN);
        let out = from_plugin(p, "badge", "source", "index.rst");
        assert_eq!(out.exit, 4);
        assert_eq!(out.path, "index.rst");
        assert!(out.message.starts_with("Plugin badge stopped during source."));
    }

    #[test]
    fn problems_become_reports_with_hints() {
        let report = protocol("A line is too long.").report();
        assert_eq!(report.code, "plugins.protocol");
        assert_eq!(report.hints.len(), 1);
        assert!(report.title.chars().all(|c| !c.is_lowercase()));
    }
}
