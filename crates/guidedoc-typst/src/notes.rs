//! Footnotes, citations, substitutions, and target notes.
//!
//! Footnotes become Typst footnotes at their first reference, keeping the source label
//! as the footnote mark. Later references repeat the mark through the footnote's label.
//! A footnote nothing refers to is a labelled block where it stands.

use guidedoc::{LinkKind, Node, NodeFlag, NodeId, NodeKind, arg_span, make_id, write_id};

use crate::messages::{CITATION_MISSING, FOOTNOTE_MISSING, SUBSTITUTION_MISSING};
use crate::state::{Mark, State};

/// Docutils' footnote symbols; value 1 is the first, as the RST reader numbers them.
const SYMBOLS: [&str; 10] = ["*", "†", "‡", "§", "¶", "#", "♠", "♥", "♦", "♣"];

impl State<'_, '_> {
    /// Returns the node an Internal link of `id` resolves to, when it is of `kind`.
    fn resolved_of_kind(&self, id: NodeId, kind: NodeKind) -> Option<NodeId> {
        let info = self.doc.link_info(id)?;
        let ok = info.kind == LinkKind::Internal
            && info.resolved.index() < self.doc.node_count()
            && self.doc.kind(info.resolved) == kind;
        ok.then_some(info.resolved)
    }

    /// Returns the noted node of `kind` that `matches` accepts, preferring the one whose
    /// anchor `build_anchors` derived from `label`, then the first in document order.
    fn find_note(
        &mut self,
        kind: NodeKind,
        label: &str,
        matches: impl Fn(&Self, NodeId) -> bool,
    ) -> Option<NodeId> {
        self.key.clear();
        if !self.options.anchor_prefix.is_empty() {
            write_id(&mut self.key, self.options.anchor_prefix);
            self.key.push('-');
        }
        let body = self.key.len();
        make_id(&mut self.key, label);
        let keyed = self.key.len() > body;
        let candidates = || self.notes.iter().copied().filter(|&n| self.doc.kind(n) == kind);
        let named = |n: NodeId| self.anchors.anchor_ids(n).any(|a| a == self.key);
        if keyed && let Some(found) = candidates().find(|&n| named(n) && matches(self, n)) {
            return Some(found);
        }
        candidates().find(|&n| matches(self, n))
    }

    /// Returns the footnote a reference points to.
    pub(crate) fn find_footnote(&mut self, reference: NodeId) -> Option<NodeId> {
        if let Some(found) = self.resolved_of_kind(reference, NodeKind::Footnote) {
            return Some(found);
        }
        let doc = self.doc;
        let r = *doc.node(reference);
        let label = doc.text(r.text);
        let label = if label.len() > 1 { label.strip_prefix('#').unwrap_or(label) } else { label };
        self.find_note(NodeKind::Footnote, label, |s, n| same_footnote(s, n, &r))
    }

    /// Writes the footnote's mark as a string literal: its number, its symbol, or its
    /// label.
    fn emit_note_mark(&mut self, note: NodeId) {
        let n = *self.doc.node(note);
        self.byte(b'"');
        if n.flags.has(NodeFlag::Symbol) {
            let index = usize::try_from(n.value - 1).unwrap_or(0);
            for _ in 0..=index / SYMBOLS.len() {
                self.emit(SYMBOLS[index % SYMBOLS.len()]);
            }
        } else if n.value > 0 {
            self.int(i64::from(n.value));
        } else {
            self.emit_string_body(self.doc.text(n.text), false);
        }
        self.byte(b'"');
    }

    pub(crate) fn render_footnote_reference(&mut self, reference: NodeId) {
        let Some(note) = self.find_footnote(reference) else {
            let n = *self.doc.node(reference);
            self.fail(FOOTNOTE_MISSING, n.span, arg_span(n.span));
            self.wrap_text("#super[", self.doc.text(n.text), "]");
            return;
        };
        let done = self.has_mark(note, Mark::Done);
        if self.quiet || (done && !self.has_mark(note, Mark::Labelled)) {
            self.emit("#super(");
            self.emit_note_mark(note);
            self.byte(b')');
            return;
        }
        self.emit("#gd-footnote(");
        self.emit_note_mark(note);
        if done {
            let name = self.anchors.anchor_of(note).unwrap_or("");
            let out = &mut self.ctx.out;
            out.emit(", label(");
            crate::escape::write_string_literal(out, name);
            out.emit("))");
            return;
        }
        self.set_mark(note, Mark::Done);
        self.emit(")[\n");
        self.render_blocks(note);
        self.byte(b']');
        self.emit_label(note);
    }

    /// Writes a footnote nothing refers to; referenced footnotes are written at their
    /// first reference instead.
    pub(crate) fn render_footnote_block(&mut self, note: NodeId) {
        let referenced = self.has_mark(note, Mark::Referenced) && !self.quiet;
        if referenced || self.has_mark(note, Mark::Done) {
            return;
        }
        if !self.quiet {
            self.set_mark(note, Mark::Done);
        }
        self.emit("#gd-note(");
        self.emit_note_mark(note);
        self.emit(")[\n");
        self.render_blocks(note);
        self.byte(b']');
        self.emit_label(note);
        self.byte(b'\n');
    }

