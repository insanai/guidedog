//! Elm-style error reports (rendered by gd-report) and the line arithmetic behind them.

use crate::errors::Error;

/// Renders `err` the way Elm writes compiler errors: a title rule naming the file, the
/// source line with carets under the problem, what went wrong, and a hint. Plain text,
/// 80 columns wide; use [`Error::report`] for colour or JSON.
///
/// `source` is the text given to [`crate::Context::render`] and `name` labels it
/// (`"diagram.dot"`, `"<stdin>"`). Returns a new string; never fails.
///
/// ```text
/// -- DOT SYNTAX ERROR [graphdog.syntax] ---------------------------- diagram.dot:3:8
///
/// Graphviz stopped at `->`.
///
/// 2|   a -> b
/// 3|   b -> -> c
///           ^^
///
/// Hint: An edge needs a node on each side, like `a -> b`. ...
/// ```
pub fn format_error(err: &Error, source: &str, name: &str) -> String {
    gd_report::render_text(&err.report(source, name), &gd_report::Style::default())
}

/// The byte range a report underlines: `near` at the 1-based character `column` of
/// `line`, or the whole line when the column is unknown. `None` when the line is.
pub(crate) fn span(
    source: &str,
    line: usize,
    column: usize,
    near: &str,
) -> Option<(usize, usize)> {
    let start = gd_report::line_offset(source, line)?;
    let (_, end) = gd_report::line_bounds(source, start);
    if column == 0 {
        return Some((start, end));
    }
    let at = source[start..end].char_indices().nth(column - 1).map_or(end, |(i, _)| start + i);
    Some((at, (at + near.len()).min(end)))
}

/// 1-based line `n` of source, without its line ending.
pub(crate) fn line_text(source: &str, n: usize) -> Option<&str> {
    let start = gd_report::line_offset(source, n)?;
    let (_, end) = gd_report::line_bounds(source, start);
    Some(&source[start..end])
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
    gd_report::line_column(source, at)
}
