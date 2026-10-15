//! Emphasis: runs of `*` and `_` on the delimiter stack, resolved into Emphasis and
//! Strong by CommonMark's "process emphasis" procedure.

use guidedoc::{Fail, NodeKind, decode_last_rune, decode_rune};
use guidedoc::{is_unicode_punctuation, is_unicode_whitespace};

use super::Inline;

/// A delimiter run that can open or close emphasis.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Delim {
    pub item: usize,
    ch: u8,
    can_open: bool,
    can_close: bool,
    count: usize,
    orig: usize,
    pub prev: Option<usize>,
    pub next: Option<usize>,
}

fn rune_before(s: &str, pos: usize) -> char {
    if pos == 0 { '\n' } else { decode_last_rune(s, pos).0 }
}

fn rune_at(s: &str, pos: usize) -> char {
    if pos >= s.len() { '\n' } else { decode_rune(s, pos).0 }
}

impl Inline<'_, '_, '_, '_> {
    /// Adds a run of `*` or `_` as text and records it on the delimiter stack when it
    /// can open or close emphasis.
    pub(super) fn handle_delim(&mut self, c: u8) -> bool {
        let s = self.subject;
        let start = self.pos;
        let end = start + s.as_bytes()[start..].iter().take_while(|&&x| x == c).count();
        let (before, after) = (rune_before(s, start), rune_at(s, end));
        let after_space = is_unicode_whitespace(after);
        let after_punct = is_unicode_punctuation(after);
        let before_space = is_unicode_whitespace(before);
        let before_punct = is_unicode_punctuation(before);
        let left = !after_space && (!after_punct || before_space || before_punct);
        let right = !before_space && (!before_punct || after_space || after_punct);
        let (mut can_open, mut can_close) = (left, right);
        if c == b'_' {
            can_open = left && (!right || before_punct);
            can_close = right && (!left || after_punct);
        }
        self.pos = end;
        let item = self.add_text_item(start, end);
        if !(can_open || can_close) {
            return true;
        }
        let id = self.delims.len();
        let n = end - start;
        self.delims.push(Delim {
            item,
            ch: c,
            can_open,
            can_close,
            count: n,
            orig: n,
            prev: self.top_delim,
            next: None,
        });
        if let Some(top) = self.top_delim {
            self.delims[top].next = Some(id);
        }
        self.top_delim = Some(id);
        true
    }

    pub(super) fn remove_delim(&mut self, id: usize) {
        let d = self.delims[id];
        if let Some(p) = d.prev {
            self.delims[p].next = d.next;
        }
        match d.next {
            None => self.top_delim = d.prev,
            Some(n) => self.delims[n].prev = d.prev,
        }
    }

    /// Resolves the delimiters above `bottom` into Emphasis and Strong.
    pub(super) fn process_emphasis(&mut self, bottom: Option<usize>) -> Result<(), Fail> {
        let mut openers_bottom = [[bottom; 6]; 2];
        let mut closer = self.top_delim;
        while let Some(c) = closer.filter(|&c| self.delims[c].prev != bottom) {
            closer = self.delims[c].prev;
        }
        while let Some(c) = closer {
            self.h.spend(1)?;
            let cd = self.delims[c];
            if !cd.can_close {
                closer = cd.next;
                continue;
            }
            let row = usize::from(cd.ch != b'*');
            let slot = if cd.can_open { 3 } else { 0 } + cd.orig % 3;
            let mut opener = cd.prev;
            let mut found = false;
            while let Some(o) = opener.filter(|&o| Some(o) != bottom)
                && opener != openers_bottom[row][slot]
            {
                self.h.spend(1)?;
                let od = self.delims[o];
                let odd = (cd.can_open || od.can_close)
                    && !cd.orig.is_multiple_of(3)
                    && (od.orig + cd.orig).is_multiple_of(3);
                if od.ch == cd.ch && od.can_open && !odd {
                    found = true;
                    break;
                }
                opener = od.prev;
            }
            if let (true, Some(o)) = (found, opener) {
                closer = self.apply_emphasis(o, c)?;
                continue;
            }
            closer = cd.next;
            openers_bottom[row][slot] = cd.prev;
            if !cd.can_open {
                self.remove_delim(c);
            }
        }
        while let Some(top) = self.top_delim.filter(|&t| Some(t) != bottom) {
            self.remove_delim(top);
        }
        Ok(())
    }

    /// Wraps the items between an opener and a closer in Emphasis or Strong and returns
    /// the closer to continue from.
    fn apply_emphasis(&mut self, opener: usize, closer: usize) -> Result<Option<usize>, Fail> {
        let (od, cd) = (self.delims[opener], self.delims[closer]);
        let used = if od.count >= 2 && cd.count >= 2 { 2 } else { 1 };
        let (oi, ci) = (od.item, cd.item);
        self.delims[opener].count -= used;
        self.delims[closer].count -= used;
        self.items[oi].text.len -= used;
        self.items[ci].text.len -= used;
        let start = self.items[oi].start + od.count - used;
        let end = self.items[ci].start + cd.count - used + used;
        let kind = if used == 2 { NodeKind::Strong } else { NodeKind::Emphasis };
        let emph = self.new_item(kind, start, end);
        let mut tmp = self.items[oi].next;
        while let Some(t) = tmp.filter(|&t| t != ci) {
            self.h.spend(1)?;
            let next = self.items[t].next;
            self.unlink_item(t);
            self.append_child(emph, t);
            tmp = next;
        }
        self.insert_after(oi, emph);
        if od.next != Some(closer) {
            self.delims[opener].next = Some(closer);
            self.delims[closer].prev = Some(opener);
        }
        if self.delims[opener].count == 0 {
            self.unlink_item(oi);
            self.remove_delim(opener);
        }
        if self.delims[closer].count == 0 {
            self.unlink_item(ci);
            let next = self.delims[closer].next;
            self.remove_delim(closer);
            return Ok(next);
        }
        Ok(Some(closer))
    }
}
