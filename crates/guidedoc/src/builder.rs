//! The builder appends nodes in preorder. Open nodes form a stack; closing a node records
//! its subtree end. The first failure poisons the builder: every later call fails with
//! the same `Fail`, so an ignored failure can never produce a truncated document.

use std::fmt;

use crate::document::Document;
use crate::kinds::{Payload, child_allowed, kind_name, payload_of};
use crate::options::Limits;
use crate::report::{
    Arg, CORE_CANCELLED, CORE_CAPACITY, CORE_DEPTH, CORE_INVALID, CORE_NESTING, CORE_NODES,
    CORE_WORK, Diagnostic, Fail, Message, Reports, arg_int, arg_static, diagnostic,
};
use crate::snapshot::Snapshot;
use crate::text::TextSink;
use crate::types::{
    Attribute, AttributeKey, AttributeRange, CapacityNeed, CellInfo, INVALID_NODE, LinkInfo,
    LinkKind, ListInfo, NONE, Node, NodeFlags, NodeId, NodeKind, ROOT, Region, SourceSpan, Status,
    Text,
};

/// The subtree end of a node that is still open.
pub const BUILDER_OPEN: u32 = u32::MAX - 1;

/// How many work steps pass between polls of the cancellation probe.
const CANCEL_POLL_STEPS: u64 = 1 << 16;

/// Builds one snapshot for one stage (a read or a pass). It borrows the snapshot, the
/// workspace's reports, and a reusable open-node stack for the stage.
pub struct Builder<'a> {
    /// The snapshot being built. Readers may read it freely and edit what the builder
    /// has no method for; validation checks the result.
    pub snap: &'a mut Snapshot,
    /// The stage's limits.
    pub limits: Limits,
    reports: &'a mut Reports,
    open: &'a mut Vec<NodeId>,
    status: Status,
    need: CapacityNeed,
    failure: Diagnostic,
    work: u64,
    cancel: Option<&'a dyn Fn() -> bool>,
}

