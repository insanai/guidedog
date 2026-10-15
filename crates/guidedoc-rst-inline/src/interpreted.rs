//! Interpreted text, phrase references, and embedded URIs and aliases.

use guidedoc::{AttributeKey, Fail, LinkInfo, LinkKind, NodeKind, ReadContext, Text};

use crate::emit::{BreakMode, check, pool_name};
use crate::extension::RoleHook;
use crate::markup::unresolved;
use crate::messages::{MULTIPLE_ROLES, ROLE_AND_REFERENCE};
use crate::scan::{EndKind, Scanner};
use crate::{InlineState, Line, TARGET_NAME_ATTRIBUTE};

/// `text <uri>` or `text <name_>` at the end of a phrase.
#[derive(Debug, Clone, Copy)]
struct Embedded {
    /// End of the reference text before the white space and "<".
    text_end: usize,
    /// Between the angle brackets.
    alias: (usize, usize),
}

impl Scanner<'_, '_> {
    /// Handles ":role:`text`". The role name is a simplename.
    pub fn role_prefix(&mut self, p: usize) -> Result<Option<usize>, Fail> {
        let r1 = p + 1;
        let r2 = self.name_end(r1);
        if r2 == r1 || self.byte_at(r2) != b':' || self.byte_at(r2 + 1) != b'`' {
            return Ok(None);
        }
        if self.byte_at(r2 + 2) == b'`' {
            return Ok(None);
        }
        self.interpreted(p, r2 + 1, Some((r1, r2)))
    }

    /// Handles interpreted text and phrase references whose backquote is at `p`. `rs`
    /// is where the construct starts: the role prefix, or `p` itself.
    pub fn interpreted(
        &mut self,
        rs: usize,
        p: usize,
        role: Option<(usize, usize)>,
    ) -> Result<Option<usize>, Fail> {
        let q = p + 1;
        if self.space_after(q) {
            return Ok(None);
        }
        if role.is_none() && self.quoted(p, q) {
            return Ok(Some(q));
        }
        let Some((j, _)) = self.find_obj(EndKind::Interpreted, q)? else {
            return Ok(self.unterminated(p, q, "interpreted text or phrase reference"));
        };
        let Some(end) = self.interpreted_suffix(j) else {
            return Ok(None);
        };
        let e = end.end;
        if role.is_some() && end.role.is_some() {
            return Ok(self.mismatch(MULTIPLE_ROLES, rs, e));
        }
        if end.underscores > 0 {
            if role.is_some() || end.role.is_some() {
                return Ok(self.mismatch(ROLE_AND_REFERENCE, rs, e));
            }
            self.flush(p)?;
            self.phrase_reference(p, e, q, j, end.underscores == 2)?;
            self.pos = e;
            return Ok(Some(e));
        }
        self.flush(rs)?;
        let name = role
            .or(end.role)
            .map_or(String::new(), |(a, b)| self.text.get(a..b).unwrap_or("").to_owned());
        self.apply_role(&name, rs, e, q, j)?;
        self.pos = e;
        Ok(Some(e))
    }

    /// Finds "text <uri>" or "text <name_>" at the end of phrase `[q, j)`: an unescaped
    /// "<" after white space (or at the start), a non-empty body without unescaped
    /// angle brackets, and a final ">".
    fn embedded(&self, q: usize, j: usize) -> Option<Embedded> {
        let t = self.bytes();
        if j - q < 3 || t[j - 1] != b'>' {
            return None;
        }
        if matches!(t[j - 2], b' ' | b'\n' | 0) {
            return None;
        }
        let mut k = j - 2;
        loop {
            if (t[k] == b'<' || t[k] == b'>') && (k == q || t[k - 1] != 0) {
                break;
            }
            if k == q {
                return None;
            }
            k -= 1;
        }
        if t[k] != b'<' || k + 1 >= j - 1 || matches!(t[k + 1], b' ' | b'\n') {
            return None;
        }
        let mut s = k;
        while s > q && matches!(t[s - 1], b' ' | b'\n') {
            s -= 1;
        }
        if s == k && k != q {
            return None;
        }
        Some(Embedded { text_end: s, alias: (k + 1, j - 1) })
    }

