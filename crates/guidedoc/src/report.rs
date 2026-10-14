//! Diagnostics: static messages with up to two arguments, the bounded report store, and
//! `Fail`, the error every fallible core call returns.

use std::borrow::Cow;
use std::fmt;

use crate::options::Policy;
use crate::types::{CapacityNeed, Region, SourceSpan, Status};

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Severity {
    /// Information only.
    #[default]
    Note,
    /// A problem that does not fail the conversion (unless strict).
    Warning,
    /// A problem that fails the conversion (unless kept by the policy).
    Error,
}

/// A diagnostic argument: an integer, a quoted source span, or text. `{0}` and `{1}`
/// in a message name the arguments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Arg {
    /// No argument.
    #[default]
    None,
    /// An integer.
    Int(i64),
    /// The source text of a span, quoted.
    Span(SourceSpan),
    /// Text: static, or owned when it was made at run time.
    Text(Cow<'static, str>),
}

/// Makes an integer argument.
pub fn arg_int(v: i64) -> Arg {
    Arg::Int(v)
}

/// Makes an argument that quotes the source text of `s`.
pub fn arg_span(s: SourceSpan) -> Arg {
    Arg::Span(s)
}

/// Makes an argument of static text; it copies nothing.
pub fn arg_static(s: &'static str) -> Arg {
    Arg::Text(Cow::Borrowed(s))
}

/// Makes an argument of text that does not live long enough to borrow; it copies `s`.
pub fn arg_text(s: &str) -> Arg {
    Arg::Text(Cow::Owned(s.to_owned()))
}

/// A static description of one problem. Packages declare them as constants:
/// `const UNRESOLVED: Message = Message { code: "rst.reference.unresolved", .. }`.
/// Every message has a hint saying how to fix the problem (Elm style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Message {
    /// Stable identifier ("core.capacity").
    pub code: &'static str,
    /// How serious it is.
    pub severity: Severity,
    /// The status it fails with, when it is an error.
    pub status: Status,
    /// Upper-case title ("NOT ENOUGH STORAGE").
    pub title: &'static str,
    /// Explanation; "{0}" and "{1}" name the arguments.
    pub message: &'static str,
    /// How to fix it.
    pub hint: &'static str,
}

/// One reported problem: a message, where it happened, and its arguments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagnostic {
    /// Stable identifier.
    pub code: &'static str,
    /// How serious it is.
    pub severity: Severity,
    /// The status it fails with.
    pub status: Status,
    /// Upper-case title.
    pub title: &'static str,
    /// Explanation template.
    pub message: &'static str,
    /// How to fix it.
    pub hint: &'static str,
    /// Where it happened (zero when nowhere in particular).
    pub span: SourceSpan,
    /// The `{0}` and `{1}` arguments.
    pub args: [Arg; 2],
}

/// Builds a diagnostic from message `m` at `span` with up to two arguments.
pub fn diagnostic(m: Message, span: SourceSpan, a: Arg, b: Arg) -> Diagnostic {
    Diagnostic {
        code: m.code,
        severity: m.severity,
        status: m.status,
        title: m.title,
        message: m.message,
        hint: m.hint,
        span,
        args: [a, b],
    }
}

impl Diagnostic {
    /// Returns the static message the diagnostic was made from.
    pub fn message_of(&self) -> Message {
        Message {
            code: self.code,
            severity: self.severity,
            status: self.status,
            title: self.title,
            message: self.message,
            hint: self.hint,
        }
    }

    /// Writes `template` (the message or the hint) with its arguments substituted into
    /// `out`; `quote` gives the source text of a span argument.
    pub fn format_into(
        &self,
        template: &str,
        quote: &dyn Fn(SourceSpan) -> String,
        out: &mut String,
    ) {
        let mut rest = template;
        while let Some(at) = rest.find('{') {
            out.push_str(&rest[..at]);
            let tail = &rest[at..];
            let index = match tail.as_bytes().get(1..3) {
                Some(b"0}") => Some(0),
                Some(b"1}") => Some(1),
                _ => None,
            };
            match index {
                Some(i) => {
                    self.write_arg(i, quote, out);
                    rest = &tail[3..];
                }
                None => {
                    out.push('{');
                    rest = &tail[1..];
                }
            }
        }
        out.push_str(rest);
    }