impl fmt::Debug for Builder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Builder")
            .field("nodes", &self.snap.nodes.len())
            .field("open", &self.open.len())
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl<'a> Builder<'a> {
    /// Starts a builder over `snap`, which it clears, and opens the root Document node at
    /// `root_span`. `open` is reusable storage for the open-node stack. The engine calls
    /// it for every read and pass; tests use it to drive a reader's pieces. A failure to
    /// open the root poisons the builder (see `status`).
    pub fn new(
        snap: &'a mut Snapshot,
        reports: &'a mut Reports,
        open: &'a mut Vec<NodeId>,
        limits: Limits,
        root_span: SourceSpan,
    ) -> Builder<'a> {
        snap.clear();
        open.clear();
        let mut b = Builder {
            snap,
            limits,
            reports,
            open,
            status: Status::Ok,
            need: CapacityNeed::default(),
            failure: Diagnostic::default(),
            work: 0,
            cancel: None,
        };
        // A failure here poisons the builder, which every later call reports.
        let _ = b.begin_node(NodeKind::Document, root_span);
        b
    }

    /// Sets the probe polled while work is spent; true cancels the stage.
    pub fn set_cancel(&mut self, cancel: Option<&'a dyn Fn() -> bool>) {
        self.cancel = cancel;
    }

    /// Returns `Ok` until the builder fails, then the failure's status.
    pub fn status(&self) -> Status {
        self.status
    }

    /// Tells whether the builder has not failed (Odin `b.status == .Ok`).
    pub fn is_ok(&self) -> bool {
        self.status == Status::Ok
    }

    /// Returns the capacity a `Capacity` failure lacked.
    pub fn need(&self) -> CapacityNeed {
        self.need
    }

    /// Returns the work spent so far.
    pub fn work(&self) -> u64 {
        self.work
    }

    /// Returns the reports the stage writes to.
    pub fn reports(&mut self) -> &mut Reports {
        self.reports
    }

    /// Reports message `m` at `span` without failing.
    pub fn report(&mut self, m: Message, span: SourceSpan, a: Arg, b: Arg) {
        self.reports.message(m, span, a, b);
    }

    /// Views the snapshot built so far, without sources.
    pub fn document(&self) -> Document<'_> {
        Document::from_snapshot(self.snap)
    }

    /// The failure every call after the first failure returns.
    fn poisoned(&self) -> Fail {
        Fail { status: self.status, need: self.need, diagnostic: Box::new(self.failure.clone()) }
    }

    /// Fails with the stored failure when the builder has already failed.
    fn check(&self) -> Result<(), Fail> {
        if self.status == Status::Ok { Ok(()) } else { Err(self.poisoned()) }
    }

    /// Reports `d` and poisons the builder with its status (the first failure wins);
    /// returns the failure to propagate (Odin `builder_fail`).
    pub fn fail(&mut self, d: Diagnostic) -> Fail {
        if self.status == Status::Ok {
            self.status = d.status;
            self.failure = d.clone();
        }
        self.reports.report(d);
        self.poisoned()
    }

    /// Poisons the builder with a `Capacity` failure of `region`, lacking `minimum`
    /// (Odin `builder_capacity`); nothing new is reported once it has failed.
    pub fn capacity(&mut self, region: Region, minimum: u64) -> Fail {
        if self.status != Status::Ok {
            return self.poisoned();
        }
        let d = diagnostic(
            CORE_CAPACITY,
            SourceSpan::default(),
            arg_static(region.name()),
            arg_int(i64::try_from(minimum).unwrap_or(i64::MAX)),
        );
        self.need = CapacityNeed { region, minimum, exact: false };
        self.fail(d)
    }

    /// Charges `steps` of work against `Limits::max_work` and polls cancellation
    /// periodically.
    pub fn spend(&mut self, steps: u64) -> Result<(), Fail> {
        self.check()?;
        let before = self.work;
        self.work = self.work.saturating_add(steps);
        if self.work > self.limits.max_work {
            let max = i64::try_from(self.limits.max_work).unwrap_or(i64::MAX);
            return Err(self.fail(diagnostic(
                CORE_WORK,
                SourceSpan::default(),
                arg_int(max),
                Arg::None,
            )));
        }
        let crossed = before / CANCEL_POLL_STEPS != self.work / CANCEL_POLL_STEPS;
        if crossed && self.cancel.is_some_and(|probe| probe()) {
            return Err(self.fail(diagnostic(
                CORE_CANCELLED,
                SourceSpan::default(),
                Arg::None,
                Arg::None,
            )));
        }
        Ok(())
    }

    /// Returns the innermost open node; `None` once the root is closed.
    pub fn current(&self) -> Option<NodeId> {
        self.open.last().copied()
    }

    /// Returns the kind of the innermost open node; Document when none is open.
    pub fn current_kind(&self) -> NodeKind {
        self.open.last().map_or(NodeKind::Document, |&id| self.snap.nodes[id].kind)
    }

    /// Returns the open nodes, outermost first (Odin `b.open.items[:b.open.used]`).
    pub fn open_nodes(&self) -> &[NodeId] {
        self.open
    }

    /// Opens a node of `kind` as the last child of the current node, to be closed with
    /// `end_node`. It fails, poisoning the builder, when the nesting is illegal
    /// (`child_allowed`), a limit is reached, or memory runs out.
    pub fn begin_node(&mut self, kind: NodeKind, span: SourceSpan) -> Result<NodeId, Fail> {
        self.check()?;
        if let Some(&parent) = self.open.last() {
            let parent_kind = self.snap.nodes[parent].kind;
            if !child_allowed(parent_kind, kind) {
                let d = diagnostic(
                    CORE_NESTING,
                    span,
                    arg_static(kind_name(parent_kind)),
                    arg_static(kind_name(kind)),
                );
                return Err(self.fail(d));
            }
        }
        if self.open.len() > self.limits.max_depth {
            let max = i64::try_from(self.limits.max_depth).unwrap_or(i64::MAX);
            return Err(self.fail(diagnostic(CORE_DEPTH, span, arg_int(max), Arg::None)));
        }
        let used = self.snap.nodes.len();
        if used >= self.limits.max_nodes || used >= BUILDER_OPEN as usize {
            let max = i64::try_from(self.limits.max_nodes).unwrap_or(i64::MAX);
            return Err(self.fail(diagnostic(CORE_NODES, span, arg_int(max), Arg::None)));
        }
        if self.snap.nodes.try_reserve(1).is_err() || self.open.try_reserve(1).is_err() {
            return Err(self.capacity(Region::Nodes, 1));
        }
        let id = NodeId::from_index(used);
        let first = u32::try_from(self.snap.attributes.len()).unwrap_or(u32::MAX);
        self.snap.nodes.push(Node {
            kind,
            subtree_end: BUILDER_OPEN,
            payload: NONE,
            span,
            attributes: AttributeRange { first, count: 0 },
            ..Node::default()
        });
        self.open.push(id);
        Ok(id)
    }

    /// Closes `id`, which must be the innermost open node, and records its subtree end;
    /// closing out of order poisons the builder.
    pub fn end_node(&mut self, id: NodeId) -> Result<(), Fail> {
        self.check()?;
        if self.current() != Some(id) {
            return Err(self.invalid(id, "closed out of order", SourceSpan::default()));
        }
        let end = u32::try_from(self.snap.nodes.len()).unwrap_or(u32::MAX);
        self.snap.nodes[id].subtree_end = end;
        self.open.pop();
        Ok(())
    }

    /// Opens a node of `kind`, runs `fill` to add its attributes and children, and closes
    /// it: `begin_node` and `end_node` in one call that cannot forget the end.
    pub fn with_node(
        &mut self,
        kind: NodeKind,
        span: SourceSpan,
        fill: impl FnOnce(&mut Self, NodeId) -> Result<(), Fail>,
    ) -> Result<NodeId, Fail> {
        let id = self.begin_node(kind, span)?;
        fill(self, id)?;
        self.end_node(id)?;
        Ok(id)
    }

    /// Runs `push`, which writes into the text pool, and returns the text it wrote:
    /// `text_open`, the pushes, and `text_close` in one call.
    pub fn build_text(
        &mut self,
        push: impl FnOnce(&mut Self) -> Result<(), Fail>,
    ) -> Result<Text, Fail> {
        let start = self.text_open();
        push(self)?;
        self.check()?;
        Ok(self.text_close(start))
    }

    /// Poisons the builder with an invalid-tree failure for node `id`.
    fn invalid(&mut self, id: NodeId, why: &'static str, span: SourceSpan) -> Fail {
        self.fail(diagnostic(CORE_INVALID, span, arg_int(i64::from(id.0)), arg_static(why)))
    }

    /// Adds a closed node of `kind` with pool text `text` and no children.
    pub fn add_leaf(
        &mut self,
        kind: NodeKind,
        span: SourceSpan,
        text: Text,
    ) -> Result<NodeId, Fail> {
        let id = self.begin_node(kind, span)?;
        self.snap.nodes[id].text = text;
        self.end_node(id)?;
        Ok(id)
    }

    /// Copies `s` into the pool and adds a leaf holding it (Odin
    /// `add_leaf(b, kind, span, add_text(b, s))`).
    pub fn add_text_leaf(
        &mut self,
        kind: NodeKind,
        span: SourceSpan,
        s: &str,
    ) -> Result<NodeId, Fail> {
        let text = self.add_text(s)?;
        self.add_leaf(kind, span, text)
    }

    /// Copies `s` into the text pool and returns where it is.
    pub fn add_text(&mut self, s: &str) -> Result<Text, Fail> {
        let start = self.text_open();
        self.text_push(s)?;
        Ok(self.text_close(start))
    }

    /// Starts a run of pushed text: the offset `text_close` measures from.
    pub fn text_open(&self) -> u32 {
        u32::try_from(self.snap.text.len()).unwrap_or(u32::MAX)
    }

    /// Returns the bytes in the pool (Odin `b.snap.text_used`).
    pub fn text_used(&self) -> usize {
        self.snap.text.len()
    }

    /// Copies `s` into the text pool.
    pub fn text_push(&mut self, s: &str) -> Result<(), Fail> {
        self.text_push_bytes(s.as_bytes())
    }

    /// Copies raw bytes into the text pool; a run must end on a character boundary
    /// before the snapshot is validated.
    pub fn text_push_bytes(&mut self, bytes: &[u8]) -> Result<(), Fail> {
        self.check()?;
        let used = self.snap.text.len();
        let wanted = used.saturating_add(bytes.len());
        let max = self.limits.max_text_bytes.min(u32::MAX as usize);
        if wanted > max {
            return Err(self.capacity(Region::Text, (wanted - max) as u64));
        }
        if self.snap.text.try_reserve(bytes.len()).is_err() {
            return Err(self.capacity(Region::Text, bytes.len() as u64));
        }
        self.snap.text.extend_from_slice(bytes);
        Ok(())
    }

    /// Copies one byte into the text pool.
    pub fn text_push_byte(&mut self, c: u8) -> Result<(), Fail> {
        self.text_push_bytes(&[c])
    }

    /// Copies `c`, UTF-8 encoded, into the text pool (Odin `text_push_rune`).
    pub fn text_push_char(&mut self, c: char) -> Result<(), Fail> {
        let mut bytes = [0u8; 4];
        self.text_push(c.encode_utf8(&mut bytes))
    }

    /// Returns the text pushed since `start` (`text_open`); empty once failed.
    pub fn text_close(&self, start: u32) -> Text {
        if self.status != Status::Ok {
            return Text::default();
        }
        Text { start, len: self.text_open().saturating_sub(start) }
    }

    /// Drops text pushed since `start`; used when a tentative parse is abandoned.
    pub fn text_rewind(&mut self, start: u32) {
        let start = start as usize;
        if start <= self.snap.text.len() {
            self.snap.text.truncate(start);
        }
    }

    /// Returns the text `t` of the pool; "" when out of range or not UTF-8.
    pub fn pool_string(&self, t: Text) -> &str {
        self.snap.pool_str(t)
    }

    /// Returns node `id` mutably when the builder has not failed and the id exists.
    fn live_node(&mut self, id: NodeId) -> Option<&mut Node> {
        if self.status != Status::Ok {
            return None;
        }
        self.snap.nodes.get_mut(id.index())
    }

    /// Sets the text of node `id`; nothing happens once failed.
    pub fn set_text(&mut self, id: NodeId, t: Text) {
        if let Some(n) = self.live_node(id) {
            n.text = t;
        }
    }

    /// Sets the value of node `id` (a level, a count); nothing happens once failed.
    pub fn set_value(&mut self, id: NodeId, v: i32) {
        if let Some(n) = self.live_node(id) {
            n.value = v;
        }
    }

    /// Adds flags to node `id`; nothing happens once failed.
    pub fn add_flags(&mut self, id: NodeId, f: NodeFlags) {
        if let Some(n) = self.live_node(id) {
            n.flags.insert(f);
        }
    }

    /// Moves the end of node `id`'s span, as a reader learns where a construct ends.
    pub fn set_span_end(&mut self, id: NodeId, end: u32) {
        if let Some(n) = self.live_node(id) {
            n.span.end = end;
        }
    }

    /// Changes the kind of node `id`, as a pass does to a copy; validation checks it.
    pub fn set_kind(&mut self, id: NodeId, k: NodeKind) {
        if let Some(n) = self.live_node(id) {
            n.kind = k;
        }
    }

    /// Adds an attribute to node `id`, which must be the most recently begun node, so
    /// each node's attributes stay contiguous.
    pub fn add_attribute(
        &mut self,
        id: NodeId,
        key: AttributeKey,
        value: Text,
    ) -> Result<(), Fail> {
        self.add_attribute_named(id, key, value, Text::default())
    }

    /// Adds an attribute whose Custom key is the pool text `name` (Odin `add_attribute`
    /// with its `name` argument).
    pub fn add_attribute_named(
        &mut self,
        id: NodeId,
        key: AttributeKey,
        value: Text,
        name: Text,
    ) -> Result<(), Fail> {
        self.check()?;
        let Some(node) = self.snap.nodes.get(id.index()).copied() else {
            return Err(self.invalid(
                id,
                "an attribute names a missing node",
                SourceSpan::default(),
            ));
        };
        let last = id.index() + 1 == self.snap.nodes.len();
        let contiguous = node.attributes.range().end == self.snap.attributes.len();
        if !last || !contiguous {
            return Err(self.invalid(id, "attribute added after a child", node.span));
        }
        if self.snap.attributes.try_reserve(1).is_err() {
            return Err(self.capacity(Region::Attributes, 1));
        }
        self.snap.attributes.push(Attribute { key, name, value });
        self.snap.nodes[id].attributes.count += 1;
        Ok(())
    }

    /// Copies `value` into the pool and adds it as attribute `key` of node `id`.
    pub fn add_attribute_string(
        &mut self,
        id: NodeId,
        key: AttributeKey,
        value: &str,
    ) -> Result<(), Fail> {
        let value = self.add_text(value)?;
        self.add_attribute(id, key, value)
    }

    /// Copies `name` and `value` into the pool and adds them as a Custom attribute
    /// (Odin `add_attribute_string(b, id, .Custom, value, name)`).
    pub fn add_custom_attribute(
        &mut self,
        id: NodeId,
        name: &str,
        value: &str,
    ) -> Result<(), Fail> {
        let name = self.add_text(name)?;
        let value = self.add_text(value)?;
        self.add_attribute_named(id, AttributeKey::Custom, value, name)
    }

    /// Stores `info` as node `id`'s list payload.
    pub fn set_list(&mut self, id: NodeId, info: ListInfo) -> Result<(), Fail> {
        self.check()?;
        if self.snap.lists.try_reserve(1).is_err() {
            return Err(self.capacity(Region::Lists, 1));
        }
        let row = self.snap.lists.len();
        self.snap.lists.push(info);
        self.set_payload(id, row);
        Ok(())
    }

    /// Stores `info` as node `id`'s link payload and returns its row, for
    /// `resolve_link`.
    pub fn set_link(&mut self, id: NodeId, info: LinkInfo) -> Result<u32, Fail> {
        self.check()?;
        if self.snap.links.try_reserve(1).is_err() {
            return Err(self.capacity(Region::Links, 1));
        }
        let row = self.snap.links.len();
        self.snap.links.push(info);
        self.set_payload(id, row);
        Ok(u32::try_from(row).unwrap_or(NONE))
    }

    /// Stores `info` as node `id`'s cell payload.
    pub fn set_cell(&mut self, id: NodeId, info: CellInfo) -> Result<(), Fail> {
        self.check()?;
        if self.snap.cells.try_reserve(1).is_err() {
            return Err(self.capacity(Region::Cells, 1));
        }
        let row = self.snap.cells.len();
        self.snap.cells.push(info);
        self.set_payload(id, row);
        Ok(())
    }

    fn set_payload(&mut self, id: NodeId, row: usize) {
        if let Some(n) = self.snap.nodes.get_mut(id.index()) {
            n.payload = u32::try_from(row).unwrap_or(NONE);
        }
    }

    /// Records the resolution of link row `row` (from `set_link`): its target, kind,
    /// and, when not empty, a new destination.
    pub fn resolve_link(&mut self, row: u32, target: NodeId, kind: LinkKind, destination: Text) {
        if self.status != Status::Ok {
            return;
        }
        if let Some(link) = self.snap.links.get_mut(row as usize) {
            link.resolved = target;
            link.kind = kind;
            if destination.len > 0 {
                link.destination = destination;
            }
        }
    }

    /// Closes the root. Every other node must already be closed.
    pub fn finish(&mut self) -> Result<(), Fail> {
        self.check()?;
        if self.open.len() != 1 {
            let top = self.current().unwrap_or(ROOT);
            let span = self.snap.nodes.get(top.index()).map(|n| n.span).unwrap_or_default();
            return Err(self.invalid(top, "left open by the reader", span));
        }
        self.end_node(ROOT)
    }

    /// Removes a closed, non-root node and keeps its descendants in place, as when an RST
    /// section becomes the document title. Links resolved to later nodes follow them.
    pub fn unwrap_node(&mut self, id: NodeId) -> Result<(), Fail> {
        self.check()?;
        let closed =
            self.snap.nodes.get(id.index()).is_some_and(|n| n.subtree_end != BUILDER_OPEN);
        if id == ROOT || !closed {
            return Err(self.invalid(
                id,
                "only a closed, non-root node can be unwrapped",
                SourceSpan::default(),
            ));
        }
        self.snap.nodes.remove(id.index());
        for n in &mut self.snap.nodes {
            if n.subtree_end != BUILDER_OPEN && n.subtree_end > id.0 {
                n.subtree_end -= 1;
            }
        }
        for open in self.open.iter_mut().filter(|o| **o > id) {
            *open = *open - 1;
        }
        for link in &mut self.snap.links {
            if link.kind == LinkKind::Internal
                && link.resolved > id
                && link.resolved != INVALID_NODE
            {
                link.resolved = link.resolved - 1;
            }
        }
        Ok(())
    }

    /// Copies a node of `doc` and its descendants, with their text, attributes, and
    /// payload rows, as the last child of the current node, and returns the copy of `id`.
    /// Copies are appended one per node, so the copy of a descendant `d` is
    /// `copy + (d - id)`: a pass adjusts copies with `set_value`, `set_text`,
    /// `add_flags`, and `set_kind`.
    pub fn copy_subtree(&mut self, doc: Document<'_>, id: NodeId) -> Result<NodeId, Fail> {
        let copy = NodeId::from_index(self.snap.nodes.len());
        self.copy_range(doc, id, doc.subtree_end(id))?;
        Ok(copy)
    }

    /// Copies nodes `first..end` in order. An open copy's source is found by offset, so
    /// no stack is needed beyond the builder's own.
    fn copy_range(&mut self, doc: Document<'_>, first: NodeId, end: NodeId) -> Result<(), Fail> {
        let base = NodeId::from_index(self.snap.nodes.len());
        let floor = self.open.len();
        for i in first.0..end.0 {
            let i = NodeId(i);
            self.close_copies(doc, first, base, floor, i)?;
            let n = doc.node(i);
            let copy = self.begin_node(n.kind, n.span)?;
            self.set_value(copy, n.value);
            self.add_flags(copy, n.flags);
            let text = self.add_text(doc.node_text(i))?;
            self.set_text(copy, text);
            for a in doc.attributes(i) {
                let value = self.add_text(doc.text(a.value))?;
                let name = self.add_text(doc.text(a.name))?;
                self.add_attribute_named(copy, a.key, value, name)?;
            }
            self.copy_payload(doc, i, copy)?;
        }
        self.close_copies(doc, first, base, floor, end)
    }

    /// Ends open copies whose source subtree ends at or before `i`.
    fn close_copies(
        &mut self,
        doc: Document<'_>,
        first: NodeId,
        base: NodeId,
        floor: usize,
        i: NodeId,
    ) -> Result<(), Fail> {
        while let Some(top) = self.current().filter(|_| self.open.len() > floor) {
            let source = NodeId(first.0 + (top.0 - base.0));
            if doc.subtree_end(source) > i {
                return Ok(());
            }
            self.end_node(top)?;
        }
        Ok(())
    }

    /// Copies node `from`'s list, cell, or link row of `doc`, with its texts, to node
    /// `to` of this builder.
    pub fn copy_payload(
        &mut self,
        doc: Document<'_>,
        from: NodeId,
        to: NodeId,
    ) -> Result<(), Fail> {
        if doc.node(from).payload == NONE {
            return Ok(());
        }
        match payload_of(doc.kind(from)) {
            Payload::None => Ok(()),
            Payload::List => self.set_list(to, doc.list_info(from)),
            Payload::Cell => self.set_cell(to, doc.cell_info(from)),
            Payload::Link => {
                let mut l = doc.link_info(from).unwrap_or(LinkInfo::UNRESOLVED);
                l.destination = self.add_text(doc.text(l.destination))?;
                l.title = self.add_text(doc.text(l.title))?;
                l.name = self.add_text(doc.text(l.name))?;
                self.set_link(to, l).map(|_| ())
            }
        }
    }
}

