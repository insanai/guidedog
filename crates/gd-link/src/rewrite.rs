//! The rewriter: copies a document into a pass's builder, asking a [`Rule`] what to do
//! with each node.
//!
//! Subtrees in which the rule takes no interest are copied whole with
//! `Builder::copy_subtree`; the rewriter only walks the nodes the rule wants to see, so
//! it is a thin decide/replace dispatch over the engine's copy. Internal links are
//! pointed at the copies afterwards, because dropping or replacing nodes shifts ids.

use guidedoc::{
    AttributeKey, Builder, Document, Fail, INVALID_NODE, LinkInfo, LinkKind, NodeId, NodeKind,
    PassOutcome, ROOT, SourceSpan,
};

/// What the rewriter does with one node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Copy the node and rewrite its children.
    Copy,
    /// Leave the node and its subtree out.
    Drop,
    /// Leave the node out but rewrite its children in its place.
    Unwrap,
    /// The rule's [`Rule::replace`] writes something else instead.
    Replace,
}

/// What a pass decides per node; `decide` and `decorate` default to a plain copy.
pub trait Rule {
    /// Whether the rule wants to see `id` itself (decide or decorate it). A subtree in
    /// which it wants to see no node is copied whole, without asking.
    fn visits(&self, doc: Document<'_>, id: NodeId) -> bool;

    /// What to do with `id`.
    fn decide(&mut self, doc: Document<'_>, id: NodeId) -> Action {
        let _ = (doc, id);
        Action::Copy
    }

    /// Writes what stands for `id` when [`Rule::decide`] said `Replace`.
    fn replace(&mut self, rw: &mut Rewriter<'_, '_, '_>, id: NodeId) -> Result<(), Fail>;

    /// Adds attributes to `copy`, the copy of `source`, before its children.
    fn decorate(
        &mut self,
        rw: &mut Rewriter<'_, '_, '_>,
        source: NodeId,
        copy: NodeId,
    ) -> Result<(), Fail> {
        let _ = (rw, source, copy);
        Ok(())
    }
}

/// A document being copied into a pass's builder.
pub struct Rewriter<'r, 'd, 'b> {
    /// The document read.
    pub doc: Document<'d>,
    /// The builder written.
    pub b: &'r mut Builder<'b>,
    /// Per source node: its copy, or `INVALID_NODE`.
    moved: Vec<NodeId>,
    /// Per source node: how many nodes before it the rule visits, to find subtrees
    /// it takes no interest in.
    visited_before: Vec<u32>,
}

impl std::fmt::Debug for Rewriter<'_, '_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rewriter").field("nodes", &self.moved.len()).finish_non_exhaustive()
    }
}

/// Copies `doc` into `b` under `rule` and remaps internal links: the body of a pass.
/// Returns `Rebuilt`, or `Failed` when the builder failed (its status says why).
pub fn rewrite_document(
    doc: Document<'_>,
    b: &mut Builder<'_>,
    rule: &mut dyn Rule,
) -> PassOutcome {
    let count = doc.node_count();
    let mut visited_before = Vec::with_capacity(count + 1);
    let mut visited = 0u32;
    for i in 0..count {
        visited_before.push(visited);
        visited += u32::from(rule.visits(doc, NodeId::from_index(i)));
    }
    visited_before.push(visited);
    let mut moved = vec![INVALID_NODE; count];
    if let Some(root) = moved.first_mut() {
        *root = ROOT;
    }
    let mut rw = Rewriter { doc, b, moved, visited_before };
    rw.b.set_value(ROOT, doc.node(ROOT).value); // who read it.
    let result = rw.rewrite_children(rule, ROOT);
    rw.remap_links();
    if result.is_ok() && rw.b.is_ok() { PassOutcome::Rebuilt } else { PassOutcome::Failed }
}

