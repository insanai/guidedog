//! Inline content: the buffer a block collects its inline markup in, with the places
//! where a line may break, and the wrapping that writes it as lines.

use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::escape::{
    is_adornment, may_follow_markup, may_precede_markup, push_text, starts_construct, width,
};

/// Inline reStructuredText being collected. Text is escaped as it arrives; markup
/// arrives whole and is never broken. Single spaces between words are the only break
/// opportunities.
#[derive(Debug, Default)]
pub(crate) struct Inline {
    pub buf: String,
    /// Byte offsets in `buf` of spaces a line may break at.
    breaks: Vec<usize>,
    /// Spaces inside markup, where a line may break only when nothing else fits (a
    /// table cell of fixed width).
    inner: Vec<usize>,
    /// The last thing appended was markup, whose end-string needs a boundary.
    markup_end: bool,
    /// Keep white space and line breaks as they are (parsed literals, line blocks).
    pub preserve: bool,
    /// Escape colons in text (a field name, which a colon would end).
    pub colons: bool,
    /// Room for escaped text and formatted markup, reused.
    escaped: String,
    piece: String,
    /// The break points `wrap` uses, reused.
    points: Vec<usize>,
    /// Lines `wrap` splits again at line breaks, reused.
    split: Lines,
}

/// Lines of output that keep their strings when cleared, so wrapping paragraph after
/// paragraph reuses them. It dereferences to the lines in use.
#[derive(Debug, Default)]
pub(crate) struct Lines {
    rows: Vec<String>,
    len: usize,
}

impl Lines {
    /// Forgets every line, keeping the strings for the next ones.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Appends a line holding `s`.
    pub fn push(&mut self, s: &str) {
        match self.rows.get_mut(self.len) {
            Some(row) => {
                row.clear();
                row.push_str(s);
            }
            None => self.rows.push(s.to_owned()),
        }
        self.len += 1;
    }
}

impl Deref for Lines {
    type Target = [String];

    fn deref(&self) -> &[String] {
        &self.rows[..self.len]
    }
}

impl<'l> IntoIterator for &'l Lines {
    type Item = &'l String;
    type IntoIter = std::slice::Iter<'l, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl DerefMut for Lines {
    fn deref_mut(&mut self) -> &mut [String] {
        &mut self.rows[..self.len]
    }
}

impl Inline {
    /// Empties the buffer for a new block, keeping its capacity.
    pub fn clear(&mut self, preserve: bool) {
        self.buf.clear();
        self.breaks.clear();
        self.inner.clear();
        self.colons = false;
        self.markup_end = false;
        self.preserve = preserve;
    }