/// Writing into a builder appends to its text pool; a failure poisons the builder, which
/// the next fallible call reports.
impl TextSink for Builder<'_> {
    fn put_str(&mut self, s: &str) {
        let _ = self.text_push(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::default_limits;

    #[test]
    fn builder_rejects_illegal_nesting_and_poisons() {
        let (mut snap, mut reports, mut open) = (Snapshot::default(), Reports::new(8), Vec::new());
        let mut b = Builder::new(
            &mut snap,
            &mut reports,
            &mut open,
            default_limits(),
            SourceSpan::default(),
        );
        let p = b.begin_node(NodeKind::Paragraph, SourceSpan::default()).ok();
        assert!(b.begin_node(NodeKind::Paragraph, SourceSpan::default()).is_err());
        assert_eq!(b.status(), Status::Internal);
        assert!(b.add_leaf(NodeKind::Text, SourceSpan::default(), Text::default()).is_err());
        assert!(p.is_some_and(|p| b.end_node(p).is_err()));
        assert!(b.finish().is_err());
    }

    #[test]
    fn depth_limit_is_a_result() {
        let (mut snap, mut reports, mut open) = (Snapshot::default(), Reports::new(8), Vec::new());
        let limits = Limits { max_depth: 3, ..default_limits() };
        let mut b =
            Builder::new(&mut snap, &mut reports, &mut open, limits, SourceSpan::default());
        for _ in 0..5 {
            let _ = b.begin_node(NodeKind::BlockQuote, SourceSpan::default());
        }
        assert_eq!(b.status(), Status::Limit);
        assert_eq!(reports.primary().map(|d| d.code), Some("core.limit.depth"));
    }

    #[test]
    fn text_runs_and_attributes() -> Result<(), Fail> {
        let (mut snap, mut reports, mut open) = (Snapshot::default(), Reports::new(8), Vec::new());
        let mut b = Builder::new(
            &mut snap,
            &mut reports,
            &mut open,
            default_limits(),
            SourceSpan::default(),
        );
        let p = b.begin_node(NodeKind::Paragraph, SourceSpan::default())?;
        b.add_attribute_string(p, AttributeKey::Class, "lead")?;
        let start = b.text_open();
        b.text_push("é")?;
        b.text_push_byte(b'!')?;
        let t = b.text_close(start);
        assert_eq!(b.pool_string(t), "é!");
        let leaf = b.add_leaf(NodeKind::Text, SourceSpan::default(), t)?;
        assert!(b.add_attribute_string(p, AttributeKey::Id, "late").is_err());
        assert_eq!(leaf, NodeId(2));
        Ok(())
    }
}
