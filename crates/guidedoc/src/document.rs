//! `Document`, the read-only view renderers and passes traverse. It borrows a snapshot
//! and the source table, so it cannot outlive them: a stale document does not compile.
//! An id out of range is a programming error (an index panic), not a data error: ids
//! come from the document itself.

use crate::kinds::{Payload, payload_of};
use crate::snapshot::{Snapshot, SourceEntry, entry_text};
use crate::text::TextSink;
use crate::types::{
    Attribute, AttributeKey, CellInfo, LinkInfo, ListInfo, NONE, Node, NodeId, NodeKind, SourceId,
    SourceSpan, Text,
};

/// A read-only view of one document and its sources. It is `Copy`; pass it by value.
#[derive(Debug, Clone, Copy)]
pub struct Document<'a> {
    snap: &'a Snapshot,
    pool: &'a str,
    main: &'a str,
    sources: &'a [SourceEntry],
}

/// The pool as text: all of it once validated, else its longest UTF-8 prefix (a view of
/// a snapshot still being built may end half-way through a character).
fn pool_text(bytes: &[u8]) -> &str {
    match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
    }
}

impl<'a> Document<'a> {
    /// Views `snap` with its sources: `main` is the main source's text.
    pub(crate) fn new(snap: &'a Snapshot, main: &'a str, sources: &'a [SourceEntry]) -> Self {
        Document { snap, pool: pool_text(&snap.text), main, sources }
    }

    /// Views a snapshot without sources, such as the one a builder is filling
    /// (`Builder::document`); source accessors then return nothing.
    pub fn from_snapshot(snap: &'a Snapshot) -> Self {
        Document::new(snap, "", &[])
    }

