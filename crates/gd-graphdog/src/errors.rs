//! Errors: kinds, stock messages and hints, and how Graphviz's log becomes an [`Error`].

use std::fmt;

use crate::format::{column_of, line_count, locate};
use crate::{Engine, Format, log};

/// What went wrong in [`crate::Context::render`] or [`crate::Context::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The DOT source does not parse.
    Syntax,
    /// An HTML-like label (`label=<...>`) is malformed.
    HtmlLabel,
    /// The source holds no graph.
    EmptyInput,
    /// This Graphviz has no plugin for the layout engine.
    MissingEngine,
    /// This Graphviz has no plugin for the output format.
    MissingFormat,
    /// The layout engine reported an error.
    Layout,
    /// The renderer reported an error.
    Render,
    /// Graphviz, or the caller's reserve, refused memory.
    OutOfMemory,
    /// The caller refused a file the graph names (`Options::files`).
    FileRefused,
    /// Graphviz could not create a context.
    NotInitialized,
}

/// Says what went wrong and how to fix it.
///
/// `line` and `column` are 1-based positions in the source (the column counts
/// characters), or 0 when Graphviz does not say, as in [`gd_report::Report`]. `message`
/// is one sentence, in Graphviz's own words when it gave any; `hint` is advice for the
/// author. [`Error::report`] places it in the source; `Display` renders it without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The kind of problem.
    pub kind: ErrorKind,
    /// 1-based line, or 0 when unknown.
    pub line: usize,
    /// 1-based character column, or 0 when unknown.
    pub column: usize,
    /// The text Graphviz stopped at (underlined in a report); empty when it named none.
    pub near: String,
    /// What went wrong.
    pub message: String,
    /// How to fix it.
    pub hint: String,
}

impl Error {
    /// An error of `kind` with the stock message and hint.
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self::with(kind, kind.message())
    }

    /// An error of `kind` with `message` and the stock hint.
    pub(crate) fn with(kind: ErrorKind, message: impl Into<String>) -> Self {
        let hint = kind.hint().to_string();
        Self { kind, line: 0, column: 0, near: String::new(), message: message.into(), hint }
    }

    /// The report of this error placed in `source`, the text given to
    /// [`crate::Context::render`], which `name` labels (`"diagram.dot"`, `"<stdin>"`):
    /// the line with carets under [`Error::near`], or under the whole line when the
    /// column is unknown. Copies what it needs; never fails.
    pub fn report(&self, source: &str, name: &str) -> gd_report::Report {
        let mut report = self.unplaced();
        report.path = name.to_string();
        if let Some((start, end)) = crate::format::span(source, self.line, self.column, &self.near)
        {
            gd_report::set_span(&mut report, source, start, end);
        }
        report
    }

    fn unplaced(&self) -> gd_report::Report {
        gd_report::Report::error(self.kind.code(), self.kind.title(), &self.message)
            .hint(&self.hint)
    }
}

/// Renders the error without a source excerpt, 80 columns wide.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = gd_report::render_text(&self.unplaced(), &gd_report::Style::default());
        f.write_str(text.trim_end())
    }
}

impl std::error::Error for Error {}