    fn write_arg(&self, i: usize, quote: &dyn Fn(SourceSpan) -> String, out: &mut String) {
        match &self.args[i] {
            Arg::None => {}
            Arg::Int(v) => out.push_str(&v.to_string()),
            Arg::Span(s) => {
                out.push('`');
                out.push_str(&quote(*s));
                out.push('`');
            }
            Arg::Text(t) => out.push_str(t),
        }
    }

    /// Returns the message with its arguments; spans are written as `source:start..end`.
    pub fn formatted_message(&self) -> String {
        let mut out = String::new();
        self.format_into(self.message, &span_placeholder, &mut out);
        out
    }

    /// Returns the hint with its arguments substituted.
    pub fn formatted_hint(&self) -> String {
        let mut out = String::new();
        self.format_into(self.hint, &span_placeholder, &mut out);
        out
    }
}

fn span_placeholder(s: SourceSpan) -> String {
    format!("{}:{}..{}", s.source.0, s.start, s.end)
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} Hint: {}", self.title, self.formatted_message(), self.formatted_hint())
    }
}

/// The failure of a core call: a status, the capacity it lacked (for `Capacity`), and
/// the diagnostic that explains it. Readers and passes propagate it with `?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fail {
    /// Why it failed.
    pub status: Status,
    /// On `Capacity`, the region to enlarge.
    pub need: CapacityNeed,
    /// The primary diagnostic (boxed to keep `Result`s small).
    pub diagnostic: Box<Diagnostic>,
}

impl Fail {
    /// Makes a failure with `status` and no diagnostic.
    pub fn new(status: Status) -> Fail {
        Fail { status, need: CapacityNeed::default(), diagnostic: Box::default() }
    }

    /// Makes a failure explained by `d`, with its status.
    pub fn from_diagnostic(d: Diagnostic) -> Fail {
        Fail { status: d.status, need: CapacityNeed::default(), diagnostic: Box::new(d) }
    }

    /// Makes a `Capacity` failure for `region`, lacking `minimum` (CORE_CAPACITY).
    pub fn capacity(region: Region, minimum: u64, exact: bool) -> Fail {
        let d = diagnostic(
            CORE_CAPACITY,
            SourceSpan::default(),
            arg_static(region.name()),
            arg_int(i64::try_from(minimum).unwrap_or(i64::MAX)),
        );
        Fail {
            status: Status::Capacity,
            need: CapacityNeed { region, minimum, exact },
            diagnostic: Box::new(d),
        }
    }

    /// Returns the diagnostic's code ("core.capacity"); "" when it has none.
    pub fn code(&self) -> &'static str {
        self.diagnostic.code
    }
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.diagnostic.code.is_empty() {
            write!(f, "conversion failed: {:?}", self.status)
        } else {
            fmt::Display::fmt(&self.diagnostic, f)
        }
    }
}

impl std::error::Error for Fail {}

/// The bounded report store of a workspace: every diagnostic in order up to
/// `Limits::max_reports`, the first error kept by value even when the store is full,
/// and counts of errors and warnings.
#[derive(Debug, Clone, Default)]
pub struct Reports {
    items: Vec<Diagnostic>,
    max: usize,
    suppressed: usize,
    truncated: bool,
    muted: bool,
    primary: Option<Diagnostic>,
    errors: usize,
    warnings: usize,
    fatal: Status,
}

impl Reports {
    /// Makes an empty store holding at most `max` diagnostics.
    pub fn new(max: usize) -> Reports {
        Reports { max, ..Reports::default() }
    }

