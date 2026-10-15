//! End-strings: where each construct may end, with a per-kind search cache.

use guidedoc::{Arg, Fail, Message, arg_span, arg_static};

use crate::chars::is_space;
use crate::messages::UNTERMINATED;
use crate::scan::{EndCache, EndKind, MARK_BLOCKED, MARK_BOUNDARY, Scanner};

/// The rest of an interpreted text end-string after its backquote.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InterpretedEnd {
    /// End of the whole end-string.
    pub end: usize,
    /// The suffix role name's range.
    pub role: Option<(usize, usize)>,
    pub underscores: usize,
}

impl EndKind {
    fn index(self) -> usize {
        self as usize
    }

    fn first_byte(self) -> u8 {
        match self {
            EndKind::Emphasis | EndKind::Strong => b'*',
            EndKind::Literal | EndKind::Target | EndKind::Interpreted => b'`',
            EndKind::Substitution => b'|',
        }
    }
}

impl Scanner<'_, '_> {
    /// The end-string at `j` follows neither white space nor an escape.
    fn plain_before(&self, j: usize) -> bool {
        if j > 0 && self.bytes()[j - 1] == 0 {
            return false;
        }
        !self.space_before(j)
    }

    /// Interpreted text may end after an escaped space, but not after unescaped white
    /// space or an escaping backslash.
    fn interpreted_before(&self, j: usize) -> bool {
        let (r, n) = self.rune_before(j);
        if n == 0 {
            return true;
        }
        if !is_space(r) && !(n == 1 && self.bytes()[j - 1] == 0) {
            return true;
        }
        j > n && self.bytes()[j - n - 1] == 0
    }

    /// Matches an optional "_" or "__" at `p` followed by an end context; returns the
    /// end and the number of underscores.
    pub fn refend_at(&self, p: usize) -> Option<(usize, usize)> {
        let n = self.len();
        if self.byte_at(p) == b'_' && p < n {
            if self.byte_at(p + 1) == b'_' && p + 1 < n && self.end_ok(p + 2, n) {
                return Some((p + 2, 2));
            }
            if self.end_ok(p + 1, n) {
                return Some((p + 1, 1));
            }
        }
        if self.end_ok(p, n) {
            return Some((p, 0));
        }
        None
    }

    /// Matches the rest of an interpreted text end-string after the backquote at `j`:
    /// an optional ":role:", then an optional "_" or "__".
    pub fn interpreted_suffix(&self, j: usize) -> Option<InterpretedEnd> {
        let after = j + 1;
        if self.byte_at(after) == b':' && after < self.len() {
            let r1 = after + 1;
            let r2 = self.name_end(r1);
            if r2 > r1
                && self.byte_at(r2) == b':'
                && r2 < self.len()
                && let Some((end, underscores)) = self.refend_at(r2 + 1)
            {
                return Some(InterpretedEnd { end, role: Some((r1, r2)), underscores });
            }
        }
        let (end, underscores) = self.refend_at(after)?;
        Some(InterpretedEnd { end, role: None, underscores })
    }

    /// Tests for the end-string of `kind` at `j` and returns where it ends. Without
    /// `before`, the rule about the preceding character is not applied: a search begins
    /// right after the start-string, where that character belongs to the start-string.
    pub fn end_at(&self, kind: EndKind, j: usize, before: bool) -> Option<usize> {
        let n = self.len();
        if j >= n {
            return None;
        }
        let c = self.byte_at(j);
        let (end, ok) = match kind {
            EndKind::Emphasis | EndKind::Target => {
                let mark = if kind == EndKind::Emphasis { b'*' } else { b'`' };
                if c != mark || (before && !self.plain_before(j)) {
                    return None;
                }
                (j + 1, self.end_ok(j + 1, n))
            }
            EndKind::Strong => {
                if c != b'*' || self.byte_at(j + 1) != b'*' || j + 1 >= n {
                    return None;
                }
                if before && !self.plain_before(j) {
                    return None;
                }
                (j + 2, self.end_ok(j + 2, n))
            }
            EndKind::Literal => {
                if c != b'`' || self.byte_at(j + 1) != b'`' || j + 1 >= n {
                    return None;
                }
                if before && self.space_before(j) {
                    return None;
                }
                (j + 2, self.end_ok(j + 2, n))
            }
            EndKind::Substitution => return self.substitution_end(j, before),
            EndKind::Interpreted => {
                if c != b'`' || (before && !self.interpreted_before(j)) {
                    return None;
                }
                return self.interpreted_suffix(j).map(|e| e.end);
            }
        };
        ok.then_some(end)
    }

    fn substitution_end(&self, j: usize, before: bool) -> Option<usize> {
        if self.byte_at(j) != b'|' || (before && !self.plain_before(j)) {
            return None;
        }
        (0..=2usize).rev().find_map(|u| {
            let ok = self.underscores_at(j + 1, u) && self.end_ok(j + 1 + u, self.len());
            ok.then_some(j + 1 + u)
        })
    }

    fn underscores_at(&self, p: usize, count: usize) -> bool {
        self.bytes().get(p..p + count).is_some_and(|s| s.iter().all(|&c| c == b'_'))
    }

    /// Returns the first end-string of `kind` at or after `from`.
    fn find_end(&mut self, kind: EndKind, from: usize) -> Result<Option<usize>, Fail> {
        let c = self.cache[kind.index()];
        if c.valid && from >= c.from && c.found.is_none_or(|f| from <= f) {
            return Ok(c.found);
        }
        let mut found = None;
        let first = kind.first_byte();
        for j in from..self.len() {
            if self.bytes()[j] != first {
                continue;
            }
            self.ctx.b.spend(1)?;
            if self.end_at(kind, j, true).is_some() {
                found = Some(j);
                break;
            }
        }
        let scanned = found.unwrap_or(self.len()).saturating_sub(from);
        self.ctx.b.spend(scanned as u64 / 16 + 1)?;
        self.cache[kind.index()] = EndCache { from, found, valid: true };
        Ok(found)
    }

    /// Finds the end-string for content starting at `q`: its start and end. As in the
    /// specification, an end-string right after the start-string ends the search.
    pub fn find_obj(&mut self, kind: EndKind, q: usize) -> Result<Option<(usize, usize)>, Fail> {
        if self.end_at(kind, q, false).is_some() {
            return Ok(None);
        }
        let Some(j) = self.find_end(kind, q + 1)? else {
            return Ok(None);
        };
        Ok(self.end_at(kind, j, true).map(|e| (j, e)))
    }

    /// Reports a start-string without end-string and keeps it as text.
    pub fn unterminated(&mut self, p: usize, q: usize, what: &'static str) -> Option<usize> {
        let span = self.span(p, q);
        self.ctx.report(UNTERMINATED, span, arg_static(what), arg_span(span));
        self.problematic(p, q);
        Some(q)
    }

    /// Reports `m` about `[rs, e)`, which stays text.
    pub fn mismatch(&mut self, m: Message, rs: usize, e: usize) -> Option<usize> {
        let span = self.span(rs, e);
        self.ctx.report(m, span, arg_span(span), Arg::None);
        self.problematic(rs, e);
        Some(e)
    }

    /// Marks `[p, q)` as a separate piece of text for URI recognition.
    pub fn problematic(&mut self, p: usize, q: usize) {
        self.marks[p] |= MARK_BOUNDARY | MARK_BLOCKED;
        self.marks[q] |= MARK_BOUNDARY;
    }
}