impl ErrorKind {
    /// A short upper-case title for the kind of error, such as `"DOT SYNTAX ERROR"`.
    pub fn title(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "DOT SYNTAX ERROR",
            ErrorKind::HtmlLabel => "BAD HTML LABEL",
            ErrorKind::EmptyInput => "EMPTY INPUT",
            ErrorKind::MissingEngine => "MISSING LAYOUT ENGINE",
            ErrorKind::MissingFormat => "MISSING OUTPUT FORMAT",
            ErrorKind::Layout => "LAYOUT FAILED",
            ErrorKind::Render => "RENDER FAILED",
            ErrorKind::OutOfMemory => "OUT OF MEMORY",
            ErrorKind::FileRefused => "FILE REFUSED",
            ErrorKind::NotInitialized => "NO CONTEXT",
        }
    }

    /// A short, stable code for the kind, such as `"graphdog.syntax"`.
    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "graphdog.syntax",
            ErrorKind::HtmlLabel => "graphdog.html-label",
            ErrorKind::EmptyInput => "graphdog.empty-input",
            ErrorKind::MissingEngine => "graphdog.missing-engine",
            ErrorKind::MissingFormat => "graphdog.missing-format",
            ErrorKind::Layout => "graphdog.layout",
            ErrorKind::Render => "graphdog.render",
            ErrorKind::OutOfMemory => "graphdog.out-of-memory",
            ErrorKind::FileRefused => "graphdog.file-refused",
            ErrorKind::NotInitialized => "graphdog.not-initialized",
        }
    }

    fn message(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "The DOT source does not parse.",
            ErrorKind::HtmlLabel => "An HTML-like label is malformed.",
            ErrorKind::EmptyInput => "The input holds no graph.",
            ErrorKind::MissingEngine => "This Graphviz has no plugin for the layout engine.",
            ErrorKind::MissingFormat => "This Graphviz has no plugin for the output format.",
            ErrorKind::Layout => "The layout engine failed.",
            ErrorKind::Render => "Rendering failed.",
            ErrorKind::OutOfMemory => "Graphviz ran out of memory.",
            ErrorKind::FileRefused => "The graph names a file that may not be read.",
            ErrorKind::NotInitialized => "Graphviz could not create a context.",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            ErrorKind::Syntax => {
                "Check this line for a missing `;`, `]`, `}` or `\"`. Attribute lists look \
                 like `[shape=box, color=red]`, and names with spaces or punctuation need \
                 double quotes."
            }
            ErrorKind::HtmlLabel => {
                "HTML-like labels must be well-formed XML: close every tag you open, in order, \
                 and write & < > as &amp; &lt; &gt; inside text."
            }
            ErrorKind::EmptyInput => "Write a graph, for example `digraph { a -> b }`.",
            ErrorKind::MissingEngine if crate::STATIC => {
                "This program has Graphviz built in with the dot, neato, fdp, sfdp, circo, \
                 twopi, osage and patchwork layouts; use one of them."
            }
            ErrorKind::MissingEngine => {
                "Register the installed plugins with `sudo dot -c`, or install the package \
                 that holds the layout plugins (graphviz on Homebrew, Debian and Ubuntu)."
            }
            ErrorKind::MissingFormat if crate::STATIC => {
                "This program has Graphviz built in without PNG support; use SVG or DOT \
                 output."
            }
            ErrorKind::MissingFormat => {
                "Register the installed plugins with `sudo dot -c`. PNG output needs the \
                 cairo or gd plugin; SVG always works."
            }
            ErrorKind::Layout => {
                "Try another engine, or simplify the graph to find the part that the engine \
                 rejects."
            }
            ErrorKind::Render => "Try SVG output, which every Graphviz build supports.",
            ErrorKind::OutOfMemory => "The graph may be too large; try splitting it.",
            ErrorKind::FileRefused => {
                "Name only files the program allows, or leave the attribute out."
            }
            ErrorKind::NotInitialized => {
                "This usually means the system is out of memory, or Graphviz is not \
                 installed correctly; check that `dot -V` works."
            }
        }
    }
}

/// Explains why agmemread returned no graph.
pub(crate) fn parse_failure(source: &str) -> Error {
    let Some(message) = log::first_error() else {
        return Error::new(ErrorKind::EmptyInput);
    };
    let mut err = Error::with(ErrorKind::Syntax, message.as_str());
    err.line = number_after(&message, "in line ");
    if let Some(near) = quoted_after(&message, "near '") {
        err.column = column_of(source, err.line, near);
        err.message = format!("Graphviz stopped at `{near}`.");
        let hint = syntax_hint(source, near);
        if !hint.is_empty() {
            err.hint = hint.to_string();
        }
        err.near = near.to_string();
        return err;
    }
    if message.contains("quoted string") {
        err.message = "A quoted string never ends.".to_string();
        err.hint = "Add the closing `\"`. A string may span lines, so the missing quote can \
                    be on an earlier line than the one reported."
            .to_string();
    } else if line_count(source) <= err.line {
        err.message = "The graph ends before it is complete.".to_string();
        err.hint = "Close every `{` with a `}` and every `[` with a `]`.".to_string();
    }
    err
}

