//! Tree edits for the transforms that follow resolution (document structure,
//! bibliographic fields). The snapshot is in preorder: a node's subtree is the range
//! `[id, subtree_end)`. New nodes are appended as children of the still-open root and
//! then moved into place. Every edit keeps subtree ends, attribute ranges, and internal
//! link targets consistent. Records of the resolver are not updated, so edits run last.

use guidedoc::{
    Attribute, AttributeKey, AttributeRange, BUILDER_OPEN, Fail, LinkKind, NodeId, NodeKind, Text,
};

use super::Resolver;

impl Resolver<'_, '_> {
    /// Returns the last child of `id`.
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        let mut last = None;
        let end = self.node_end(id);
        let mut c = id.index() + 1;
        while c < end {
            last = Some(NodeId::from_index(c));
            c = self.node_end(NodeId::from_index(c));
        }
        last
    }

    /// Grows `parent` and every ancestor of it by `n` nodes.
    fn extend(&mut self, parent: NodeId, n: usize) {
        let nodes = &mut self.ctx.b.snap.nodes;
        for (i, node) in nodes.iter_mut().enumerate().take(parent.index() + 1) {
            let end = node.subtree_end;
            if end == BUILDER_OPEN {
                continue;
            }
            if i == parent.index() || end as usize > parent.index() {
                node.subtree_end = end + n as u32;
            }
        }
    }

    /// Moves the nodes appended since `first_new` to position `at`, as the last
    /// children so far of `parent`: `at` must lie inside parent's subtree or at its end.
    pub fn insert_new(&mut self, first_new: usize, at: usize, parent: NodeId) -> Result<(), Fail> {
        let used = self.used();
        if used <= first_new {
            return Ok(());
        }
        self.ctx.b.spend(used as u64)?;
        self.extend(parent, used - first_new);
        self.rotate(at, first_new, used);
        Ok(())
    }

    /// Moves the child range `[a, e)` of `parent` to follow it, after any nodes already
    /// moved there: `after` nodes follow parent and precede the new position.
    pub fn move_out(
        &mut self,
        parent: NodeId,
        a: usize,
        e: usize,
        after: usize,
    ) -> Result<(), Fail> {
        let parent_end = self.ctx.b.snap.nodes[parent].subtree_end as usize;
        self.ctx.b.spend((parent_end + after - a) as u64)?;
        self.rotate(a, e, parent_end + after);
        self.ctx.b.snap.nodes[parent.index()].subtree_end = (parent_end - (e - a)) as u32;
        Ok(())
    }

    /// Deletes the nodes `[a, e)`: whole subtrees that are siblings. Links that
    /// resolved into the range resolve to the node that takes index `a`.
    pub fn remove_range(&mut self, a: usize, e: usize) -> Result<(), Fail> {
        if e <= a {
            return Ok(());
        }
        let k = e - a;
        self.ctx.b.spend(self.used() as u64)?;
        let snap = &mut *self.ctx.b.snap;
        snap.nodes.drain(a..e);
        for node in &mut snap.nodes {
            let end = node.subtree_end;
            if end != BUILDER_OPEN && end as usize >= e {
                node.subtree_end = end - k as u32;
            }
        }
        let last = snap.nodes.len().saturating_sub(1);
        for l in &mut snap.links {
            if l.kind != LinkKind::Internal || l.resolved == guidedoc::INVALID_NODE {
                continue;
            }
            if l.resolved.index() >= e {
                l.resolved = l.resolved - k as u32;
            } else if l.resolved.index() >= a {
                l.resolved = NodeId::from_index(a.min(last));
            }
        }
        Ok(())
    }

    /// Gives an existing node one more attribute. The node's attributes are copied to
    /// the end of the attribute table, since each node's range is contiguous.
    pub fn set_attribute(
        &mut self,
        id: NodeId,
        key: AttributeKey,
        value: &str,
        name: &str,
    ) -> Result<(), Fail> {
        let value = self.ctx.b.add_text(value)?;
        let name = if name.is_empty() { Text::default() } else { self.ctx.b.add_text(name)? };
        let snap = &mut *self.ctx.b.snap;
        let old = snap.nodes[id].attributes;
        let first = snap.attributes.len();
        for k in old.range() {
            let a = snap.attributes[k];
            snap.attributes.push(a);
        }
        snap.attributes.push(Attribute { key, name, value });
        snap.nodes[id.index()].attributes =
            AttributeRange { first: first as u32, count: old.count + 1 };
        Ok(())
    }

    /// Changes the level of every section and heading in `[a, e)` by `delta`.
    pub fn shift_levels(&mut self, a: usize, e: usize, delta: i32) {
        for node in &mut self.ctx.b.snap.nodes[a..e] {
            let k = node.kind;
            if (k == NodeKind::Section || k == NodeKind::Heading) && node.value + delta >= 1 {
                node.value += delta;
            }
        }
    }

    /// Moves the nodes `[h, t)` in front of the sibling nodes `[s, h)`, fixing subtree
    /// ends and internal link targets that point into the moved range.
    pub fn rotate(&mut self, s: usize, h: usize, t: usize) {
        if s >= h || h >= t {
            return;
        }
        let (before, moved) = ((h - s) as u32, (t - h) as u32);
        let snap = &mut *self.ctx.b.snap;
        for (i, node) in snap.nodes[s..t].iter_mut().enumerate() {
            let end = node.subtree_end;
            node.subtree_end = if s + i < h { end + moved } else { end - before };
        }
        snap.nodes[s..t].rotate_left(h - s);
        for l in &mut snap.links {
            let target = l.resolved.index();
            if l.kind != LinkKind::Internal || target < s || target >= t {
                continue;
            }
            l.resolved = if target < h { l.resolved + moved } else { l.resolved - before };
        }
    }
}