    /// Writes referenced footnotes whose references were never rendered, such as
    /// references inside an unused substitution definition.
    pub(crate) fn flush_footnotes(&mut self) {
        for i in 0..self.notes.len() {
            let id = self.notes[i];
            if self.doc.kind(id) != NodeKind::Footnote
                || !self.has_mark(id, Mark::Referenced)
                || self.has_mark(id, Mark::Done)
            {
                continue;
            }
            self.clear_mark(id, Mark::Referenced);
            self.byte(b'\n');
            self.map_node(id);
            self.render_footnote_block(id);
        }
    }

    /// Returns the citation a reference names.
    fn find_citation(&mut self, reference: NodeId) -> Option<NodeId> {
        if let Some(found) = self.resolved_of_kind(reference, NodeKind::Citation) {
            return Some(found);
        }
        let label = self.doc.node_text(reference);
        self.find_note(NodeKind::Citation, label, |s, n| {
            s.doc.node_text(n).eq_ignore_ascii_case(label)
        })
    }

    pub(crate) fn render_citation(&mut self, id: NodeId) {
        self.emit("#gd-citation(");
        self.emit_string(self.doc.node_text(id));
        self.emit(")[\n");
        self.render_blocks(id);
        self.byte(b']');
        self.emit_label(id);
        self.byte(b'\n');
    }

    pub(crate) fn render_citation_reference(&mut self, reference: NodeId) {
        let n = *self.doc.node(reference);
        let citation = self.find_citation(reference);
        if citation.is_none() {
            self.fail(CITATION_MISSING, n.span, arg_span(n.span));
        }
        let name = citation.and_then(|c| self.anchors.anchor_of(c)).filter(|_| !self.quiet);
        if let Some(name) = name {
            let out = &mut self.ctx.out;
            out.emit("#gd-link(");
            crate::escape::write_string_literal(out, name);
            out.emit(")[");
        }
        let linked = name.is_some();
        self.wrap_text("\\[", self.doc.text(n.text), "\\]");
        if linked {
            self.byte(b']');
        }
    }

    /// Writes the definition's inlines in place of the reference.
    pub(crate) fn render_substitution(&mut self, reference: NodeId) {
        let doc = self.doc;
        let n = *doc.node(reference);
        let definition =
            self.resolved_of_kind(reference, NodeKind::SubstitutionDefinition).or_else(|| {
                let name = doc.text(n.text);
                self.notes.iter().copied().find(|&d| {
                    doc.kind(d) == NodeKind::SubstitutionDefinition && doc.node_text(d) == name
                })
            });
        match definition {
            Some(d) if !self.has_mark(d, Mark::Expanding) => {
                self.set_mark(d, Mark::Expanding);
                self.render_inlines(d);
                self.clear_mark(d, Mark::Expanding);
            }
            _ => {
                self.fail(SUBSTITUTION_MISSING, n.span, arg_span(n.span));
                self.emit_text(doc.text(n.text));
            }
        }
    }

    /// Lists every named external target with its URL.
    pub(crate) fn render_target_notes(&mut self) {
        let doc = self.doc;
        let mut count = 0;
        for i in 0..doc.node_count() {
            let t = NodeId::from_index(i);
            let kind = doc.kind(t);
            if kind != NodeKind::Target && kind != NodeKind::Link {
                continue;
            }
            let Some(info) = doc.link_info(t) else {
                continue;
            };
            if info.kind != LinkKind::External || info.destination.len == 0 {
                continue;
            }
            let name =
                if kind == NodeKind::Target { doc.node_text(t) } else { doc.text(info.name) };
            if name.is_empty() || doc.node(t).flags.has(NodeFlag::Anonymous) {
                continue;
            }
            if count == 0 {
                self.emit("#enum(\n");
            }
            count += 1;
            self.wrap_text("[", name, ": ");
            let destination = doc.text(info.destination);
            if self.url_allowed(t, destination) && !self.quiet {
                self.emit("#link(");
                self.emit_url(destination, false);
                self.emit(")");
            }
            self.emit("],\n");
        }
        if count > 0 {
            self.emit(")\n");
        }
    }
}

/// Tells whether footnote `note` is the one reference `r` means: the same symbol
/// sequence position, the same number, or the same label.
fn same_footnote(s: &State<'_, '_>, note: NodeId, r: &Node) -> bool {
    let f = s.doc.node(note);
    if f.flags.has(NodeFlag::Symbol) != r.flags.has(NodeFlag::Symbol) {
        return false;
    }
    if r.value > 0 || r.flags.has(NodeFlag::Symbol) {
        return f.value == r.value;
    }
    s.doc.text(f.text) == s.doc.text(r.text)
}
