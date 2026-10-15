//! Inline markup recognition, as the reStructuredText specification's "Inline markup
//! recognition rules" describe. The leftmost start-string wins; its end-string is the
//! first valid one after it. Text between constructs is scanned for standalone
//! hyperlinks.

use guidedoc::{Fail, LinkInfo, NodeFlags, NodeId, NodeKind, ReadContext, Text};

use crate::emit::{BreakMode, check};
use crate::extension::RoleHook;
use crate::scan::{EndKind, Scanner};
use crate::{InlineState, Line};

/// Parses `lines` into the builder's open node (see `parse_inline`).
pub(crate) fn parse_inline_lines(
    ctx: &mut ReadContext<'_>,
    state: &mut InlineState,
    lines: &[Line<'_>],
    hook: &mut dyn RoleHook,
) -> Result<(), Fail> {
    check(&mut ctx.b)?;
    if lines.is_empty() {
        return Ok(());
    }
    let buffers = std::mem::take(&mut state.buffers);
    let mut sc = Scanner::new(ctx, state, hook, lines, buffers, true)?;
    let result = scan_all(&mut sc);
    let buffers = sc.finish();
    state.buffers = buffers;
    result
}

fn scan_all(sc: &mut Scanner<'_, '_>) -> Result<(), Fail> {
    let n = sc.len();
    let mut p = 0;
    while p < n {
        sc.ctx.b.spend(1)?;
        match try_at(sc, p)? {
            Some(next) => {
                p = next;
                sc.base = next;
            }
            None => p += sc.rune_at(p).1.max(1),
        }
    }
    sc.flush(n)
}

/// Tries every construct that may start at `p`, in the specification's order. `Some`
/// means the text from the returned position on is parsed afresh, either because a
/// construct was emitted or because a start-string was kept as text.
fn try_at(sc: &mut Scanner<'_, '_>, p: usize) -> Result<Option<usize>, Fail> {
    let c = sc.bytes()[p];
    let marker = matches!(c, b'*' | b'`' | b'_' | b'|' | b'[' | b':');
    if !marker && c < 0x80 && !c.is_ascii_alphanumeric() {
        return Ok(None);
    }
    if !sc.start_ok(p, sc.base) {
        return Ok(None);
    }
    match c {
        b'*' => star(sc, p),
        b'`' if sc.byte_at(p + 1) == b'`' => literal(sc, p),
        b'`' => sc.interpreted(p, p, None),
        b'_' => inline_target(sc, p),
        b'|' => substitution(sc, p),
        b'[' => footnote_reference(sc, p),
        b':' => sc.role_prefix(p),
        _ => simple_reference(sc, p),
    }
}

fn star(sc: &mut Scanner<'_, '_>, p: usize) -> Result<Option<usize>, Fail> {
    let strong = sc.byte_at(p + 1) == b'*';
    let q = if strong { p + 2 } else { p + 1 };
    if sc.space_after(q) {
        return Ok(None);
    }
    if sc.quoted(p, q) {
        return Ok(Some(q));
    }
    let kind = if strong { EndKind::Strong } else { EndKind::Emphasis };
    let Some((j, e)) = sc.find_obj(kind, q)? else {
        let what = if strong { "strong emphasis" } else { "emphasis" };
        return Ok(sc.unterminated(p, q, what));
    };
    sc.flush(p)?;
    let node = if strong { NodeKind::Strong } else { NodeKind::Emphasis };
    let id = sc.ctx.b.begin_node(node, sc.span(p, e))?;
    sc.emit_cooked(q, j, BreakMode::Breaks)?;
    sc.ctx.b.end_node(id)?;
    sc.pos = e;
    Ok(Some(e))
}

fn literal(sc: &mut Scanner<'_, '_>, p: usize) -> Result<Option<usize>, Fail> {
    let q = p + 2;
    if sc.space_after(q) {
        return Ok(None);
    }
    if sc.quoted(p, q) {
        return Ok(Some(q));
    }
    let Some((j, e)) = sc.find_obj(EndKind::Literal, q)? else {
        return Ok(sc.unterminated(p, q, "inline literal"));
    };
    sc.flush(p)?;
    let start = sc.ctx.b.text_open();
    sc.push_raw(q, j)?;
    let t = sc.ctx.b.text_close(start);
    sc.ctx.b.add_leaf(NodeKind::Code, sc.span(p, e), t)?;
    sc.pos = e;
    Ok(Some(e))
}

fn inline_target(sc: &mut Scanner<'_, '_>, p: usize) -> Result<Option<usize>, Fail> {
    if sc.byte_at(p + 1) != b'`' {
        return Ok(None);
    }
    let q = p + 2;
    if sc.space_after(q) {
        return Ok(None);
    }
    if sc.quoted(p, q) {
        return Ok(Some(q));
    }
    let Some((j, e)) = sc.find_obj(EndKind::Target, q)? else {
        return Ok(sc.unterminated(p, q, "inline target"));
    };
    sc.flush(p)?;
    let id = sc.ctx.b.begin_node(NodeKind::InlineTarget, sc.span(p, e))?;
    let name = sc.name_text(q, j, true)?;
    sc.ctx.b.set_text(id, name);
    sc.emit_cooked(q, j, BreakMode::Breaks)?;
    sc.ctx.b.end_node(id)?;
    sc.pos = e;
    Ok(Some(e))
}

/// An unresolved link row naming `name`.
pub(crate) fn unresolved(name: Text) -> LinkInfo {
    LinkInfo { name, ..LinkInfo::UNRESOLVED }
}

/// Handles `|name|`, and `|name|_` or `|name|__`, which wrap the substitution reference
/// in a hyperlink reference.
fn substitution(sc: &mut Scanner<'_, '_>, p: usize) -> Result<Option<usize>, Fail> {
    let q = p + 1;
    if sc.byte_at(q) == b'|' || sc.space_after(q) {
        return Ok(None);
    }
    if sc.quoted(p, q) {
        return Ok(Some(q));
    }
    let Some((j, e)) = sc.find_obj(EndKind::Substitution, q)? else {
        return Ok(sc.unterminated(p, q, "substitution reference"));
    };
    sc.flush(p)?;
    let underscores = e - j - 1;
    let mut wrap = None;
    if underscores > 0 {
        let name = if underscores == 1 { sc.name_text(q, j, true)? } else { Text::default() };
        wrap = Some(sc.reference_node(p, e, name, underscores == 2)?);
    }
    let written = sc.name_text(q, j, false)?;
    let id = sc.ctx.b.add_leaf(NodeKind::SubstitutionReference, sc.span(p, j + 1), written)?;
    let name = sc.name_text(q, j, true)?;
    sc.ctx.b.set_link(id, unresolved(name))?;
    if let Some(wrap) = wrap {
        sc.ctx.b.end_node(wrap)?;
    }
    sc.pos = e;
    Ok(Some(e))
}

impl Scanner<'_, '_> {
    /// Opens a Reference whose link row awaits resolution.
    pub fn reference_node(
        &mut self,
        p: usize,
        e: usize,
        name: Text,
        anonymous: bool,
    ) -> Result<NodeId, Fail> {
        let id = self.ctx.b.begin_node(NodeKind::Reference, self.span(p, e))?;
        if anonymous {
            self.ctx.b.add_flags(id, NodeFlags::ANONYMOUS);
        }
        self.ctx.b.set_link(id, unresolved(name))?;
        Ok(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelKind {
    Number,
    Auto,
    Symbol,
    Citation,
}

/// Matches the label of `[1]_`, `[#]_`, `[#name]_`, `[*]_`, or `[CIT2002]_` at `q` and
/// returns the position of the closing bracket.
fn footnote_label(sc: &Scanner<'_, '_>, q: usize) -> Option<(LabelKind, usize)> {
    let mut d = q;
    while d < sc.len() && sc.byte_at(d).is_ascii_digit() {
        d += 1;
    }
    if d > q && sc.byte_at(d) == b']' {
        return Some((LabelKind::Number, d));
    }
    match sc.byte_at(q) {
        b'#' => {
            let l = sc.name_end(q + 1);
            if sc.byte_at(l) == b']' {
                return Some((LabelKind::Auto, l));
            }
        }
        b'*' if sc.byte_at(q + 1) == b']' => return Some((LabelKind::Symbol, q + 1)),
        _ => {}
    }
    let l = sc.name_end(q);
    (l > q && sc.byte_at(l) == b']').then_some((LabelKind::Citation, l))
}

fn footnote_reference(sc: &mut Scanner<'_, '_>, p: usize) -> Result<Option<usize>, Fail> {
    let q = p + 1;
    let Some((kind, l)) = footnote_label(sc, q) else {
        return Ok(None);
    };
    if sc.byte_at(l + 1) != b'_' || !sc.end_ok(l + 2, sc.len()) {
        return Ok(None);
    }
    let e = l + 2;
    sc.flush(p)?;
    let start = sc.ctx.b.text_open();
    sc.ctx.b.text_push_bytes(&sc.text.as_bytes()[q..l])?;
    let label = sc.ctx.b.text_close(start);
    let node_kind = if kind == LabelKind::Citation {
        NodeKind::CitationReference
    } else {
        NodeKind::FootnoteReference
    };
    let id = sc.ctx.b.add_leaf(node_kind, sc.span(p, e), label)?;
    let name = match kind {
        LabelKind::Auto => {
            sc.ctx.b.add_flags(id, NodeFlags::AUTO);
            sc.name_text(q + 1, l, true)?
        }
        LabelKind::Symbol => {
            sc.ctx.b.add_flags(id, NodeFlags::SYMBOL);
            Text::default()
        }
        _ => sc.name_text(q, l, true)?,
    };
    sc.ctx.b.set_link(id, unresolved(name))?;
    sc.pos = e;
    Ok(Some(e))
}

/// Handles `name_` and `name__`. The name is the longest simplename followed by the
/// underscores and an end context.
fn simple_reference(sc: &mut Scanner<'_, '_>, p: usize) -> Result<Option<usize>, Fail> {
    if p < sc.no_ref {
        return Ok(None);
    }
    let mut best = None;
    let mut last = p;
    let mut i = p;
    loop {
        let j = sc.word_end(i);
        if j == i {
            break;
        }
        last = j;
        if let Some((e, u)) = sc.refend_at(j)
            && u > 0
        {
            best = Some((j, e));
        }
        if !crate::chars::is_name_separator(sc.byte_at(j)) {
            break;
        }
        let (r, n) = sc.rune_at(j + 1);
        if n == 0 || !crate::chars::is_word(r) {
            break;
        }
        i = j + 1;
    }
    sc.ctx.b.spend((last - p) as u64 / 16 + 1)?;
    let Some((best, end)) = best else {
        sc.no_ref = last;
        return Ok(None);
    };
    sc.flush(p)?;
    let anonymous = end - best == 2;
    let name = if anonymous { Text::default() } else { sc.name_text(p, best, true)? };
    let id = sc.reference_node(p, end, name, anonymous)?;
    sc.emit_cooked(p, best, BreakMode::Breaks)?;
    sc.ctx.b.end_node(id)?;
    sc.pos = end;
    Ok(Some(end))
}
