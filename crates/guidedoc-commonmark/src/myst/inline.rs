//! Inline syntax of MyST: roles, footnote references, substitutions, and dollar math;
//! and how links resolve: to URLs, heading anchors, or cross-references for the host.

use guidedoc::{Fail, LinkInfo, LinkKind, NodeId, NodeKind, SourceId, SourceSpan};

use super::notes::emit_footnote_reference;
use super::subs::expand_substitution;
use super::{SubRef, relocate, trim_space};
use crate::blocks::{Block, BlockKind, Seg, Skeleton};
use crate::extension::{Line, Role};
use crate::host::Host;
use crate::inline::{Inline, Item, Txt, parse_leaf_range};
use crate::messages::ROLE_UNKNOWN;
use crate::settings::{Rule, SyntaxExtension};

/// The longest role name read.
const MAX_ROLE_NAME: usize = 36;

impl<'k> Inline<'_, '_, '_, 'k> {
    /// Reads `[^label]`.
    pub(crate) fn parse_footnote_ref(&mut self) -> bool {
        if !self.h.on(Rule::FootnoteRef) {
            return false;
        }
        let s = self.subject.as_bytes();
        let i = self.pos;
        if s.get(i + 1) != Some(&b'^') {
            return false;
        }
        let Some(e) = s[i + 2..].iter().position(|&c| matches!(c, b']' | b' ' | b'\t' | b'\n'))
        else {
            return false;
        };
        let e = i + 2 + e;
        if s[e] != b']' || e == i + 2 {
            return false;
        }
        let id = self.add_item(NodeKind::FootnoteReference, i, e + 1);
        self.items[id].aux = (i + 2, e);
        self.pos = e + 1;
        true
    }

    /// Reads ``{name}`content` `` with any number of backticks around the content.
    pub(crate) fn parse_role(&mut self) -> Result<bool, Fail> {
        if !self.h.on(Rule::MystRole) {
            return Ok(false);
        }
        let s = self.subject.as_bytes();
        let mut i = self.pos + 1;
        while i < s.len() && i - self.pos <= MAX_ROLE_NAME + 1 {
            let c = s[i];
            if !(c.is_ascii_alphanumeric() || b"_-+:".contains(&c)) {
                break;
            }
            i += 1;
        }
        if i == self.pos + 1 || s.get(i) != Some(&b'}') || s.get(i + 1) != Some(&b'`') {
            return Ok(false);
        }
        let name_end = i;
        let t = i + 1;
        let n = s[t..].iter().take_while(|&&c| c == b'`').count();
        let q = t + n;
        // No closing run from the memo on: later roles with the same run need not rescan.
        let slot = n.min(self.no_role_end.len()) - 1;
        if q >= self.no_role_end[slot] {
            return Ok(false);
        }
        let mut j = q;
        while j < s.len() {
            if s[j] == b'\n' && s.get(j + 1) == Some(&b'\n') {
                return Ok(false);
            }
            self.h.spend(1)?;
            if s[j] != b'`' {
                j += 1;
                continue;
            }
            let k = j + s[j..].iter().take_while(|&&c| c == b'`').count();
            if k - j == n && j > q {
                let id = self.add_item(NodeKind::Role, self.pos, k);
                self.items[id].aux = (self.pos + 1, name_end);
                self.items[id].content = (q, j);
                self.pos = k;
                return Ok(true);
            }
            j = k;
        }
        if n <= self.no_role_end.len() {
            self.no_role_end[slot] = self.no_role_end[slot].min(q);
        }
        Ok(false)
    }

    /// Reads `{{ expression }}` (`Settings::sub_delimiters`).
    pub(crate) fn parse_substitution(&mut self) -> Result<bool, Fail> {
        if !self.h.has(SyntaxExtension::Substitution) {
            return Ok(false);
        }
        let [o, c] = self.h.settings.sub_delimiters;
        let s = self.subject.as_bytes();
        let i = self.pos;
        if s[i] != o || s.get(i + 1) != Some(&o) || i >= self.no_sub_end {
            return Ok(false);
        }
        let mut j = i + 2;
        while j + 1 < s.len() {
            if s[j] == b'\n' && s[j + 1] == b'\n' {
                return Ok(false);
            }
            self.h.spend(1)?;
            if s[j] == c && s[j + 1] == c {
                let id = self.add_item(NodeKind::SubstitutionReference, i, j + 2);
                self.items[id].aux = (i + 2, j);
                self.pos = j + 2;
                return Ok(true);
            }
            j += 1;
        }
        // No closing delimiters from here on: later openings need not rescan.
        self.no_sub_end = self.no_sub_end.min(i);
        Ok(false)
    }

    /// Reads `$math$` (and `$$math$$` inline with `dmath_double_inline`).
    pub(crate) fn parse_dollar_math(&mut self) -> Result<bool, Fail> {
        let settings = self.h.settings;
        let s = self.subject.as_bytes();
        let i = self.pos;
        let double = s.get(i + 1) == Some(&b'$');
        if double && !settings.dmath_double_inline {
            return Ok(false);
        }
        if !settings.dmath_allow_digits && i > 0 && s[i - 1].is_ascii_digit() {
            return Ok(false);
        }
        let open = if double { 2 } else { 1 };
        let q = i + open;
        let mut j = q;
        while j < s.len() {
            self.h.spend(1)?;
            if s[j] == b'\\' {
                j += 2;
                continue;
            }
            if s[j] == b'$' && (!double || s.get(j + 1) == Some(&b'$')) {
                break;
            }
            j += 1;
        }
        if j >= s.len() || j == q {
            return Ok(false);
        }
        if !settings.dmath_allow_space && (s[q] == b' ' || s[j - 1] == b' ') {
            return Ok(false);
        }
        let e = j + open;
        if !settings.dmath_allow_digits && s.get(e).is_some_and(u8::is_ascii_digit) {
            return Ok(false);
        }
        let id = self.add_item(NodeKind::Math, i, e);
        let subject = self.subject;
        self.items[id].text = self.push_text(&subject[q..j]);
        self.pos = e;
        Ok(true)
    }

    /// Subject range `[a, z)` as lines with source offsets.
    fn subject_lines(&self, a: usize, z: usize) -> Vec<Line<'k>> {
        let subject: &'k str = self.subject;
        let mut lines = Vec::new();
        let mut start = a;
        for (i, c) in
            subject[a..z].bytes().enumerate().map(|(k, c)| (a + k, c)).chain([(z, b'\n')])
        {
            if c != b'\n' {
                continue;
            }
            let at = self.sk.source_offset(self.blk, self.base + start);
            lines.push(Line { text: &subject[start..i], offset: at as u32 });
            start = i + 1;
        }
        lines
    }

    pub(crate) fn emit_myst_item(&mut self, it: Item) -> Result<(), Fail> {
        let span = self.item_span(it.start, it.end);
        let subject = self.subject;
        match it.kind {
            NodeKind::Math => {
                let text = self.str(it.text).to_owned();
                self.h.b().add_text_leaf(NodeKind::Math, span, &text).map(|_| ())
            }
            NodeKind::FootnoteReference => {
                emit_footnote_reference(self.h, &subject[it.aux.0..it.aux.1], span)
            }
            NodeKind::SubstitutionReference => {
                expand_substitution(self.h, &subject[it.aux.0..it.aux.1], span, false)
            }
            _ => self.emit_role(it, span),
        }
    }

    /// Runs a role through the extension; sub-ref is the reader's own.
    fn emit_role(&mut self, it: Item, span: SourceSpan) -> Result<(), Fail> {
        let subject = self.subject;
        let name = &subject[it.aux.0..it.aux.1];
        let (q, j) = it.content;
        if name == "sub-ref" {
            let key = trim_space(&subject[q..j]);
            if let Some(k) = SubRef::ALL.into_iter().find(|k| k.name() == key)
                && !self.h.ms.sub_refs.contains(&k)
            {
                self.h.ms.sub_refs.push(k);
            }
            let b = self.h.b();
            let id = b.add_text_leaf(NodeKind::SubstitutionReference, span, key)?;
            b.set_link(id, LinkInfo::UNRESOLVED)?;
            return Ok(());
        }
        let Some(ext) = self.h.ext() else {
            let b = self.h.b();
            let id = b.begin_node(NodeKind::Role, span)?;
            let t = b.add_text(name)?;
            b.set_text(id, t);
            b.add_text_leaf(NodeKind::Text, span, &subject[q..j])?;
            return b.end_node(id);
        };
        let start = it.start;
        let role = Role {
            name,
            lines: self.subject_lines(start, it.end),
            content: q - start..j - start,
            span,
            source: self.sk.source,
        };
        if ext.role(self.h, &role)? {
            return Ok(());
        }
        self.h.report_span(ROLE_UNKNOWN, span);
        self.h.b().add_text_leaf(NodeKind::Text, span, &subject[start..it.end]).map(|_| ())
    }

    /// Opens a link as MyST resolves it: a URL of an allowed scheme is a Link, a
    /// heading anchor an internal Link, a path to a document an Xref of type doc, and
    /// anything else an Xref of type any (label, document, term, or object).
    pub(crate) fn open_myst_link(&mut self, id: usize) -> Result<NodeId, Fail> {
        let it = self.items[id];
        let span = self.item_span(it.start, it.end);
        let raw = self.str(it.dest).to_owned();
        let scheme = url_scheme(&raw);
        if !scheme.is_empty() && scheme_allowed(self.h, scheme) {
            return self.plain_link(it, span);
        }
        let dest = relocate(self.h, self.sk.source, &raw, false);
        let (mut path, mut anchor) = match dest.split_once('#') {
            Some((p, a)) => (p, a),
            None => (dest.as_str(), ""),
        };
        while let Some(rest) = path.strip_prefix("./") {
            path = rest;
        }
        if path.is_empty()
            && !dest.is_empty()
            && self.h.settings.heading_anchors > 0
            && let Some(node) = self.anchor_link(it, anchor, span)?
        {
            return Ok(node);
        }
        if path.is_empty() && dest.starts_with('#') {
            return self.plain_link(it, span);
        }
        let (reftype, target) = match document_path(path) {
            Some(doc) => ("doc", doc),
            None => {
                anchor = "";
                ("any", dest.as_str())
            }
        };
        let b = self.h.b();
        let node = b.begin_node(NodeKind::Xref, span)?;
        b.add_custom_attribute(node, "reftype", reftype)?;
        if it.first.is_some() {
            b.add_custom_attribute(node, "refexplicit", "1")?;
        }
        if !anchor.is_empty() {
            b.add_custom_attribute(node, "anchor", anchor)?;
        }
        let name = b.add_text(target)?;
        b.set_link(node, LinkInfo { name, ..LinkInfo::UNRESOLVED })?;
        Ok(node)
    }

    fn plain_link(&mut self, it: Item, span: SourceSpan) -> Result<NodeId, Fail> {
        let dest = self.str(it.dest).to_owned();
        let title = it.title.map(|t: Txt| self.str(t).to_owned());
        let b = self.h.b();
        let node = b.begin_node(NodeKind::Link, span)?;
        let destination = b.add_text(&dest)?;
        let title = match title {
            Some(t) => b.add_text(&t)?,
            None => guidedoc::Text::default(),
        };
        b.set_link(
            node,
            LinkInfo { destination, title, kind: it.link_kind, ..LinkInfo::default() },
        )?;
        Ok(node)
    }

    /// Links to a heading anchor of this document; without text, the link shows the
    /// heading.
    fn anchor_link(
        &mut self,
        it: Item,
        anchor: &str,
        span: SourceSpan,
    ) -> Result<Option<NodeId>, Fail> {
        let Some(&index) = self.h.ms.slug_index.get(anchor) else { return Ok(None) };
        let title = self.h.ms.slugs[index].title.clone();
        let b = self.h.b();
        let node = b.begin_node(NodeKind::Link, span)?;
        let row =
            b.set_link(node, LinkInfo { kind: LinkKind::Internal, ..LinkInfo::UNRESOLVED })?;
        if it.first.is_none() {
            b.add_text_leaf(NodeKind::Text, span, &title)?;
        }
        self.h.ms.anchors.push((row, index));
        Ok(Some(node))
    }
}

