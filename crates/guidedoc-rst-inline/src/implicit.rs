//! Implicit PEP and RFC references (`Options::pep_references`, `rfc_references`):
//! "PEP" or "RFC", white space, and a number, as whole words in text outside inline
//! markup, link to the document, as the `:PEP:` and `:RFC:` roles do.

use guidedoc::{Fail, LinkInfo, LinkKind, NodeKind};

use crate::chars::is_word;
use crate::emit::BreakMode;
use crate::roles::{PEP_URL, RFC_URL, push_int};
use crate::scan::Scanner;

impl Scanner<'_, '_> {
    /// Writes the URL of PEP `n`: the default base gives
    /// `https://peps.python.org/pep-0008/`, a configured base `<base>pep-0008`.
    pub fn push_pep_url(&mut self, n: i64) -> Result<(), Fail> {
        let base = self.state.options.pep_base_url.as_str();
        let b = &mut self.ctx.b;
        b.text_push(if base.is_empty() { PEP_URL } else { base })?;
        if !base.is_empty() {
            b.text_push("pep-")?;
        }
        push_int(b, n, 4)?;
        if base.is_empty() {
            b.text_push_byte(b'/')?;
        }
        Ok(())
    }

    /// Writes the URL of RFC `n` and a "#section" fragment: the default base gives
    /// `https://datatracker.ietf.org/doc/html/rfc2822`, a configured base
    /// `<base>rfc2822.html`.
    pub fn push_rfc_url(&mut self, n: i64, fragment: &str) -> Result<(), Fail> {
        let base = self.state.options.rfc_base_url.as_str();
        let b = &mut self.ctx.b;
        b.text_push(if base.is_empty() { RFC_URL } else { base })?;
        if !base.is_empty() {
            b.text_push("rfc")?;
        }
        push_int(b, n, 0)?;
        if !base.is_empty() {
            b.text_push(".html")?;
        }
        b.text_push(fragment)
    }

    /// Matches "PEP n" or "RFC n" at `p`: the end, the number, and whether it is a PEP.
    pub fn match_implicit(&self, p: usize, bound: usize) -> Option<(usize, i64, bool)> {
        let o = &self.state.options;
        if (!o.pep_references && !o.rfc_references) || p + 5 > bound {
            return None;
        }
        let t = self.bytes();
        let word = &t[p..p + 3];
        let pep = word == b"PEP";
        if !(pep && o.pep_references) && !(word == b"RFC" && o.rfc_references) {
            return None;
        }
        let (before, w) = self.rune_before(p);
        if w > 0 && is_word(before) {
            return None;
        }
        let mut k = p + 3;
        while k < bound && matches!(t[k], b' ' | b'\n' | b'\t') {
            k += 1;
        }
        if k == p + 3 {
            return None;
        }
        let mut d = k;
        while d < bound && t[d].is_ascii_digit() && d - k < 12 {
            d += 1;
        }
        if d == k {
            return None;
        }
        let (after, w) = self.rune_at(d);
        if w > 0 && (is_word(after) || after == '_') {
            return None;
        }
        let n = t[k..d].iter().fold(0i64, |n, &c| n * 10 + i64::from(c - b'0'));
        if pep && n > 9999 {
            return None;
        }
        Some((d, n, pep))
    }

    /// Emits the link for `[p, e)`.
    pub fn emit_implicit(&mut self, p: usize, e: usize, n: i64, pep: bool) -> Result<(), Fail> {
        let start = self.ctx.b.text_open();
        if pep {
            self.push_pep_url(n)?;
        } else {
            self.push_rfc_url(n, "")?;
        }
        let dest = self.ctx.b.text_close(start);
        let id = self.ctx.b.begin_node(NodeKind::Link, self.span(p, e))?;
        let link =
            LinkInfo { destination: dest, kind: LinkKind::External, ..LinkInfo::UNRESOLVED };
        self.ctx.b.set_link(id, link)?;
        self.emit_cooked(p, e, BreakMode::Breaks)?;
        self.ctx.b.end_node(id)
    }
}