    /// Empties the store, keeping its storage, and sets its bound.
    pub fn clear(&mut self, max: usize) {
        self.items.clear();
        self.max = max;
        self.suppressed = 0;
        self.truncated = false;
        self.muted = false;
        self.primary = None;
        self.errors = 0;
        self.warnings = 0;
        self.fatal = Status::Ok;
    }

    /// Records `d`, unless muted. The first error is kept as the primary even when the
    /// store is full; a full store counts the diagnostic as suppressed.
    pub fn report(&mut self, d: Diagnostic) {
        if self.muted {
            return;
        }
        match d.severity {
            Severity::Error => {
                self.errors += 1;
                if self.fatal == Status::Ok && !content_error(d.status) {
                    self.fatal = d.status;
                }
                if self.primary.is_none() {
                    self.primary = Some(d.clone());
                }
            }
            Severity::Warning => self.warnings += 1,
            Severity::Note => {}
        }
        if self.items.len() < self.max && self.items.try_reserve(1).is_ok() {
            self.items.push(d);
        } else {
            self.suppressed += 1;
            self.truncated = true;
        }
    }

    /// Reports message `m` at `span` with its arguments, as `report` does.
    pub fn message(&mut self, m: Message, span: SourceSpan, a: Arg, b: Arg) {
        self.report(diagnostic(m, span, a, b));
    }

    /// Returns every stored diagnostic, in order.
    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    /// Returns the first error, if any.
    pub fn primary(&self) -> Option<&Diagnostic> {
        self.primary.as_ref()
    }

    /// Returns the primary error, or the first warning when there is none.
    pub fn first_problem(&self) -> Option<&Diagnostic> {
        self.primary
            .as_ref()
            .or_else(|| self.items.iter().find(|d| d.severity == Severity::Warning))
    }

    /// Returns the number of errors reported (stored or not).
    pub fn error_count(&self) -> usize {
        self.errors
    }

    /// Returns the number of warnings reported (stored or not).
    pub fn warning_count(&self) -> usize {
        self.warnings
    }

    /// Returns how many diagnostics a full store dropped.
    pub fn suppressed_count(&self) -> usize {
        self.suppressed
    }

    /// Tells whether the store dropped diagnostics.
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// Tells whether reporting is muted (a trial parse).
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Mutes or unmutes reporting; returns the previous setting, to restore.
    pub fn set_muted(&mut self, muted: bool) -> bool {
        std::mem::replace(&mut self.muted, muted)
    }

    /// Returns the first error that is not a content error (`Ok` when none).
    pub fn fatal(&self) -> Status {
        self.fatal
    }
}

/// Tells whether an error's status describes the content (an unknown directive, a bad
/// option, an unresolved reference, content refused by policy) rather than a tree that
/// cannot be used. `Policy::keep_documents_with_errors` keeps documents whose errors
/// are all content errors.
pub fn content_error(status: Status) -> bool {
    matches!(status, Status::InvalidInput | Status::Unsupported | Status::Policy)
}

/// Returns `m` as a renderer reports it: a content error becomes a warning when the
/// policy keeps documents with errors, so the page is still written.
pub fn kept_message(policy: &Policy, m: Message) -> Message {
    let mut m = m;
    if policy.keep_documents_with_errors
        && m.severity == Severity::Error
        && content_error(m.status)
    {
        m.severity = Severity::Warning;
    }
    m
}

/// A region ran out of memory; `{0}` names it and `{1}` the shortfall.
pub const CORE_CAPACITY: Message = Message {
    code: "core.capacity",
    severity: Severity::Error,
    status: Status::Capacity,
    title: "NOT ENOUGH STORAGE",
    message: "The {0} region is full; at least {1} more is needed.",
    hint: "Retry with a larger {0} limit, within your memory budget.",
};

/// More than `Limits::max_work` steps.
pub const CORE_WORK: Message = Message {
    code: "core.limit.work",
    severity: Severity::Error,
    status: Status::Limit,
    title: "WORK LIMIT REACHED",
    message: "Processing this input needed more than {0} steps.",
    hint: "Raise limits.max_work, or simplify deeply repeated markup.",
};