impl Rewriter<'_, '_, '_> {
    /// Rewrites the children of `parent`, each as the rule decides.
    pub fn rewrite_children(&mut self, rule: &mut dyn Rule, parent: NodeId) -> Result<(), Fail> {
        for child in self.doc.children(parent) {
            if !self.wants(child) {
                self.copy_whole(child)?;
                continue;
            }
            match rule.decide(self.doc, child) {
                Action::Copy => self.copy_node(rule, child)?,
                Action::Drop => {}
                Action::Unwrap => self.rewrite_children(rule, child)?,
                Action::Replace => rule.replace(self, child)?,
            }
        }
        Ok(())
    }

    /// Whether the rule wants to see any node of `id`'s subtree.
    fn wants(&self, id: NodeId) -> bool {
        let end = self.doc.subtree_end(id).index();
        self.visited_before[end] > self.visited_before[id.index()]
    }

    /// Copies a subtree the rule takes no interest in, as the engine copies it.
    fn copy_whole(&mut self, id: NodeId) -> Result<(), Fail> {
        let copy = self.b.copy_subtree(self.doc, id)?;
        for source in id.0..self.doc.subtree_end(id).0 {
            self.moved[source as usize] = NodeId(copy.0 + (source - id.0));
        }
        Ok(())
    }

    /// Copies one node with its text, attributes and payload, lets the rule decorate
    /// it, then rewrites its children.
    pub fn copy_node(&mut self, rule: &mut dyn Rule, id: NodeId) -> Result<(), Fail> {
        let n = *self.doc.node(id);
        let copy = self.b.begin_node(n.kind, n.span)?;
        self.moved[id.index()] = copy;
        self.b.set_value(copy, n.value);
        self.b.add_flags(copy, n.flags);
        let text = self.b.add_text(self.doc.node_text(id))?;
        self.b.set_text(copy, text);
        self.copy_attributes(id, copy, |_, _| true)?;
        rule.decorate(self, id, copy)?;
        self.b.copy_payload(self.doc, id, copy)?;
        self.rewrite_children(rule, id)?;
        self.b.end_node(copy)
    }

    /// Copies the attributes of `from` that `keep` accepts (given the key and the
    /// Custom name) to `to`, the node most recently begun.
    pub fn copy_attributes(
        &mut self,
        from: NodeId,
        to: NodeId,
        keep: impl Fn(AttributeKey, &str) -> bool,
    ) -> Result<(), Fail> {
        let doc = self.doc;
        for a in doc.attributes(from) {
            let name = doc.text(a.name);
            if !keep(a.key, name) {
                continue;
            }
            let value = self.b.add_text(doc.text(a.value))?;
            let name = self.b.add_text(name)?;
            self.b.add_attribute_named(to, a.key, value, name)?;
        }
        Ok(())
    }

    /// Records that `copy` stands for source node `id`, so links to `id` reach it.
    pub fn moved_to(&mut self, id: NodeId, copy: NodeId) {
        if let Some(slot) = self.moved.get_mut(id.index()) {
            *slot = copy;
        }
    }

    /// Opens a node of `kind`.
    pub fn open(&mut self, kind: NodeKind, span: SourceSpan) -> Result<NodeId, Fail> {
        self.b.begin_node(kind, span)
    }

    /// Closes `id`, the innermost open node.
    pub fn close(&mut self, id: NodeId) -> Result<(), Fail> {
        self.b.end_node(id)
    }

    /// Adds a Text leaf.
    pub fn add_plain(&mut self, text: &str, span: SourceSpan) -> Result<(), Fail> {
        self.b.add_text_leaf(NodeKind::Text, span, text).map(|_| ())
    }

    /// Adds a Class attribute to `id`.
    pub fn add_class(&mut self, id: NodeId, class: &str) -> Result<(), Fail> {
        self.b.add_attribute_string(id, AttributeKey::Class, class)
    }

    /// Adds a Custom attribute to `id`.
    pub fn add_custom(&mut self, id: NodeId, name: &str, value: &str) -> Result<(), Fail> {
        self.b.add_custom_attribute(id, name, value)
    }

    /// Opens a Link node whose destination is `url` of `kind`, with a title.
    pub fn open_link(
        &mut self,
        url: &str,
        title: &str,
        kind: LinkKind,
        span: SourceSpan,
    ) -> Result<NodeId, Fail> {
        let link = self.open(NodeKind::Link, span)?;
        let destination = self.b.add_text(url)?;
        let title = self.b.add_text(title)?;
        self.b.set_link(link, LinkInfo { destination, title, kind, ..LinkInfo::default() })?;
        Ok(link)
    }

    /// Points internal links at the copies of their targets; a link whose target was
    /// left out points at the document root.
    fn remap_links(&mut self) {
        for link in &mut self.b.snap.links {
            if link.kind != LinkKind::Internal {
                continue;
            }
            link.resolved = match self.moved.get(link.resolved.index()) {
                Some(&copy) if copy != INVALID_NODE => copy,
                _ => ROOT,
            };
        }
    }
}
