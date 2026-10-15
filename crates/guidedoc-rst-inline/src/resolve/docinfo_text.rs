//! Authors and RCS keywords of bibliographic fields.

use guidedoc::{Fail, ListStyle, NodeId, NodeKind};

use super::Resolver;
use crate::languages::Language;

impl Resolver<'_, '_> {
    /// Gives an authors field one paragraph per author. The body may be one paragraph
    /// whose plain text separates the names with the language's separators (";"
    /// before ","), one bullet list with a paragraph per item, or several paragraphs.
    pub fn extract_authors(&mut self, field: NodeId, lang: &Language) -> Result<bool, Fail> {
        let body = self.field_body(field);
        let first = body + 1;
        match self.body_count(field) {
            0 => Ok(false),
            1 => match self.kind_of(first) {
                NodeKind::Paragraph => self.split_authors(body, first, lang),
                NodeKind::List => self.list_authors(first),
                _ => Ok(false),
            },
            _ => {
                let mut c = first.index();
                while c < self.node_end(body) {
                    if self.kind_of(NodeId::from_index(c)) != NodeKind::Paragraph {
                        return Ok(false);
                    }
                    c = self.node_end(NodeId::from_index(c));
                }
                Ok(true)
            }
        }
    }

    /// Unwraps a bullet list whose items each hold one paragraph.
    fn list_authors(&mut self, list: NodeId) -> Result<bool, Fail> {
        if self.ctx.b.document().list_info(list).style != ListStyle::Bullet {
            return Ok(false);
        }
        let mut items = Vec::new();
        let mut c = list.index() + 1;
        while c < self.node_end(list) {
            let item = NodeId::from_index(c);
            let lone_paragraph = self.node_end(item) == self.node_end(item + 1)
                && self.kind_of(item + 1) == NodeKind::Paragraph;
            if !lone_paragraph {
                return Ok(false);
            }
            items.push(item);
            c = self.node_end(item);
        }
        if items.is_empty() {
            return Ok(false);
        }
        // Unwrap items from the last, so earlier indices stay valid.
        for &item in items.iter().rev() {
            self.ctx.b.unwrap_node(item)?;
        }
        self.ctx.b.unwrap_node(list)?;
        Ok(true)
    }

    /// The text of a paragraph's subtree; line breaks become "\n".
    fn paragraph_text(&self, p: NodeId) -> String {
        let mut out = String::new();
        for i in p.index() + 1..self.node_end(p) {
            let id = NodeId::from_index(i);
            match self.kind_of(id) {
                NodeKind::Text => out.push_str(self.text_of(id)),
                NodeKind::SoftBreak | NodeKind::HardBreak => out.push('\n'),
                _ => {}
            }
        }
        out
    }

    fn split_authors(&mut self, body: NodeId, p: NodeId, lang: &Language) -> Result<bool, Fail> {
        let text = self.paragraph_text(p);
        let sep = lang.separators.chars().find(|&s| text.contains(s));
        let span = self.node_span(p);
        let first_new = self.used();
        let parts: Vec<&str> = match sep {
            Some(s) => text.split(s).collect(),
            None => vec![text.as_str()],
        };
        for part in parts {
            let name = part.trim_matches([' ', '\n', '\t']);
            if name.is_empty() {
                continue;
            }
            let para = self.ctx.b.begin_node(NodeKind::Paragraph, span)?;
            self.ctx.b.add_text_leaf(NodeKind::Text, span, name)?;
            self.ctx.b.end_node(para)?;
        }
        if self.used() == first_new {
            return Ok(false);
        }
        let old = self.node_end(p) - p.index();
        self.remove_range(p.index(), p.index() + old)?;
        self.insert_new(first_new - old, p.index(), body)?;
        Ok(true)
    }

    /// Reduces RCS keywords in a paragraph of plain text: "$Date: <date> <time> ... $"
    /// becomes the date as YYYY-MM-DD, "$RCSfile: name,v $" the file name, and any
    /// other "$Keyword: value $" its value.
    pub fn clean_keywords(&mut self, p: NodeId) -> Result<(), Fail> {
        let first = p + 1;
        let end = self.node_end(p);
        if first.index() >= end || self.kind_of(first) != NodeKind::Text {
            return Ok(());
        }
        let plain = (first.index()..end).all(|i| {
            matches!(self.kind_of(NodeId::from_index(i)), NodeKind::Text | NodeKind::SoftBreak)
        });
        if !plain {
            return Ok(());
        }
        let text = self.paragraph_text(p);
        let mut out = String::new();
        if !rcs_substitute(&text, &mut out) {
            return Ok(());
        }
        let t = self.ctx.b.add_text(&out)?;
        self.ctx.b.set_text(first, t);
        if end > first.index() + 1 {
            self.remove_range(first.index() + 1, end)?;
        }
        Ok(())
    }
}

