//! Errors: kinds, stock messages and hints, and how Graphviz's log becomes an [`Error`].

use std::fmt;

use crate::format::{column_of, line_count, locate};
use crate::{Engine, Format, engine_name, format_name, log};

/// What went wrong in [`crate::render`] or [`crate::init`].
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
/// characters), or 0 when Graphviz does not say. `message` is one sentence, in Graphviz's
/// own words when it gave any; `hint` is advice for the author. [`crate::format_error`]
/// renders it with a source excerpt; `Display` renders it without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The kind of problem.
    pub kind: ErrorKind,
    /// 1-based line, or 0 when unknown.
    pub line: usize,
    /// 1-based character column, or 0 when unknown.
    pub column: usize,
    /// The text Graphviz stopped at, for `Syntax` and `HtmlLabel`.
    pub near: String,
    /// What went wrong.
    pub message: String,
    /// How to fix it.
    pub hint: String,
}

impl Error {
    /// An error of `kind` with the stock message and hint.
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self::with(kind, String::new())
    }

    /// An error of `kind` with `message` (or the stock one when empty) and the stock hint.
    pub(crate) fn with(kind: ErrorKind, message: String) -> Self {
        let message = if message.is_empty() { stock_message(kind).to_string() } else { message };
        let hint = stock_hint(kind).to_string();
        Self { kind, line: 0, column: 0, near: String::new(), message, hint }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = crate::format_message(error_title(self.kind), "graphdog", &self.message,
            &self.hint);
        f.write_str(text.trim_end())
    }
}

impl std::error::Error for Error {}

/// A short upper-case title for the kind of error, such as `"DOT SYNTAX ERROR"`.
pub fn error_title(kind: ErrorKind) -> &'static str {
    match kind {
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

fn stock_message(kind: ErrorKind) -> &'static str {
    match kind {
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

fn stock_hint(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Syntax => {
            "Check this line for a missing `;`, `]`, `}` or `\"`. Attribute lists look like \
             `[shape=box, color=red]`, and names with spaces or punctuation need double quotes."
        }
        ErrorKind::HtmlLabel => {
            "HTML-like labels must be well-formed XML: close every tag you open, in order, \
             and write & < > as &amp; &lt; &gt; inside text."
        }
        ErrorKind::EmptyInput => "Write a graph, for example `digraph { a -> b }`.",
        ErrorKind::MissingEngine => {
            "Register the installed plugins with `sudo dot -c`, or install the package that \
             holds the layout plugins (graphviz on Homebrew, Debian and Ubuntu)."
        }
        ErrorKind::MissingFormat => {
            "Register the installed plugins with `sudo dot -c`. PNG output needs the cairo \
             or gd plugin; SVG always works."
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
            "This usually means the system is out of memory, or Graphviz is not installed \
             correctly; check that `dot -V` works."
        }
    }
}

/// Explains why agmemread returned no graph.
pub(crate) fn parse_failure(source: &str) -> Error {
    let message = log::first_error();
    if message.is_empty() {
        return Error::new(ErrorKind::EmptyInput);
    }
    let mut err = Error::with(ErrorKind::Syntax, message.clone());
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
    for i in 0..bytes.len() {
        let end = (i + 5).min(bytes.len());
        if !bytes[i..end].eq_ignore_ascii_case(b"graph") {
            continue;
        }
        return i >= 2 && bytes[i - 2..i].eq_ignore_ascii_case(b"di");
    }
    false
}

/// Explains a failed gvLayout, or an error reported during a layout that Graphviz still
/// finished (a bad HTML-like label is reported this way).
pub(crate) fn layout_failure(source: &str, engine: Engine) -> Error {
    let message = log::first_error();
    if message.contains("layout engine support") || message.contains("Layout type") {
        let name = engine_name(engine);
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
    let message = log::first_error();
    if message.is_empty() || message.contains("not recognized") {
        let name = format_name(format);
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
