//! The terminal form of a report.

use crate::ansi::{BOLD_CYAN, BOLD_RED, BOLD_YELLOW, CYAN, FAINT, GREEN, RED, write_styled};
use crate::width::{display_width, escape_char, escape_terminal, escaped_width};
use crate::{Excerpt, Report, Severity};
use std::fmt::Write;

/// How [`render_text`] lays a report out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Whether to use ANSI colors and bold; words never depend on them.
    pub color: bool,
    /// The width of the title rule and of wrapped prose, in terminal cells.
    pub width: usize,
}

impl Default for Style {
    fn default() -> Style {
        Style { color: false, width: 80 }
    }
}

/// The widest excerpt window in cells; longer lines are cut around the span with `…`.
const MAX_EXCERPT: usize = 160;

/// Lays `report` out the Elm way: a title rule naming the place, the explanation, the
/// source line with carets under the span, and the hints. Every text from the report
/// is escaped so control characters in file names or sources cannot reach the terminal;
/// only the style's own colors do. Returns an allocated string ending in a line break;
/// [`write_text`] appends to a buffer the caller reuses instead.
pub fn render_text(report: &Report, style: &Style) -> String {
    let mut out = String::with_capacity(256);
    write_text(&mut out, report, style);
    out
}

/// Appends the text [`render_text`] returns to `out`, so a caller rendering many reports
/// can `clear()` and reuse one buffer.
pub fn write_text(out: &mut String, report: &Report, style: &Style) {
    write_title(out, report, style);
    if !report.message.is_empty() {
        out.push('\n');
        write_wrapped(out, &escape_terminal(&report.message), style.width);
    }
    if let Some(excerpt) = &report.excerpt
        && report.line > 0
    {
        out.push('\n');
        write_excerpt(out, excerpt, report.line, style.color);
    }
    write_hints(out, report, style);
}

fn location(report: &Report) -> String {
    let path = escape_terminal(&report.path);
    match (report.line, report.column) {
        (0, _) => path.into_owned(),
        (line, 0) => format!("{path}:{line}"),
        (line, column) => format!("{path}:{line}:{column}"),
    }
}

fn write_title(out: &mut String, report: &Report, style: &Style) {
    let (prefix, heading) = match report.severity {
        Severity::Error => ("", BOLD_RED),
        Severity::Warning => ("WARNING: ", BOLD_YELLOW),
        Severity::Note => ("NOTE: ", BOLD_CYAN),
    };
    let title = format!("{prefix}{}", escape_terminal(&report.title));
    let code = if report.code.is_empty() {
        String::new()
    } else {
        format!(" [{}]", escape_terminal(&report.code))
    };
    let place = location(report);
    let used = 4 + display_width(&title) + display_width(&code) + display_width(&place);
    let dashes = style.width.saturating_sub(used + 1).max(3);
    out.push_str("-- ");
    write_styled(out, &title, heading, style.color);
    write_styled(out, &code, FAINT, style.color);
    out.push(' ');
    out.push_str(&"-".repeat(dashes));
    if !place.is_empty() {
        out.push(' ');
        write_styled(out, &place, CYAN, style.color);
    }
    out.push('\n');
}

fn write_hints(out: &mut String, report: &Report, style: &Style) {
    let mut hints = report.hints.iter().filter(|h| !h.is_empty());
    let Some(first) = hints.next() else { return };
    out.push('\n');
    let mut text = format!("Hint: {}", escape_terminal(first));
    for more in hints {
        text.push('\n');
        text.push_str(&escape_terminal(more));
    }
    if style.color {
        write_styled(out, "Hint:", GREEN, true);
        let mark = out.len();
        write_wrapped(out, &text, style.width);
        out.replace_range(mark..mark + 5, "");
    } else {
        write_wrapped(out, &text, style.width);
    }
}

/// Writes the optional context line and the reported line with a `N| ` gutter, and a
/// caret row under the span, cutting long lines to a window around it.
fn write_excerpt(out: &mut String, excerpt: &Excerpt, line: usize, color: bool) {
    let gutter = line.to_string().len();
    if line > 1 && !excerpt.before.trim().is_empty() {
        let (text, _, _) = window(&excerpt.before, 0, 0);
        let _ = writeln!(out, "{:>gutter$}| {text}", line - 1);
    }
    let (text, marker, carets) = window(&excerpt.text, excerpt.start, excerpt.end);
    let _ = writeln!(out, "{line:>gutter$}| {text}");
    out.push_str(&" ".repeat(gutter + 2 + marker));
    write_styled(out, &"^".repeat(carets.max(1)), RED, color);
    out.push('\n');
}

/// Escapes `line` for the terminal within [`MAX_EXCERPT`] cells and returns the text,
/// the cells before the span and the cells under it. The window starts a third of the
/// way before the span when the span lies far right.
fn window(line: &str, start: usize, end: usize) -> (String, usize, usize) {
    let before: usize =
        line.char_indices().take_while(|&(i, _)| i < start).map(|(_, c)| escaped_width(c)).sum();
    let cut = before.saturating_sub(MAX_EXCERPT / 3);
    let (mut text, mut skipped) = (String::new(), 0);
    let (mut cells, mut marker, mut under) = (0, 0, 0);
    for (i, c) in line.char_indices() {
        let w = escaped_width(c);
        if skipped < cut && i < start {
            skipped += w;
            continue;
        }
        if cells == 0 && skipped > 0 {
            text.push('…');
            (cells, marker) = (1, 1);
        }
        if cells + w > MAX_EXCERPT - 1 {
            text.push('…');
            break;
        }
        if i < start {
            marker += w;
        } else if i < end {
            under += w;
        }
        escape_char(&mut text, c);
        cells += w;
    }
    (text, marker, under)
}

/// Wraps `text` at spaces so lines take at most `width` cells where words allow, keeping
/// its own line breaks and the indentation that starts each line. Returns allocated text
/// ending in a line break (empty for empty text); [`write_wrapped`] appends instead.
pub fn wrap(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    write_wrapped(&mut out, text, width);
    out
}

/// Appends `text` wrapped as [`wrap`] wraps it to `out`.
pub fn write_wrapped(out: &mut String, text: &str, width: usize) {
    for line in text.lines() {
        let indent_len = line.len() - line.trim_start_matches(' ').len();
        let indent = &line[..indent_len];
        out.push_str(indent);
        let mut column = indent_len;
        let mut fresh = true;
        for word in line[indent_len..].split(' ').filter(|w| !w.is_empty()) {
            let w = display_width(word);
            if !fresh && column + 1 + w > width {
                out.push('\n');
                out.push_str(indent);
                column = indent_len;
            } else if !fresh {
                out.push(' ');
                column += 1;
            }
            out.push_str(word);
            column += w;
            fresh = false;
        }
        out.push('\n');
    }
}