    /// Appends plain text, escaped.
    pub fn text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        let mut escaped = std::mem::take(&mut self.escaped);
        escaped.clear();
        if self.colons {
            let mut plain = std::mem::take(&mut self.piece);
            plain.clear();
            push_text(&mut plain, s, None);
            push_colons_escaped(&mut escaped, &plain);
            self.piece = plain;
        } else {
            push_text(&mut escaped, s, None);
        }
        self.push_escaped(&escaped);
        self.escaped = escaped;
    }

    /// Appends escaped text, recording its break opportunities.
    fn push_escaped(&mut self, escaped: &str) {
        if self.markup_end {
            let first = escaped.chars().next().unwrap_or(' ');
            if !(first == '\\' || may_follow_markup(first)) {
                self.buf.push_str("\\ ");
            }
        }
        self.markup_end = false;
        if self.preserve {
            self.buf.push_str(escaped);
            return;
        }
        let bytes = escaped.as_bytes();
        for (i, c) in escaped.char_indices() {
            if c == '\n' || c == '\r' || c == '\t' {
                self.space();
                continue;
            }
            if c == ' ' {
                let prev_space = self.buf.ends_with(' ') || self.buf.is_empty();
                let next_space = bytes.get(i + 1).is_some_and(|&b| b == b' ');
                if !prev_space && !next_space {
                    self.breaks.push(self.buf.len());
                }
            }
            self.buf.push(c);
        }
    }

    /// Appends a break opportunity (a soft line break of the source).
    pub fn space(&mut self) {
        if self.preserve {
            self.buf.push('\n');
            self.markup_end = false;
            return;
        }
        if !self.buf.is_empty() && !self.buf.ends_with(' ') {
            self.breaks.push(self.buf.len());
            self.buf.push(' ');
        }
        self.markup_end = false;
    }

    /// Appends a whole piece of markup, separated from what precedes it by an escaped
    /// space when its start-string would not be recognized there.
    pub fn markup(&mut self, s: &str) {
        if let Some(last) = self.buf.chars().next_back()
            && (self.markup_end || !may_precede_markup(last))
        {
            self.buf.push_str("\\ ");
        }
        let base = self.buf.len();
        let bytes = s.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            let single = i > 0 && bytes[i - 1] != b' ' && bytes[i - 1] != b'\\';
            if b == b' ' && single && bytes.get(i + 1).is_some_and(|&n| n != b' ') {
                self.inner.push(base + i);
            }
        }
        self.buf.push_str(s);
        self.markup_end = true;
    }

    /// Appends a whole piece of markup formatted from `args` (see `markup`).
    pub fn markup_fmt(&mut self, args: fmt::Arguments<'_>) {
        let mut piece = std::mem::take(&mut self.piece);
        piece.clear();
        let _ = fmt::write(&mut piece, args);
        self.markup(&piece);
        self.piece = piece;
    }

    /// Appends text that is neither markup nor to be escaped (already escaped).
    pub fn raw(&mut self, s: &str) {
        self.buf.push_str(s);
        self.markup_end = false;
    }

    /// Escapes a final "::" so the paragraph does not announce a literal block.
    pub fn guard_literal_marker(&mut self) {
        if self.buf.ends_with("::") && !self.markup_end {
            self.buf.insert(self.buf.len() - 1, '\\');
        }
    }

    /// Breaks the buffer into `out`'s lines: the first at most `first` columns wide,
    /// the others `rest`. Lines that would start another construct are escaped.
    pub fn wrap(&mut self, first: usize, rest: usize, tight: bool, out: &mut Lines) {
        out.clear();
        if self.preserve {
            self.buf.split('\n').for_each(|line| out.push(line));
            return;
        }
        let mut points = std::mem::take(&mut self.points);
        points.clear();
        points.extend_from_slice(&self.breaks);
        if tight {
            points.extend_from_slice(&self.inner);
            points.sort_unstable();
        }
        let mut start = 0;
        let mut last_break: Option<usize> = None;
        let mut limit = first;
        for &b in points.iter().chain(std::iter::once(&self.buf.len())) {
            let candidate = &self.buf[start..b];
            if width(candidate) > limit
                && let Some(lb) = last_break
            {
                out.push(&self.buf[start..lb]);
                start = lb + 1;
                limit = rest;
            }
            last_break = (b < self.buf.len()).then_some(b);
        }
        self.points = points;
        out.push(&self.buf[start..]);
        guard_lines(out);
        if out.iter().any(|l| l.contains('\n')) {
            // Markup holding a line break (math) continues on the next line.
            let mut split = std::mem::take(&mut self.split);
            split.clear();
            out.iter().flat_map(|l| l.split('\n')).for_each(|line| split.push(line));
            std::mem::swap(out, &mut split);
            self.split = split;
        }
    }
}

/// Appends escaped text to `out`, escaping the colons that are not escaped yet.
fn push_colons_escaped(out: &mut String, s: &str) {
    let mut escaping = false;
    for c in s.chars() {
        if c == ':' && !escaping {
            out.push('\\');
        }
        escaping = c == '\\' && !escaping;
        out.push(c);
    }
}

/// Escapes the first character of lines that a reader would take for something else:
/// the first line of a paragraph when it starts a construct, a later line when it
/// looks like a section adornment.
fn guard_lines(lines: &mut [String]) {
    for (i, line) in lines.iter_mut().enumerate() {
        let risky = if i == 0 { starts_construct(line) } else { is_adornment(line) };
        if risky {
            line.insert(0, '\\');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_gets_boundaries() {
        let mut i = Inline::default();
        i.text("foo");
        i.markup("*bar*");
        i.text("baz qux");
        assert_eq!(i.buf, "foo\\ *bar*\\ baz qux");
        let mut i = Inline::default();
        i.text("(");
        i.markup("*a*");
        i.markup("**b**");
        i.text(").");
        assert_eq!(i.buf, "(*a*\\ **b**).");
    }

    #[test]
    fn wraps_at_single_spaces_only() {
        let mut i = Inline::default();
        i.text("aaa bbb  ccc ddd");
        i.space();
        i.markup("`x y`_");
        let mut lines = Lines::default();
        i.wrap(8, 8, false, &mut lines);
        assert_eq!(*lines, ["aaa", "bbb  ccc", "ddd", "`x y`_"]);
        i.wrap(4, 4, true, &mut lines);
        assert_eq!(lines.last().map(String::as_str), Some("y`_"));
    }

    #[test]
    fn risky_line_starts_are_escaped() {
        let mut i = Inline::default();
        i.text("1. first ------");
        let mut lines = Lines::default();
        i.wrap(8, 8, false, &mut lines);
        assert_eq!(*lines, ["\\1. first", "\\------"]);
    }
}
