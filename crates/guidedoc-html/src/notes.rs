//! Footnotes and citations, their references and backlinks, and the footnotes of
//! target-notes.

use guidedoc::{AnchorExtra, LinkInfo, LinkKind, NodeFlag, NodeId, NodeKind};

use crate::state::Html;
use crate::walk::{Close, Visit};

/// Links every footnote and citation to the references that resolve to it, so a note
/// can list backlinks even when its references come later in the document. References
/// and backlinks use the ids of the anchor table, as Docutils numbers them.
#[derive(Debug, Default)]
pub(crate) struct Notes {
    /// Per note: its references, in document order; empty when nothing refers to notes.
    references: Vec<Vec<NodeId>>,
}

/// The footnote symbols of Docutils, in order.
const SYMBOLS: [&str; 10] = ["*", "†", "‡", "§", "¶", "#", "♠", "♥", "♦", "♣"];

/// Bounds labels like "**" for the eleventh symbol footnote.
const MAX_SYMBOL_REPEATS: usize = 8;

fn is_note_reference(k: NodeKind) -> bool {
    k == NodeKind::FootnoteReference || k == NodeKind::CitationReference
}

impl Html<'_, '_> {
    /// Finds the references of every note.
    pub fn build_notes(&mut self) {
        let doc = self.doc;
        let n = doc.node_count();
        let mut references: Vec<Vec<NodeId>> = Vec::new();
        for i in 0..n {
            let id = NodeId::from_index(i);
            if !is_note_reference(doc.kind(id)) {
                continue;
            }
            let Some(l) = doc.link_info(id) else {
                continue;
            };
            if l.kind != LinkKind::Internal || l.resolved.index() >= n {
                continue;
            }
            if references.is_empty() {
                references.resize_with(n, Vec::new);
            }
            references[l.resolved.index()].push(id);
        }
        self.notes = Notes { references };
    }

    /// Writes a note's or reference's number, symbol, or label.
    fn write_note_label(&mut self, id: NodeId) {
        let node = self.doc.node(id);
        let citation = matches!(node.kind, NodeKind::Citation | NodeKind::CitationReference);
        if node.flags.has(NodeFlag::Symbol) {
            let index = usize::try_from(node.value.saturating_sub(1)).unwrap_or(0);
            let repeats = (index / SYMBOLS.len() + 1).min(MAX_SYMBOL_REPEATS);
            for _ in 0..repeats {
                self.w.put(SYMBOLS[index % SYMBOLS.len()]);
            }
        } else if node.value > 0 && !citation {
            self.w.put_int(i64::from(node.value));
        } else {
            self.w.esc(self.doc.node_text(id));
        }
    }

    pub fn enter_note_reference(&mut self, id: NodeId) -> Visit {
        let footnote = self.doc.kind(id) == NodeKind::FootnoteReference;
        self.w.put("<a class=\"");
        self.w.put(if footnote { "footnote-reference" } else { "citation-reference" });
        self.w.put_byte(b'"');
        self.write_id_attribute(id);
        self.write_target("href", id);
        self.w.put(">[");
        self.write_note_label(id);
        self.w.put("]</a>");
        Visit::Skip
    }

    /// Opens a footnote or citation with its label and backlinks.
    pub fn enter_note(&mut self, id: NodeId) -> Visit {
        let footnote = self.doc.kind(id) == NodeKind::Footnote;
        self.w.cr();
        self.tag("aside", id, if footnote { "footnote" } else { "citation" });
        self.w.cr();
        self.w.put("<p class=\"label\"><span class=\"label-text\">[");
        self.write_note_label(id);
        self.w.put("]</span>");
        let references = self.notes.references.get(id.index()).map_or(&[][..], Vec::as_slice);
        let count = references.len();
        for (k, &r) in references.iter().enumerate() {
            let target = if self.w.reference { None } else { self.anchors.anchor_of(r) };
            write_backref(&mut self.w, target.unwrap_or(""), if count > 1 { k + 1 } else { 0 });
        }
        self.w.put("</p>");
        self.w.cr();
        Visit::Descend(Close::Block("aside"))
    }

    /// Writes the footnotes of a target-notes directive, one per external URI that
    /// references lead to, as Docutils makes them; `note_references` writes the
    /// footnote references it adds after those references.
    pub fn target_notes(&mut self, id: NodeId) {
        let doc = self.doc;
        if self.w.reference || self.anchors.anchor_extras(id).is_empty() {
            return;
        }
        self.w.cr();
        self.open_tag("div", id, "target-notes");
        self.w.put(">");
        self.write_aliases(id);
        for i in 0..self.anchors.anchor_extras(id).len() {
            let note = self.anchors.anchor_extras(id)[i];
            self.w.cr();
            self.w.put("<aside class=\"footnote\" id=\"");
            self.w.esc(self.anchors.extra_text(&note));
            self.w.put("\">\n<p class=\"label\"><span class=\"label-text\">[");
            self.w.put_int(i64::from(note.number));
            self.w.put("]</span>");
            self.write_note_backrefs(id, note);
            self.w.put("</p>\n<p><a");
            self.write_target("href", note.item);
            self.w.put_byte(b'>');
            let l = doc.link_info(note.item).unwrap_or(LinkInfo::UNRESOLVED);
            self.w.esc(doc.text(l.destination));
            self.w.put("</a></p>\n</aside>");
        }
        self.w.cr();
        self.w.put("</div>");
        self.w.cr();
    }

    /// Writes the backlinks of a target-notes footnote: one to each footnote reference
    /// added for it, which are the extras owned by references, in document order.
    fn write_note_backrefs(&mut self, directive: NodeId, note: AnchorExtra) {
        let is_backref =
            |e: &&AnchorExtra| e.item == directive && e.note == note.item && e.owner != directive;
        let count = self.anchors.extras().iter().filter(is_backref).count();
        for (k, e) in self.anchors.extras().iter().filter(is_backref).enumerate() {
            let text = self.anchors.extra_text(e);
            write_backref(&mut self.w, text, if count > 1 { k + 1 } else { 0 });
        }
    }

    /// Writes, after a reference, the footnote references target-notes add.
    pub fn note_references(&mut self, reference: NodeId) {
        if self.w.reference {
            return;
        }
        for e in self.anchors.anchor_extras(reference) {
            let Some(note) = self.anchors.extra_anchor(e.item, e.note) else {
                continue;
            };
            self.w.put(" <a class=\"footnote-reference\" id=\"");
            self.w.esc(self.anchors.extra_text(e));
            self.w.put("\" href=\"#");
            self.w.esc(note);
            self.w.put("\">[");
            self.w.put_int(i64::from(e.number));
            self.w.put("]</a>");
        }
    }
}

/// Links a note back to a reference; `k` numbers the links of a note that has several.
fn write_backref(w: &mut crate::state::Writer<'_, '_>, target: &str, k: usize) {
    w.put(" <a class=\"backref\" href=\"#");
    w.esc(target);
    w.put("\">↩");
    if k > 0 {
        w.put_int(i64::try_from(k).unwrap_or(i64::MAX));
    }
    w.put("</a>");
}