/// Nesting past `Limits::max_depth`.
pub const CORE_DEPTH: Message = Message {
    code: "core.limit.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "NESTING TOO DEEP",
    message: "Content is nested more than {0} levels deep.",
    hint: "Flatten the nesting, or raise limits.max_depth.",
};

/// Recursion past `Limits::stack_bytes`.
pub const CORE_STACK: Message = Message {
    code: "core.limit.stack",
    severity: Severity::Error,
    status: Status::Limit,
    title: "NESTING NEEDS TOO MUCH STACK",
    message: "Content is nested more deeply than {0} bytes of stack can hold.",
    hint: "Flatten the nesting, or raise limits.stack_bytes on a thread with more stack.",
};

/// More than `Limits::max_nodes` nodes.
pub const CORE_NODES: Message = Message {
    code: "core.limit.nodes",
    severity: Severity::Error,
    status: Status::Limit,
    title: "TOO MANY NODES",
    message: "The document needs more than {0} nodes.",
    hint: "Split the document, or raise limits.max_nodes.",
};

/// A renderer routed a node to a handler that does not write it.
pub const CORE_UNHANDLED_KIND: Message = Message {
    code: "core.render.kind",
    severity: Severity::Error,
    status: Status::Internal,
    title: "NODE KIND NOT RENDERED",
    message: "The renderer routed a {0} node to a handler that does not write it.",
    hint: "Report this renderer bug to the Guidedog developers with the input that caused it; \
     the output was discarded.",
};

/// A bytes source is not UTF-8.
pub const CORE_UTF8: Message = Message {
    code: "core.source.utf8",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "SOURCE IS NOT UTF-8",
    message: "The byte at this position does not start a valid UTF-8 character.",
    hint: "Convert the file to UTF-8, for example with iconv, and retry.",
};

/// No document to use.
pub(crate) const CORE_NO_DOCUMENT: Message = Message {
    code: "core.document.none",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "NO DOCUMENT TO USE",
    message: "The workspace holds no document: nothing was read, or the last read failed.",
    hint: "Call read_document (or load_object) and check its result before this stage.",
};

/// Unknown input format.
pub(crate) const CORE_UNKNOWN_READER: Message = Message {
    code: "core.reader.unknown",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "UNKNOWN INPUT FORMAT",
    message: "No reader is registered for {0}.",
    hint: "Use rst, sphinx, commonmark, or myst, or register a reader with this ID.",
};

/// Unknown output format.
pub(crate) const CORE_UNKNOWN_RENDERER: Message = Message {
    code: "core.renderer.unknown",
    severity: Severity::Error,
    status: Status::Unsupported,
    title: "UNKNOWN OUTPUT FORMAT",
    message: "No renderer is registered for {0}.",
    hint: "Use html, typst, or text, or register a renderer with this ID.",
};

/// Reader settings of another type.
pub(crate) const CORE_SETTINGS_TYPE: Message = Message {
    code: "core.read.settings_type",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "READER SETTINGS OF ANOTHER TYPE",
    message: "The read options carry settings of type {0}; this reader takes settings of \
              type {1}.",
    hint: "Set ReadOptions::extension with read_extension(&settings) of this reader's own \
     settings type, or leave it empty for the defaults.",
};

/// Reader takes no settings.
pub(crate) const CORE_SETTINGS_NONE: Message = Message {
    code: "core.read.settings_none",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "READER TAKES NO SETTINGS",
    message: "The read options carry settings of type {0}; this reader takes none.",
    hint: "Leave ReadOptions::extension empty, or choose the reader the settings are for \
     (myst for commonmark settings).",
};