/// Specific advice for the tokens people most often get wrong, or "".
fn syntax_hint(source: &str, near: &str) -> &'static str {
    let directed = is_directed(source);
    match near {
        "->" if !directed => {
            "Undirected graphs (`graph`) join nodes with `--`. To draw arrows, write \
             `digraph` instead of `graph`."
        }
        "--" if directed => {
            "Directed graphs (`digraph`) join nodes with `->`. For lines without arrows, \
             write `graph` instead of `digraph`, or add `[dir=none]` to the edge."
        }
        "->" | "--" => {
            "An edge needs a node on each side, like `a -> b`. Look for a doubled or \
             dangling edge operator on this line."
        }
        "}" | "]" => {
            "Finish the statement before this: give an edge its second node, or an \
             attribute its value, like `color=red` instead of `color=`."
        }
        "=" => {
            "Only attribute lists and graph statements take `name=value`. Quote values with \
             spaces: `label=\"two words\"`."
        }
        _ => "",
    }
}

/// Whether the first graph keyword in source is `digraph`. DOT keywords ignore case.
fn is_directed(source: &str) -> bool {
    let bytes = source.as_bytes();
    let first = bytes.windows(5).position(|w| w.eq_ignore_ascii_case(b"graph"));
    first.is_some_and(|i| i >= 2 && bytes[i - 2..i].eq_ignore_ascii_case(b"di"))
}

/// Explains a failed gvLayout, or an error reported during a layout that Graphviz still
/// finished (a bad HTML-like label is reported this way).
pub(crate) fn layout_failure(source: &str, engine: Engine) -> Error {
    let Some(message) = log::first_error() else { return Error::new(ErrorKind::Layout) };
    if message.contains("layout engine support") || message.contains("Layout type") {
        let name = engine.name();
        let mut err = Error::with(
            ErrorKind::MissingEngine,
            format!("This Graphviz cannot lay out graphs with {name}."),
        );
        err.near = name.to_string();
        return err;
    }
    if let Some(owner_at) = message.find("in label of ") {
        // Graphviz writes "mismatched tag in line 1 \n... x</i> ...\nin label of node a",
        // where the line counts within the label, so the excerpt is located in the source.
        let problem = message.find(" in line ").map_or(message.as_str(), |at| &message[..at]);
        let owner = message[owner_at + "in label of ".len()..].trim();
        let mut err = Error::with(
            ErrorKind::HtmlLabel,
            format!("The HTML-like label of {owner} has a problem: {problem}."),
        );
        if let Some(near) = between(&message, "... ", " ...") {
            (err.line, err.column) = locate(source, near);
            err.near = near.to_string();
        }
        return err;
    }
    Error::with(ErrorKind::Layout, message)
}

/// Explains a failed gvRenderData.
pub(crate) fn render_failure(format: Format) -> Error {
    let message = log::first_error().unwrap_or_default();
    if message.is_empty() || message.contains("not recognized") {
        let name = format.name();
        let mut err = Error::with(
            ErrorKind::MissingFormat,
            format!("This Graphviz cannot write {name} output."),
        );
        err.near = name.to_string();
        return err;
    }
    Error::with(ErrorKind::Render, message)
}

fn number_after(text: &str, prefix: &str) -> usize {
    let Some(at) = text.find(prefix) else { return 0 };
    let rest = &text[at + prefix.len()..];
    let end = rest.bytes().take_while(u8::is_ascii_digit).count();
    rest[..end].parse().unwrap_or(0)
}

fn quoted_after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let at = text.find(prefix)?;
    let rest = &text[at + prefix.len()..];
    let end = rest.rfind('\'')?;
    Some(&rest[..end])
}

fn between<'a>(text: &'a str, open: &str, shut: &str) -> Option<&'a str> {
    let at = text.find(open)?;
    let rest = &text[at + open.len()..];
    let end = rest.find(shut)?;
    Some(&rest[..end])
}
