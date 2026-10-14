//! Elm-style diagnostic reports shared by every Guidedog crate.
//!
//! Libraries return structured errors; hosts turn them into a [`Report`] and render it
//! with [`render_text`] for people or [`render_json`] for tools:
//!
//! ```text
//! -- UNRESOLVED REFERENCE ----------------------------------------------- intro.rst:12:5
//!
//! No target is named `instal`.
//!
//! 12| See instal_ for details.
//!         ^^^^^^^
//! Hint: Define it with `.. _instal:` above the section, or correct the spelling.
//! ```
//!
//! Columns count Unicode scalars, as editors do; terminal cell widths (wide East Asian
//! characters, combining marks, escaped controls) only affect where carets are drawn.
//! The crate has no dependencies and never prints.

mod json;
mod text;
mod width;

pub use json::{escape_json, render_json, write_json};
pub use text::{Style, render_text, wrap, write_text, write_wrapped};
pub use width::{char_width, display_width, escape_terminal, is_control, is_wide, is_zero_width};

/// The version of the JSON form [`render_json`] writes; it changes only when a field does.
pub const SCHEMA: u32 = 1;

/// How serious a report is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The input cannot be used as it is.
    #[default]
    Error,
    /// The result was produced but probably not as intended.
    Warning,
    /// Extra information, such as where something was first defined.
    Note,
}

impl Severity {
    /// The lower-case name used in JSON: `"error"`, `"warning"` or `"note"`.
    pub fn name(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }
}

/// The source line a report points at, with the byte range to underline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Excerpt {
    /// The complete source line, without its line ending.
    pub text: String,
    /// Byte offset in `text` where the underline starts (clamped to a char boundary).
    pub start: usize,
    /// Byte offset in `text` where the underline ends; at least one caret is drawn.
    pub end: usize,
    /// The line before, shown above for context when it is not blank; may be empty.
    pub before: String,
}

/// A diagnostic ready to render: what happened, where, and what to do about it.
///
/// Fill it with struct-update syntax, or start from [`Report::error`] and attach a
/// position with [`Report::at`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// A stable identifier such as `"toml.duplicate-key"`; may be empty.
    pub code: String,
    /// How serious the report is.
    pub severity: Severity,
    /// A short upper-case title such as `"DUPLICATE KEY"`.
    pub title: String,
    /// The plain explanation; may span several lines.
    pub message: String,
    /// How to fix it; the first is shown after `Hint:`, the rest on their own lines.
    pub hints: Vec<String>,
    /// The file name as it should be shown; may be empty.
    pub path: String,
    /// 1-based line, or 0 when the report has no position.
    pub line: usize,
    /// 1-based column counting Unicode scalars, or 0 when unknown.
    pub column: usize,
    /// 1-based line where the reported span ends, or 0.
    pub end_line: usize,
    /// 1-based column where the reported span ends (exclusive), or 0.
    pub end_column: usize,
    /// The source line to show with carets, if any.
    pub excerpt: Option<Excerpt>,
    /// A category as `"type.subtype"` that users may silence, such as `"ref.term"`.
    pub category: String,
}

impl Report {
    /// Starts an error report with a code, title and message; it has no position yet.
    pub fn error(code: &str, title: &str, message: &str) -> Report {
        Report {
            code: code.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            ..Report::default()
        }
    }

    /// Adds a hint and returns the report, for chaining.
    pub fn hint(mut self, hint: &str) -> Report {
        self.hints.push(hint.to_string());
        self
    }

    /// Positions the report at bytes `start..end` of `source` (clamped to the text and
    /// to char boundaries), naming the file `path`; fills line, columns and excerpt.
    pub fn at(mut self, path: &str, source: &str, start: usize, end: usize) -> Report {
        self.path = path.to_string();
        set_span(&mut self, source, start, end);
        self
    }
}

/// Fills `report`'s line, columns and excerpt from bytes `start..end` of `source`. Offsets
/// past the end are clamped and offsets inside a character move back to its start. The
/// excerpt underlines the part of the span on its first line.
pub fn set_span(report: &mut Report, source: &str, start: usize, end: usize) {
    let start = floor_boundary(source, start);
    let end = floor_boundary(source, end.max(start));
    (report.line, report.column) = line_column(source, start);
    (report.end_line, report.end_column) = line_column(source, end);
    let (line_start, line_end) = line_bounds(source, start);
    let before = if line_start > 0 {
        let (s, e) = line_bounds(source, line_start - 1);
        source[s..e].to_string()
    } else {
        String::new()
    };
    report.excerpt = Some(Excerpt {
        text: source[line_start..line_end].to_string(),
        start: start - line_start,
        end: end.min(line_end) - line_start,
        before,
    });
}

/// Fills `report`'s position and excerpt from a 1-based `line` and scalar `column` of
/// `source`, for positions that tools report without byte offsets. Does nothing when
/// the line does not exist.
pub fn set_line_column(report: &mut Report, source: &str, line: usize, column: usize) {
    let Some(offset) = line_offset(source, line) else { return };
    let (_, line_end) = line_bounds(source, offset);
    let at = source[offset..line_end]
        .char_indices()
        .nth(column.saturating_sub(1))
        .map_or(line_end, |(i, _)| offset + i);
    set_span(report, source, at, at);
}

/// The byte offset where 1-based line `line` of `text` starts, or `None` past the end.
pub fn line_offset(text: &str, line: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    if line == 1 {
        return Some(0);
    }
    text.match_indices('\n').nth(line - 2).map(|(i, _)| i + 1)
}

/// The 1-based line and column of byte `offset` in `text`; the column counts Unicode
/// scalars. Offsets past the end or inside a character are clamped down.
pub fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let offset = floor_boundary(text, offset);
    let head = &text[..offset];
    let line = head.bytes().filter(|&b| b == b'\n').count() + 1;
    let line_start = head.rfind('\n').map_or(0, |i| i + 1);
    (line, head[line_start..].chars().count() + 1)
}

/// The byte range of the line holding `offset`, without its `\n` or `\r\n` ending.
pub fn line_bounds(text: &str, offset: usize) -> (usize, usize) {
    let offset = floor_boundary(text, offset);
    let start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let mut end = text[offset..].find('\n').map_or(text.len(), |i| offset + i);
    if end > start && text.as_bytes()[end - 1] == b'\r' {
        end -= 1;
    }
    (start, end)
}

/// Clamps `offset` to `text` and moves it back to the start of the character it is in.
pub fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut i = offset.min(text.len());
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// The Levenshtein distance between `a` and `b`, counting Unicode scalars.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The candidate closest to `name` when it is near enough to be a typo (one edit for
/// names of up to three characters, two otherwise), for "Did you mean" hints. Borrows
/// from `candidates`; returns `None` when nothing is close or `name` itself is listed.
pub fn suggest<'a, I>(name: &str, candidates: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let limit = if name.chars().count() <= 3 { 1 } else { 2 };
    let mut best: Option<(&str, usize)> = None;
    for c in candidates {
        if c == name {
            return None;
        }
        if c.len() > name.len() + 8 || name.len() > c.len() + 8 {
            continue;
        }
        let d = edit_distance(name, c);
        if d <= limit && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((c, d));
        }
    }
    best.map(|(c, _)| c)
}

#[cfg(test)]
mod tests;