/// Conversion cancelled.
pub(crate) const CORE_CANCELLED: Message = Message {
    code: "core.cancelled",
    severity: Severity::Error,
    status: Status::Cancelled,
    title: "CONVERSION CANCELLED",
    message: "The caller's cancellation probe stopped the conversion.",
    hint: "Run the conversion again, with a cancellation probe that lets it finish.",
};

/// Invalid document tree.
pub(crate) const CORE_INVALID: Message = Message {
    code: "core.document.invalid",
    severity: Severity::Error,
    status: Status::Internal,
    title: "INVALID DOCUMENT TREE",
    message: "Node {0} breaks the document invariant: {1}.",
    hint: "This is a reader or pass bug; report it with the input that caused it.",
};

/// Illegal nesting.
pub(crate) const CORE_NESTING: Message = Message {
    code: "core.document.nesting",
    severity: Severity::Error,
    status: Status::Internal,
    title: "ILLEGAL NESTING",
    message: "A {1} cannot be placed inside a {0}.",
    hint: "This is a reader or pass bug; report it with the input that caused it.",
};

/// Warnings refused by strict mode.
pub(crate) const CORE_STRICT: Message = Message {
    code: "core.policy.strict",
    severity: Severity::Error,
    status: Status::Policy,
    title: "WARNINGS REFUSED BY STRICT MODE",
    message: "The conversion produced {0} warning(s) or error(s) and strict mode is on.",
    hint: "Fix the warnings reported above, or set policy.strict to false (guidedog convert \
     without --strict).",
};

/// Renderer wrote invalid utf-8.
pub(crate) const CORE_OUTPUT_UTF8: Message = Message {
    code: "core.render.utf8",
    severity: Severity::Error,
    status: Status::Internal,
    title: "RENDERER WROTE INVALID UTF-8",
    message: "The renderer's output is not valid UTF-8 at byte {0}.",
    hint: "Report this renderer bug to the Guidedog developers with the input that caused it; \
     the output was discarded.",
};

/// Transform failed.
pub(crate) const CORE_PASS: Message = Message {
    code: "core.pass.failed",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "TRANSFORM FAILED",
    message: "The pass {0} reported a failure.",
    hint: "Check the pass's own diagnostics.",
};

/// Invalid format registry.
pub(crate) const REGISTRY_INVALID: Message = Message {
    code: "core.registry.invalid",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "INVALID FORMAT REGISTRY",
    message: "The adapter {0} is {1}.",
    hint: "Give every adapter a unique ID and a version.",
};

/// Source too large.
pub(crate) const CORE_SOURCE_SIZE: Message = Message {
    code: "core.source.size",
    severity: Severity::Error,
    status: Status::Limit,
    title: "SOURCE TOO LARGE",
    message: "The source holds {0} bytes; a source holds less than 4 GiB.",
    hint: "Split the source into several files.",
};

#[cfg(test)]
mod tests {
    use super::*;

    const TOY: Message = Message {
        code: "toy",
        severity: Severity::Error,
        status: Status::Unsupported,
        title: "TOY",
        message: "A {0} and {1} and {2}.",
        hint: "Fix {0}.",
    };

    #[test]
    fn messages_substitute_arguments() {
        let d = diagnostic(TOY, SourceSpan::default(), arg_static("x"), arg_int(3));
        assert_eq!(d.formatted_message(), "A x and 3 and {2}.");
        assert_eq!(d.formatted_hint(), "Fix x.");
    }

    #[test]
    fn reports_are_bounded_and_keep_the_primary() {
        let mut r = Reports::new(1);
        r.message(CORE_DEPTH, SourceSpan::default(), arg_int(1), Arg::None);
        r.message(TOY, SourceSpan::default(), Arg::None, Arg::None);
        assert_eq!(r.items().len(), 1);
        assert_eq!(r.suppressed_count(), 1);
        assert!(r.is_truncated());
        assert_eq!(r.primary().map(|d| d.code), Some("core.limit.depth"));
        assert_eq!(r.fatal(), Status::Limit);
        assert_eq!(r.error_count(), 2);
    }
}
