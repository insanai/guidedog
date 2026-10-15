//! The errors of a parse: structured (code, title, message, hint, byte span) so the
//! caller can report them in its own diagnostics. The crate never prints.

use std::fmt;
use std::ops::Range;

/// The limit a parse stopped at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stop {
    /// Nested past `Limits::max_depth` (`core.limit.depth`).
    Depth,
    /// Past the stack budget, `Limits::stack_bytes` (`core.limit.stack`).
    Stack,
    /// Backtracking took more than `STEPS_PER_BYTE` steps per byte (`core.limit.work`).
    Work,
}

/// What went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The text is not a declaration (or name, or expression) of the kind asked for, as
    /// Sphinx raises `DefinitionError`. The caller reports its own "invalid signature".
    Invalid,
    /// The parse stopped at a limit: a declaration too deep or too costly to parse is
    /// not an invalid one.
    Limit(Stop),
    /// The declaration parsed but has no id of the newest version (Sphinx's
    /// `NoOldIdError` for version 4).
    NoId,
}

/// An error of this crate, ready to be shown in Elm style by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CppError {
    /// What went wrong.
    pub kind: ErrorKind,
    /// A stable code: `cpp.invalid`, `cpp.no-id`, or the engine's `core.limit.depth`,
    /// `core.limit.stack`, `core.limit.work`.
    pub code: &'static str,
    /// The title rule's text.
    pub title: &'static str,
    /// A plain explanation.
    pub message: String,
    /// How to fix it.
    pub hint: &'static str,
    /// The byte range of the definition the error is about.
    pub span: Range<usize>,
    /// For a limit, the value that was exceeded (levels, bytes, or steps); else 0.
    pub limit: usize,
}

impl CppError {
    pub(crate) fn invalid(what: &str, span: Range<usize>) -> CppError {
        CppError {
            kind: ErrorKind::Invalid,
            code: "cpp.invalid",
            title: "INVALID C++ DECLARATION",
            message: format!("This is not a valid C++ {what}."),
            hint: "Write the declaration as C++ source would, e.g. `int f(double x)`.",
            span,
            limit: 0,
        }
    }

    pub(crate) fn no_id(span: Range<usize>) -> CppError {
        CppError {
            kind: ErrorKind::NoId,
            code: "cpp.no-id",
            title: "C++ DECLARATION WITHOUT AN ID",
            message: "This declaration has no id Sphinx could link to.".to_string(),
            hint: "Simplify the declaration, e.g. drop an explicit instantiation's `template`.",
            span,
            limit: 0,
        }
    }

    pub(crate) fn limit(stop: Stop, limit: usize, span: Range<usize>) -> CppError {
        let (code, title, message, hint) = match stop {
            Stop::Depth => (
                "core.limit.depth",
                "NESTING TOO DEEP",
                format!("This declaration is nested more than {limit} levels deep."),
                "Flatten the nesting, or raise limits.max_depth.",
            ),
            Stop::Stack => (
                "core.limit.stack",
                "NESTING NEEDS TOO MUCH STACK",
                format!(
                    "This declaration is nested more deeply than {limit} bytes of stack can hold."
                ),
                "Flatten the nesting, or raise limits.stack_bytes on a thread with more stack.",
            ),
            Stop::Work => (
                "core.limit.work",
                "WORK LIMIT REACHED",
                format!("Parsing this declaration needed more than {limit} steps."),
                "Simplify the declaration: it nests alternatives the parser retries.",
            ),
        };
        CppError { kind: ErrorKind::Limit(stop), code, title, message, hint, span, limit }
    }

    /// Tells whether the parse stopped at a limit (the caller fails the read with a
    /// `Limit` status rather than warning).
    pub fn is_limit(&self) -> bool {
        matches!(self.kind, ErrorKind::Limit(_))
    }
}

impl fmt::Display for CppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "-- {} -- {}..{}\n\n{}\n\nHint: {}",
            self.title, self.span.start, self.span.end, self.message, self.hint
        )
    }
}

impl std::error::Error for CppError {}
