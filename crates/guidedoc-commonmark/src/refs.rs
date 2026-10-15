//! Link scanning shared by reference definitions and inline links (labels,
//! destinations, titles), and the link reference definitions that open paragraphs.

use guidedoc::{Fail, normalize_label};

use crate::blocks::{BlockParser, is_space_or_tab};
use crate::host::{Host, Ref};
use crate::inline::entities::push_unescaped;
use crate::messages::DUPLICATE_REFERENCE;
use crate::settings::Rule;

/// The longest label kept, in bytes (CommonMark allows 999 characters).
pub(crate) const MAX_LABEL_BYTES: usize = 4000;

/// MAX_LINK_PARENS bounds parenthesis nesting in a destination, as the reference C
/// implementation does; it keeps repeated unclosed openers from rescanning a line.
const MAX_LINK_PARENS: usize = 32;

/// A scanned part of a subject: `[start, end)` is the content, `next` follows it.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Range {
    pub start: usize,
    pub end: usize,
    pub next: usize,
}

/// Skips spaces and tabs with at most one line ending among them.
pub(crate) fn spnl(s: &[u8], pos: usize) -> usize {
    let mut i = pos;
    while i < s.len() && is_space_or_tab(s[i]) {
        i += 1;
    }
    if i < s.len() && s[i] == b'\n' {
        i += 1;
        while i < s.len() && is_space_or_tab(s[i]) {
            i += 1;
        }
    }
    i
}

/// Returns the length of a link label at `s[pos]`, brackets included, or 0. A label
/// holds at most 999 characters and no unescaped brackets.
pub(crate) fn scan_link_label(s: &[u8], pos: usize) -> usize {
    if s.get(pos) != Some(&b'[') {
        return 0;
    }
    let mut chars = 0;
    let mut i = pos + 1;
    while i < s.len() && chars <= 999 {
        match s[i] {
            b']' => return i + 1 - pos,
            b'[' => return 0,
            b'\\' if i + 1 < s.len() => {
                i += 1;
                chars += 1;
            }
            _ => {}
        }
        i += 1;
        while i < s.len() && s[i] & 0xC0 == 0x80 {
            i += 1;
        }
        chars += 1;
    }
    0
}

/// Scans a link destination at `pos`: `<...>` or a run without spaces and with
/// balanced parentheses.
pub(crate) fn scan_link_destination(s: &[u8], pos: usize) -> Result<Range, usize> {
    if s.get(pos) == Some(&b'<') {
        let mut i = pos + 1;
        while i < s.len() {
            match s[i] {
                b'>' => return Ok(Range { start: pos + 1, end: i, next: i + 1 }),
                b'<' | b'\n' => return Err(i),
                b'\\' if i + 1 < s.len() && s[i + 1] != b'\n' => i += 1,
                _ => {}
            }
            i += 1;
        }
        return Err(i);
    }
    let mut depth = 0;
    let mut i = pos;
    while i < s.len() {
        let c = s[i];
        if c == b'\\' && i + 1 < s.len() && s[i + 1].is_ascii_punctuation() {
            i += 2;
            continue;
        }
        if c == b'(' {
            depth += 1;
            if depth > MAX_LINK_PARENS {
                return Err(i);
            }
        } else if c == b')' {
            if depth < 1 {
                break;
            }
            depth -= 1;
        } else if c <= b' ' || c == 0x7f {
            break;
        }
        i += 1;
    }
    if (i == pos && s.get(i) != Some(&b')')) || depth != 0 {
        return Err(i);
    }
    Ok(Range { start: pos, end: i, next: i })
}

/// Scans a link title at `pos`: "...", '...', or (...).
pub(crate) fn scan_link_title(s: &[u8], pos: usize) -> Result<Range, usize> {
    let close = match s.get(pos) {
        Some(b'"') => b'"',
        Some(b'\'') => b'\'',
        Some(b'(') => b')',
        _ => return Err(pos),
    };
    let mut i = pos + 1;
    while i < s.len() {
        let c = s[i];
        if c == b'\\' && i + 1 < s.len() {
            i += 2;
            continue;
        }
        if c == close {
            return Ok(Range { start: pos + 1, end: i, next: i + 1 });
        }
        if close == b')' && c == b'(' {
            return Err(i);
        }
        i += 1;
    }
    Err(i)
}

