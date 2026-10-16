//! References in a translation are matched with the original's, as Sphinx's Locale
//! transform keeps them: a named reference by its name, a footnote or citation by its
//! label (auto-numbered ones in order), a substitution by its name, a cross-reference by
//! its role and target. A matched reference takes over the original's resolution (and a
//! cross-reference its context: module, class, domain), so the translated text links
//! where the original did. When the two sets differ, a warning says so; a translation
//! with a reference the original lacks cannot be resolved and is not used.

use guidedoc::{
    Attribute, AttributeKey, Document, Fail, LinkInfo, NONE, NodeFlag, NodeId, NodeKind, arg_span,
    arg_text,
};

use crate::apply::attribute_range;
use crate::messages::{I18N_REFERENCES, I18N_UNRESOLVED};
use crate::translate::Walk;
use crate::units::written_content;

/// A reference-like node and what identifies it.
#[derive(Debug, Clone, Copy)]
struct Ref<'s> {
    id: NodeId,
    kind: NodeKind,
    key: &'s str,
    /// A cross-reference's role (reftype).
    role: &'s str,
    matched: bool,
}

/// Describes node `id` as a reference, or returns `None` for other nodes.
fn ref_of(doc: Document<'_>, id: NodeId) -> Option<Ref<'_>> {
    let n = doc.node(id);
    let link_name = || doc.link_info(id).map_or("", |l| doc.text(l.name));
    let (key, role) = match n.kind {
        NodeKind::Reference if n.flags.has(NodeFlag::Anonymous) => ("", ""),
        NodeKind::Reference => (link_name(), ""),
        NodeKind::FootnoteReference
        | NodeKind::CitationReference
        | NodeKind::SubstitutionReference
        | NodeKind::InlineTarget => (doc.node_text(id), ""),
        NodeKind::Xref => (link_name(), doc.custom_attribute(id, "reftype").unwrap_or("")),
        NodeKind::Index if doc.attribute(id, AttributeKey::Id).is_some() => {
            (doc.custom_attribute(id, "entries").unwrap_or(""), "")
        }
        _ => return None,
    };
    Some(Ref { id, kind: n.kind, key, role, matched: false })
}

/// Lists the references among nodes `first..end` of `doc`.
fn collect(doc: Document<'_>, first: NodeId, end: NodeId) -> Vec<Ref<'_>> {
    (first.0..end.0).filter_map(|i| ref_of(doc, NodeId(i))).collect()
}

/// How a translation's references compare with the original's.
#[derive(Debug, Default)]
struct Matching {
    /// (original, translation) pairs.
    pairs: Vec<(NodeId, NodeId)>,
    /// The translation has a reference the original lacks.
    missing: bool,
    /// The original has a reference the translation lacks.
    extra: bool,
}

/// Pairs the references of `new` with those of `old`, each used once.
fn pair_references(old: &mut [Ref<'_>], new: &mut [Ref<'_>]) -> Matching {
    let mut m = Matching::default();
    for r in new.iter_mut() {
        let found = old
            .iter_mut()
            .find(|o| !o.matched && o.kind == r.kind && o.key == r.key && o.role == r.role);
        if let Some(o) = found {
            o.matched = true;
            r.matched = true;
            m.pairs.push((o.id, r.id));
        }
    }
    // Translated :index: roles may change their entry text. Match unchanged entries
    // first, then pair remaining indexes in order, preserving document-wide ids.
    for r in new.iter_mut().filter(|r| !r.matched && r.kind == NodeKind::Index) {
        if let Some(o) = old.iter_mut().find(|o| !o.matched && o.kind == NodeKind::Index) {
            o.matched = true;
            r.matched = true;
            m.pairs.push((o.id, r.id));
        }
    }
    m.missing = new.iter().any(|r| !r.matched && r.kind != NodeKind::InlineTarget);
    m.extra = old.iter().any(|o| !o.matched && o.kind != NodeKind::InlineTarget);
    m
}

impl Walk<'_, '_, '_, '_, '_> {
    /// Pairs the translation's references (the builder's nodes from `first` on) with
    /// those of original node `orig` and gives each the original's resolution. Returns
    /// false, after a warning, when a reference of the translation has no counterpart.
    pub(crate) fn match_references(
        &mut self,
        orig: NodeId,
        first: NodeId,
        msgstr: &str,
    ) -> Result<bool, Fail> {
        let doc = self.doc;
        let matching = {
            let fresh = self.b.document();
            let end = NodeId::from_index(fresh.node_count());
            let mut old = collect(doc, written_content(doc, orig), doc.subtree_end(orig));
            let mut new = collect(fresh, first, end);
            pair_references(&mut old, &mut new)
        };
        let span = doc.node(orig).span;
        if matching.missing {
            self.b.report(I18N_UNRESOLVED, span, arg_span(span), arg_text(msgstr));
            return Ok(false);
        }
        if matching.extra {
            self.b.report(I18N_REFERENCES, span, arg_span(span), arg_text(msgstr));
        }
        for (old, new) in matching.pairs {
            self.adopt(old, new)?;
        }
        Ok(true)
    }

    /// Gives the translation's node the original's resolution: its link row, number,
    /// flags, and, for a cross-reference, its context attributes. Links to the original
    /// node now reach the translation's.
    fn adopt(&mut self, old: NodeId, new: NodeId) -> Result<(), Fail> {
        let doc = self.doc;
        self.st.moved[old.index()] = new;
        let n = doc.node(old);
        self.b.set_value(new, n.value);
        self.b.add_flags(new, n.flags);
        if n.payload != NONE {
            let l = doc.link_info(old).unwrap_or(LinkInfo::UNRESOLVED);
            let l = LinkInfo {
                destination: self.b.add_text(doc.text(l.destination))?,
                title: self.b.add_text(doc.text(l.title))?,
                name: self.b.add_text(doc.text(l.name))?,
                ..l
            };
            match self.b.snap.nodes[new.index()].payload {
                NONE => {
                    self.b.set_link(new, l)?;
                }
                row => self.b.snap.links[row as usize] = l,
            }
        }
        if n.kind == NodeKind::Xref {
            self.adopt_attributes(old, new)?;
        } else if n.kind == NodeKind::Index {
            self.adopt_index_id(old, new)?;
        }
        Ok(())
    }

    /// Inline parsing has its own index serial. Keep the source document's identity
    /// instead, so separate translated messages cannot both become index-0.
    fn adopt_index_id(&mut self, old: NodeId, new: NodeId) -> Result<(), Fail> {
        let original = self.doc.attribute(old, AttributeKey::Id).unwrap_or("");
        let value = self.b.add_text(original)?;
        for row in self.b.snap.nodes[new.index()].attributes.range() {
            if self.b.snap.attributes[row].key == AttributeKey::Id {
                self.b.snap.attributes[row].value = value;
            }
        }
        Ok(())
    }

    /// Replaces a cross-reference's attributes with the original's (the context the
    /// reader recorded where the original stood), keeping the translation's own
    /// explicit-title attribute. The new range is appended to the attribute table.
    fn adopt_attributes(&mut self, old: NodeId, new: NodeId) -> Result<(), Fail> {
        let doc = self.doc;
        let explicit = self.b.document().custom_attribute(new, "refexplicit").map(str::to_owned);
        let mut rows = Vec::with_capacity(doc.attributes(old).len() + 1);
        for a in doc.attributes(old) {
            let name = doc.text(a.name);
            if a.key == AttributeKey::Custom && name == "refexplicit" {
                continue;
            }
            let value = self.b.add_text(doc.text(a.value))?;
            let name = self.b.add_text(name)?;
            rows.push(Attribute { key: a.key, name, value });
        }
        if let Some(explicit) = explicit {
            let value = self.b.add_text(&explicit)?;
            let name = self.b.add_text("refexplicit")?;
            rows.push(Attribute { key: AttributeKey::Custom, name, value });
        }
        let first = self.b.snap.attributes.len();
        self.b.snap.attributes.extend(rows);
        let count = self.b.snap.attributes.len() - first;
        self.b.snap.nodes[new.index()].attributes = attribute_range(first, count);
        Ok(())
    }
}