/// The scheme of a URL ("https" of https://...), or "".
pub(crate) fn url_scheme(dest: &str) -> &str {
    let b = dest.as_bytes();
    if b.first().is_none_or(|c| !c.is_ascii_alphabetic()) {
        return "";
    }
    for (k, &c) in b.iter().enumerate().skip(1) {
        if c == b':' {
            return &dest[..k];
        }
        if !(c.is_ascii_alphanumeric() || b"+-.".contains(&c)) {
            return "";
        }
    }
    ""
}

fn scheme_allowed(h: &Host<'_, '_>, scheme: &str) -> bool {
    match &h.settings.url_schemes {
        None => true,
        Some(list) => list.iter().any(|s| s.eq_ignore_ascii_case(scheme)),
    }
}

/// Recognizes a path to a source document and returns it without its suffix, as :doc:
/// takes it.
fn document_path(path: &str) -> Option<&str> {
    [".md", ".rst", ".ipynb"]
        .into_iter()
        .find_map(|suffix| path.strip_suffix(suffix).filter(|p| !p.is_empty()))
}

/// Parses `lines` of `source` as inline content of the builder's open node.
pub(crate) fn inline_lines(
    h: &mut Host<'_, '_>,
    lines: &[Line<'_>],
    source: SourceId,
) -> Result<(), Fail> {
    let (Some(first), Some(last)) = (lines.first(), lines.last()) else { return Ok(()) };
    let text = h.ctx.source_of(source);
    let mut sk = Skeleton::new(source, &text, false, false);
    let mut blk = Block {
        kind: BlockKind::Paragraph,
        start: first.offset,
        end: last.offset + last.text.len() as u32,
        ..Block::default()
    };
    for l in lines {
        sk.segs.push(Seg { content: sk.arena.len(), source: l.offset });
        sk.arena.push_str(&l.text.replace('\0', "\u{fffd}"));
        sk.arena.push('\n');
    }
    blk.seg_end = sk.segs.len();
    blk.content_end = sk.arena.len();
    sk.blocks.push(blk);
    parse_leaf_range(h, &sk, 0, 0, blk.content_end)
}