/// A keyword form: "$Date: ...", "$RCSfile: ...", or any keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Date,
    RcsFile,
    Any,
}

/// Applies the first keyword form that occurs in `s` to every occurrence of it,
/// appending the result to `out`; false when no form occurs.
pub(crate) fn rcs_substitute(s: &str, out: &mut String) -> bool {
    for form in [Form::Date, Form::RcsFile, Form::Any] {
        let mut found = false;
        let (mut i, mut from) = (0, 0);
        while i < s.len() {
            let Some(m) = rcs_match(s, i, form) else {
                i += 1;
                continue;
            };
            found = true;
            out.push_str(&s[from..i]);
            match m.date {
                Some((y, mo, d)) => {
                    out.push_str(y);
                    out.push('-');
                    out.push_str(mo);
                    out.push('-');
                    out.push_str(d);
                }
                None => out.push_str(&s[m.value.0..m.value.1]),
            }
            (i, from) = (m.end, m.end);
        }
        if found {
            out.push_str(&s[from..]);
            return true;
        }
    }
    false
}

/// A matched keyword: where it ends, its value, and a date's parts.
struct KeywordMatch<'s> {
    end: usize,
    value: (usize, usize),
    date: Option<(&'s str, &'s str, &'s str)>,
}

/// Matches keyword `form` at `i`. Matches stay within one line; the value runs to the
/// last " $" on the line.
fn rcs_match(s: &str, i: usize, form: Form) -> Option<KeywordMatch<'_>> {
    let t = s.as_bytes();
    if t[i] != b'$' {
        return None;
    }
    let line_end = t[i..].iter().position(|&c| c == b'\n').map_or(t.len(), |k| i + k);
    let mut k = i + 1;
    while k < line_end && t[k].is_ascii_alphabetic() {
        k += 1;
    }
    if k == i + 1 || k + 1 >= line_end || t[k] != b':' || t[k + 1] != b' ' {
        return None;
    }
    let keyword = &s[i + 1..k];
    let value = k + 2;
    let close = last_close(t, value, line_end);
    match form {
        Form::Date => {
            if !keyword.eq_ignore_ascii_case("date") {
                return None;
            }
            let date = rcs_date(s.get(value..line_end)?)?;
            // The rest up to " $" must not contain "$".
            let mut close = None;
            for j in value..line_end - 1 {
                if t[j] == b'$' {
                    break;
                }
                if t[j] == b' ' && t[j + 1] == b'$' {
                    close = Some(j);
                }
            }
            let close = close.filter(|&c| c >= value + 11)?;
            Some(KeywordMatch { end: close + 2, value: (value, close), date: Some(date) })
        }
        Form::RcsFile => {
            let close = close?;
            if !keyword.eq_ignore_ascii_case("rcsfile") || close - value < 3 {
                return None;
            }
            if &t[close - 2..close] != b",v" {
                return None;
            }
            Some(KeywordMatch { end: close + 2, value: (value, close - 2), date: None })
        }
        Form::Any => {
            let close = close.filter(|&c| c > value)?;
            Some(KeywordMatch { end: close + 2, value: (value, close), date: None })
        }
    }
}

fn last_close(t: &[u8], from: usize, line_end: usize) -> Option<usize> {
    (from..line_end.saturating_sub(1)).rev().find(|&j| t[j] == b' ' && t[j + 1] == b'$')
}

/// Reads "YYYY[-/]MM[-/]DD[ T]" followed by a digit or colon.
fn rcs_date(s: &str) -> Option<(&str, &str, &str)> {
    let t = s.as_bytes();
    if t.len() < 12 {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| t[r].iter().all(u8::is_ascii_digit);
    let ok = digits(0..4)
        && digits(5..7)
        && digits(8..10)
        && matches!(t[4], b'-' | b'/')
        && matches!(t[7], b'-' | b'/')
        && matches!(t[10], b' ' | b'T')
        && (t[11] == b':' || t[11].is_ascii_digit());
    ok.then(|| (&s[0..4], &s[5..7], &s[8..10]))
}