/// Returns the position after trailing spaces and one line ending.
fn at_line_end(s: &[u8], pos: usize) -> Option<usize> {
    let mut i = pos;
    while i < s.len() && is_space_or_tab(s[i]) {
        i += 1;
    }
    if i >= s.len() {
        return Some(i);
    }
    (s[i] == b'\n').then_some(i + 1)
}

/// Finds a definition by its label as written, without brackets.
pub(crate) fn lookup_ref(h: &mut Host<'_, '_>, label: &str) -> Option<Ref> {
    if h.refs.is_empty() || label.len() > MAX_LABEL_BYTES {
        return None;
    }
    h.label_tmp.clear();
    normalize_label(&mut h.label_tmp, label);
    if h.label_tmp.is_empty() {
        return None;
    }
    h.refs.get(h.label_tmp.as_str()).cloned()
}

impl BlockParser<'_, '_, '_, '_> {
    /// Consumes link reference definitions at the start of a paragraph and tells
    /// whether it found any.
    pub(crate) fn parse_paragraph_refs(&mut self, id: usize) -> Result<bool, Fail> {
        if !self.h.on(Rule::Reference) {
            return Ok(false);
        }
        let mut found = false;
        loop {
            let blk = self.sk.blocks[id];
            if blk.content_start >= blk.content_end
                || self.sk.arena.as_bytes()[blk.content_start] != b'['
            {
                break;
            }
            let n = self.parse_reference(blk.content_start, blk.content_end)?;
            if n == 0 {
                break;
            }
            self.sk.blocks[id].content_start += n;
            found = true;
        }
        Ok(found)
    }

    /// Reads one definition at the start of arena range `[a, z)` and returns the bytes
    /// it consumed, or 0.
    fn parse_reference(&mut self, a: usize, z: usize) -> Result<usize, Fail> {
        let s = &self.sk.arena.as_bytes()[a..z];
        let n = scan_link_label(s, 0);
        if n == 0 || n >= s.len() || s[n] != b':' {
            return Ok(0);
        }
        let dest = scan_link_destination(s, spnl(s, n + 1));
        let spent = match dest {
            Ok(r) => r.next,
            Err(next) => next,
        };
        self.h.spend(spent)?;
        let Ok(dest) = dest else { return Ok(0) };
        let s = &self.sk.arena.as_bytes()[a..z];
        let before_title = dest.next;
        let pos = spnl(s, before_title);
        let mut title = None;
        if pos != before_title {
            let r = scan_link_title(s, pos);
            let spent = match r {
                Ok(r) => r.next - pos,
                Err(next) => next.saturating_sub(pos),
            };
            self.h.spend(spent.max(1))?;
            title = r.ok();
        }
        let s = &self.sk.arena.as_bytes()[a..z];
        let mut end = at_line_end(s, title.map_or(before_title, |t| t.next));
        if end.is_none() {
            if title.is_none_or(|t| t.end == t.start) {
                return Ok(0);
            }
            title = None;
            end = at_line_end(s, before_title);
        }
        let Some(end) = end else { return Ok(0) };
        let label = a + 1..a + n - 1;
        let dest = a + dest.start..a + dest.end;
        let title = title.map(|t| a + t.start..a + t.end);
        if !self.store_reference(label, dest, title) {
            return Ok(0);
        }
        Ok(end)
    }

    /// Keeps the first definition of each normalized label, its destination and title
    /// decoded. The label, destination, and title are arena ranges.
    fn store_reference(
        &mut self,
        label: std::ops::Range<usize>,
        dest: std::ops::Range<usize>,
        title: Option<std::ops::Range<usize>>,
    ) -> bool {
        let arena = &self.sk.arena;
        let h = &mut *self.h;
        h.label_tmp.clear();
        normalize_label(&mut h.label_tmp, &arena[label.clone()]);
        if h.label_tmp.is_empty() {
            return false;
        }
        if h.refs.contains_key(&h.label_tmp) {
            if h.myst {
                let at = self.sk.arena_source(&arena[label.clone()]);
                let sp = self.sk.span(at.saturating_sub(1), at + label.len() + 1);
                h.report_span(DUPLICATE_REFERENCE, sp);
            }
            return true;
        }
        let start = h.ref_text.len();
        push_unescaped(&mut h.ref_text, &arena[dest]);
        let mut r = Ref { dest: start..h.ref_text.len(), title: None };
        if let Some(t) = title {
            let start = h.ref_text.len();
            push_unescaped(&mut h.ref_text, &arena[t]);
            r.title = Some(start..h.ref_text.len());
        }
        h.refs.insert(&h.label_tmp, r);
        true
    }
}