    /// Returns the underlying tables.
    pub fn snapshot(&self) -> &'a Snapshot {
        self.snap
    }

    /// Returns the number of nodes, the root included.
    pub fn node_count(&self) -> usize {
        self.snap.nodes.len()
    }

    /// Returns every node, in preorder.
    pub fn nodes(&self) -> &'a [Node] {
        &self.snap.nodes
    }

    /// Returns node `id`.
    pub fn node(&self, id: NodeId) -> &'a Node {
        &self.snap.nodes[id.index()]
    }

    /// Returns the kind of node `id`.
    pub fn kind(&self, id: NodeId) -> NodeKind {
        self.node(id).kind
    }

    /// Returns the id after the last descendant of node `id`.
    pub fn subtree_end(&self, id: NodeId) -> NodeId {
        NodeId(self.node(id).subtree_end)
    }

    /// Returns the text `t` names; "" when it is out of range.
    pub fn text(&self, t: Text) -> &'a str {
        self.pool.get(t.range()).unwrap_or("")
    }

    /// Returns the text of node `id`.
    pub fn node_text(&self, id: NodeId) -> &'a str {
        self.text(self.node(id).text)
    }

    /// Tells whether node `id` has a child.
    pub fn has_children(&self, id: NodeId) -> bool {
        id.0 + 1 < self.node(id).subtree_end
    }

    /// Returns the first child of node `id`.
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.has_children(id).then_some(id + 1)
    }

    /// Returns a code block's Caption child (its parsed Custom `caption`).
    pub fn code_caption(&self, id: NodeId) -> Option<NodeId> {
        self.first_child(id).filter(|&c| self.kind(c) == NodeKind::Caption)
    }

    /// Iterates the children of node `id` (Odin `children` + `next_child`).
    pub fn children(&self, id: NodeId) -> Children<'a> {
        Children { nodes: &self.snap.nodes, next: id.0 + 1, end: self.node(id).subtree_end }
    }

    /// Counts the children of node `id`, in time linear in their number.
    pub fn child_count(&self, id: NodeId) -> usize {
        self.children(id).count()
    }

    /// Returns the parent of node `id`; `None` for the root. It scans backwards, so it
    /// is meant for diagnostics, not traversal.
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        let nodes = &self.snap.nodes[..id.index().min(self.snap.nodes.len())];
        nodes.iter().rposition(|n| n.subtree_end > id.0).map(NodeId::from_index)
    }

    /// Returns every attribute row of node `id`.
    pub fn attributes(&self, id: NodeId) -> &'a [Attribute] {
        self.snap.attributes.get(self.node(id).attributes.range()).unwrap_or(&[])
    }

    /// Returns the value of node `id`'s first attribute with `key`.
    pub fn attribute(&self, id: NodeId, key: AttributeKey) -> Option<&'a str> {
        let a = self.attributes(id).iter().find(|a| a.key == key)?;
        Some(self.text(a.value))
    }

    /// Returns the value of node `id`'s Custom attribute named `name`.
    pub fn custom_attribute(&self, id: NodeId, name: &str) -> Option<&'a str> {
        let found = self
            .attributes(id)
            .iter()
            .find(|a| a.key == AttributeKey::Custom && self.text(a.name) == name)?;
        Some(self.text(found.value))
    }

    /// Tells whether one of node `id`'s Class attributes (each a space-separated list)
    /// holds `class`.
    pub fn has_class(&self, id: NodeId, class: &str) -> bool {
        self.attributes(id)
            .iter()
            .filter(|a| a.key == AttributeKey::Class)
            .any(|a| self.text(a.value).split(' ').any(|c| c == class))
    }

    fn payload_row(&self, id: NodeId, table: Payload) -> Option<usize> {
        let node = self.node(id);
        if node.payload == NONE || payload_of(node.kind) != table {
            return None;
        }
        Some(node.payload as usize)
    }

    /// Returns the list payload of node `id`, or a bullet list starting at 1.
    pub fn list_info(&self, id: NodeId) -> ListInfo {
        self.payload_row(id, Payload::List)
            .and_then(|p| self.snap.lists.get(p).copied())
            .unwrap_or_default()
    }

    /// Returns the link payload of node `id`, or `None` when it has none. Odin's
    /// `l, _ := link_info(v, id)` is `doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED)`.
    pub fn link_info(&self, id: NodeId) -> Option<LinkInfo> {
        self.payload_row(id, Payload::Link).and_then(|p| self.snap.links.get(p).copied())
    }

    /// Returns the cell payload of node `id`, or a cell spanning one row and column.
    pub fn cell_info(&self, id: NodeId) -> CellInfo {
        self.payload_row(id, Payload::Cell)
            .and_then(|p| self.snap.cells.get(p).copied())
            .unwrap_or_default()
    }

    /// Returns every source-table entry; 0 is the main source.
    pub fn sources(&self) -> &'a [SourceEntry] {
        self.sources
    }

    /// Returns the source-table entry of `id`, or `None` for an unknown id.
    pub fn source_entry(&self, id: SourceId) -> Option<&'a SourceEntry> {
        self.sources.get(id.index())
    }

    /// Returns the whole text of source `id`; "" for an unknown id.
    pub fn source_str(&self, id: SourceId) -> &'a str {
        entry_text(self.main, self.sources, id)
    }

    /// Returns the source text a span covers, or "" for an invalid span.
    pub fn source_text(&self, span: SourceSpan) -> &'a str {
        let text = self.source_str(span.source);
        text.get(span.start as usize..span.end as usize).unwrap_or("")
    }

    /// Writes the concatenated text of a subtree, as alt text and ids use it: text,
    /// code, math, and raw inline nodes, and a space for each line break.
    pub fn plain_text(&self, id: NodeId, out: &mut (impl TextSink + ?Sized)) {
        let end = self.node(id).subtree_end as usize;
        for node in &self.snap.nodes[id.index()..end] {
            match node.kind {
                NodeKind::Text | NodeKind::Code | NodeKind::Math | NodeKind::RawInline => {
                    out.put_str(self.text(node.text));
                }
                NodeKind::SoftBreak | NodeKind::HardBreak => out.put_char(' '),
                _ => {}
            }
        }
    }
}

/// The children of a node, in order; `Document::children` makes it.
#[derive(Debug, Clone)]
pub struct Children<'a> {
    nodes: &'a [Node],
    next: u32,
    end: u32,
}

impl Iterator for Children<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        if self.next >= self.end {
            return None;
        }
        let id = NodeId(self.next);
        // A corrupt subtree end would loop; validated documents never have one.
        self.next = self.nodes.get(id.index()).map_or(self.end, |n| n.subtree_end.max(id.0 + 1));
        Some(id)
    }
}
