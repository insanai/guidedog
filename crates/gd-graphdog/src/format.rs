//! Elm-style error reports and the line arithmetic behind them.

use std::fmt::Write as _;

use crate::errors::{Error, error_title};

/// Width of a report's title rule and wrapped text.
const WIDTH: usize = 80;

/// Writes `err` the way Elm writes compiler errors: a title rule naming the file, the
/// source line with a caret under the problem, what went wrong, and a hint.
///
/// `source` is the text given to [`crate::render`] and `name` labels it
/// (`"diagram.dot"`, `"<stdin>"`). Returns a new string; never fails.
///
/// ```text
/// -- DOT SYNTAX ERROR ---------------------------------------------- diagram.dot
///
/// 3 |   b -> -> c
///            ^^
/// Graphviz stopped at `->`.
///
/// Hint: An edge needs a node on each side, like `a -> b`. ...
/// ```
pub fn format_error(err: &Error, source: &str, name: &str) -> String {
    let mut out = String::new();
    write_title(&mut out, error_title(err.kind), name);
    if err.line > 0 {
        write_excerpt(&mut out, source, err.line, err.column, err.near.chars().count());
    }
    write_body(&mut out, &err.message, &err.hint);
    out
}

/// Writes an error that has no source to point at, such as a bad command-line option, in
/// the same shape as [`format_error`]. Returns a new string; never fails.
pub fn format_message(title: &str, name: &str, message: &str, hint: &str) -> String {
    let mut out = String::new();
    write_title(&mut out, title, name);
    write_body(&mut out, message, hint);
    out
}

fn write_title(out: &mut String, title: &str, name: &str) {
    let used = title.chars().count() + name.chars().count() + 5;
    let dashes = WIDTH.saturating_sub(used).max(4);
    let _ = write!(out, "-- {title} {} {name}\n\n", "-".repeat(dashes));
}

fn write_body(out: &mut String, message: &str, hint: &str) {
    if !message.is_empty() {
        write_wrapped(out, "", message);
    }
    if !hint.is_empty() {
        out.push('\n');
        write_wrapped(out, "Hint: ", hint);
    }
}

/// Writes prefix and text as lines of at most WIDTH characters, breaking at spaces and
/// keeping the line breaks already in text.
fn write_wrapped(out: &mut String, prefix: &str, text: &str) {
    out.push_str(prefix);
    let mut column = prefix.chars().count();
    for line in text.lines() {
        let mut fresh = true;
        for word in line.split_whitespace() {
            let width = word.chars().count();
            if !fresh && column + 1 + width > WIDTH {
                out.push('\n');
                column = 0;
            } else if !fresh {
                out.push(' ');
                column += 1;
            }
            out.push_str(word);
            column += width;
            fresh = false;
        }
        out.push('\n');
        column = 0;
    }
}

/// Writes the line before (for context) and the reported line with a gutter, and a caret
/// row under the columns `[column, column + width)` when the column is known.
fn write_excerpt(out: &mut String, source: &str, line: usize, column: usize, width: usize) {
    let Some(text) = line_text(source, line) else { return };
    let gutter = digits(line);
    if let Some(previous) = line_text(source, line - 1)
        && !previous.trim().is_empty()
    {
        write_numbered(out, gutter, line - 1, previous);
    }
    write_numbered(out, gutter, line, text);
    if column > 0 {
        out.push_str(&" ".repeat(gutter + 3));
        for c in text.chars().take(column - 1) {
            out.push_str(if c == '\t' { "    " } else { " " });
        }
        out.push_str(&"^".repeat(width.max(1)));
        out.push('\n');
    }
}

/// Writes "  3 | text", expanding tabs to four spaces so carets line up.
fn write_numbered(out: &mut String, gutter: usize, n: usize, text: &str) {
    let _ = write!(out, "{}{n} | ", " ".repeat(gutter - digits(n)));
    for c in text.chars() {
        if c == '\t' {
            out.push_str("    ");
        } else {
            out.push(c);
        }
    }
    out.push('\n');
}

fn digits(n: usize) -> usize {
    n.to_string().len()
}

/// 1-based line `n` of source, without its line ending.
pub(crate) fn line_text(source: &str, n: usize) -> Option<&str> {
    if n < 1 {
        return None;
    }
    let line = source.split('\n').nth(n - 1)?;
    Some(line.strip_suffix('\r').unwrap_or(line))
}

/// The number of lines in source, ignoring trailing newlines.
pub(crate) fn line_count(source: &str) -> usize {
    source.trim_end_matches('\n').matches('\n').count() + 1
}

/// The 1-based character column of `near` on the given line, or 0. Graphviz names only
/// the token, so when it occurs more than once the last occurrence is used: the parser
/// reports the first token it cannot accept, which usually follows valid ones.
pub(crate) fn column_of(source: &str, line: usize, near: &str) -> usize {
    let Some(text) = line_text(source, line) else { return 0 };
    if near.is_empty() {
        return 0;
    }
    text.rfind(near).map_or(0, |at| text[..at].chars().count() + 1)
}

/// The first occurrence of needle in source as a 1-based line and column, or (0, 0).
pub(crate) fn locate(source: &str, needle: &str) -> (usize, usize) {
    let Some(at) = source.find(needle) else { return (0, 0) };
    let line = source[..at].matches('\n').count() + 1;
    let start = source[..at].rfind('\n').map_or(0, |i| i + 1);
    (line, source[start..at].chars().count() + 1)
}