    /// Emits `` `text`_ `` or `` `text`__ `` at `[p, e)` with content `[q, j)`.
    fn phrase_reference(
        &mut self,
        p: usize,
        e: usize,
        q: usize,
        j: usize,
        anonymous: bool,
    ) -> Result<(), Fail> {
        let Some(emb) = self.embedded(q, j) else {
            let name = if anonymous { Text::default() } else { self.name_text(q, j, true)? };
            let id = self.reference_node(p, e, name, anonymous)?;
            self.emit_cooked(q, j, BreakMode::Breaks)?;
            return self.ctx.b.end_node(id);
        };
        let (a0, a1) = emb.alias;
        let t = self.bytes();
        let alias_name = t[a1 - 1] == b'_' && t[a1 - 2] != 0 && !self.uri_matches_at(a0, a1);
        if alias_name {
            self.embedded_alias(p, e, q, emb, anonymous)
        } else {
            self.embedded_uri(p, e, q, emb, anonymous)
        }
    }

    /// Emits `` `text <name_>`_ ``: a reference to name. Without "__" the text also
    /// names an indirect target to name, recorded as the custom attribute
    /// `target-name` for resolution.
    fn embedded_alias(
        &mut self,
        p: usize,
        e: usize,
        q: usize,
        emb: Embedded,
        anonymous: bool,
    ) -> Result<(), Fail> {
        let (a0, a1) = emb.alias;
        let alias = self.name_text(a0, a1 - 1, true)?;
        let id = self.ctx.b.begin_node(NodeKind::Reference, self.span(p, e))?;
        self.ctx.b.set_link(id, unresolved(alias))?;
        if emb.text_end > q {
            if !anonymous {
                let key = self.ctx.b.add_text(TARGET_NAME_ATTRIBUTE)?;
                let name = self.name_text(q, emb.text_end, true)?;
                self.ctx.b.add_attribute_named(id, AttributeKey::Custom, name, key)?;
            }
            self.emit_cooked(q, emb.text_end, BreakMode::Breaks)?;
        } else {
            self.ctx.b.add_leaf(NodeKind::Text, self.span(a0, a1), alias)?;
        }
        self.ctx.b.end_node(id)
    }

    /// Emits `` `text <uri>`_ `` as a Link; without "__" the text names a target.
    /// Unescaped white space in the URI is removed; escaped white space becomes a space.
    fn embedded_uri(
        &mut self,
        p: usize,
        e: usize,
        q: usize,
        emb: Embedded,
        anonymous: bool,
    ) -> Result<(), Fail> {
        let (a0, a1) = emb.alias;
        let start = self.ctx.b.text_open();
        let mut i = a0;
        while i < a1 {
            let c = self.bytes()[i];
            i += 1;
            if c == 0 {
                if i < a1 && matches!(self.bytes()[i], b' ' | b'\n') {
                    self.ctx.b.text_push_byte(b' ')?;
                    i += 1;
                }
                continue;
            }
            if !matches!(c, b' ' | b'\n' | b'\t') {
                self.ctx.b.text_push_byte(c)?;
            }
        }
        let dest = self.ctx.b.text_close(start);
        let email = self.email_end(a0, a1).0 == Some(a1);
        let kind = if email { LinkKind::Email } else { LinkKind::External };
        let mut name = Text::default();
        if !anonymous {
            name = if emb.text_end > q {
                self.name_text(q, emb.text_end, true)?
            } else {
                let written = self.ctx.b.pool_string(dest).to_owned();
                pool_name(&mut self.ctx.b, &written, true)?
            };
        }
        let id = self.ctx.b.begin_node(NodeKind::Link, self.span(p, e))?;
        self.ctx
            .b
            .set_link(id, LinkInfo { destination: dest, name, kind, ..LinkInfo::UNRESOLVED })?;
        if emb.text_end > q {
            self.emit_cooked(q, emb.text_end, BreakMode::Breaks)?;
        } else {
            self.ctx.b.add_leaf(NodeKind::Text, self.span(a0, a1), dest)?;
        }
        self.ctx.b.end_node(id)
    }
}

/// See `crate::apply_host_role`.
pub(crate) fn apply_host_role(
    ctx: &mut ReadContext<'_>,
    state: &mut InlineState,
    hook: &mut dyn RoleHook,
    name: &str,
    lines: &[Line<'_>],
    content: std::ops::Range<usize>,
) -> Result<(), Fail> {
    check(&mut ctx.b)?;
    let buffers = std::mem::take(&mut state.buffers);
    let mut sc = Scanner::new(ctx, state, hook, lines, buffers, false)?;
    let e = sc.len();
    let q = content.start.min(e);
    let j = content.end.clamp(q, e);
    let result = sc.apply_role(name, 0, e, q, j);
    state.buffers = sc.finish();
    result
}
