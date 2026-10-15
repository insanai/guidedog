//! The order of anchors: `build_anchors` gives ids in the order Docutils 0.22 gives them
//! (docutils/parsers/rst/states.py, nodes.py `set_id`, transforms/references.py and
//! parts.py), since that order numbers the "idN" ids. First the parser names elements as
//! it meets them, in document order (`parse_order`); then the transforms add ids, in
//! their priority order (`transform_order`):
//!
//! - AutoNumbering (210, Sphinx only): each figure, table, and code block with a caption
//!   or title and no id yet takes the next "idN";
//! - Substitutions (220): an undefined substitution reference becomes problematic: an
//!   id for the system message, which Guidedoc does not write, and one for the reference;
//! - AnonymousHyperlinks (440): when anonymous references and targets do not pair up,
//!   the same for the message and every anonymous reference;
//! - TargetNotes (540): a footnote per external URI referred to by name, then a footnote
//!   reference after each such reference;
//! - Contents (720): an entry per section a table of contents lists;
//! - DanglingReferences (850): an unresolved reference becomes problematic: the message
//!   takes an id, and the reference takes one unless it has one.
//!
//! Sphinx's SortIds (261) then moves a section's first id to the end when it starts with
//! "id" and others follow (`sort_section_ids`, once every id is given).
//!
//! The parser's part follows the source text, each include read in place. The tree does
//! not keep that order everywhere (title promotion moves a title to the start), so the
//! order comes from the nodes' source positions, and the tree breaks ties.

use crate::anchors::{AnchorExtra, Anchors, Chain};
use crate::document::Document;
use crate::types::{
    AdmonitionKind, AttributeKey, INVALID_NODE, LinkKind, NONE, NodeFlag, NodeFlags, NodeId,
    NodeKind, ROOT,
};

/// Gives a document's ids in Docutils' order.
pub(crate) struct Order<'a, 'd> {
    a: &'a mut Anchors,
    doc: Document<'d>,
    /// The Sphinx layer read the document: its transforms apply too.
    sphinx: bool,
    /// Headings outside sections have ids (not in the reference profile).
    headings: bool,
    /// The implicit name of an untitled table of contents.
    contents: &'d str,
}

/// The working storage of `Order`, kept in the anchor table so a rebuilt table reuses
/// it.
#[derive(Debug, Clone, Default)]
pub(crate) struct OrderWork {
    positions: Positions,
    events: Vec<ParseEvent>,
    /// The ends of the sections enclosing a node, innermost last.
    ends: Vec<u32>,
    /// Room for a node's own name.
    own: String,
}

/// Where the parser names a node: where it meets it, or later, after the part of it
/// that `naming_point` names.
#[derive(Debug, Clone, Copy)]
struct ParseEvent {
    /// Position in the source text read with includes in place.
    at: u64,
    /// Position in the tree, for events at one source position.
    tie: u32,
    node: NodeId,
    later: bool,
}

/// Maps a node's source position into the text Docutils reads: every source with its
/// included sources in place.
#[derive(Debug, Clone, Default)]
struct Positions {
    /// Per source: where its text starts.
    base: Vec<u64>,
    /// Per source: its length with its includes.
    expanded: Vec<u64>,
    /// The position of the last node placed, for nodes without one.
    last: u64,
}

impl<'a, 'd> Order<'a, 'd> {
    pub(crate) fn new(
        a: &'a mut Anchors,
        doc: Document<'d>,
        sphinx: bool,
        headings: bool,
        contents: &'d str,
    ) -> Self {
        Order { a, doc, sphinx, headings, contents }
    }

    fn give(&mut self, id: NodeId, name: &str, chain: Chain) {
        self.a.assign(id, name, chain);
    }

    /// Names elements in the order Docutils' parser names them.
    pub(crate) fn parse_order(&mut self) {
        let doc = self.doc;
        let mut p = std::mem::take(&mut self.a.work.positions);
        p.measure(doc);
        let mut events = std::mem::take(&mut self.a.work.events);
        events.clear();
        for i in 0..doc.node_count() {
            let id = NodeId::from_index(i);
            let start = p.position(doc, id, false);
            if named_at_start(doc, id) {
                events.push(ParseEvent { at: start, tie: 2 * id.0 + 1, node: id, later: false });
            }
            if let Some(by) = self.naming_point(id) {
                // A topic or sidebar without a title is named where it starts.
                let untitled =
                    by == id && matches!(doc.kind(id), NodeKind::Topic | NodeKind::Sidebar);
                let at = if untitled { start } else { p.position(doc, by, true) };
                events.push(ParseEvent {
                    at,
                    tie: 2 * doc.subtree_end(by).0,
                    node: id,
                    later: true,
                });
            }
        }
        // Inner elements are named first at one position. No two events compare equal
        // (a node's two ties differ in parity), so unstable sorting, which does not
        // allocate, gives the same order.
        events.sort_unstable_by(|x, y| {
            x.at.cmp(&y.at).then(x.tie.cmp(&y.tie)).then(y.node.cmp(&x.node))
        });
        for e in &events {
            if e.later {
                self.name_later(e.node);
            } else {
                self.name_start(e.node);
            }
        }
        self.a.work.positions = p;
        self.a.work.events = events;
    }

    /// Gives the ids the parser gives a node where it meets it.
    fn name_start(&mut self, id: NodeId) {
        let doc = self.doc;
        for at in doc.attributes(id).iter().filter(|a| a.key == AttributeKey::Id) {
            self.a.assign_verbatim(id, doc.text(at.value));
        }
        self.name_at_start(id);
        if self.naming_point(id).is_some() {
            return;
        }
        if let Some(name) = element_name(doc, id) {
            self.give(id, name, Chain::Own);
        }
    }

    /// Names a node the parser names after reading its title or content. A node a host
    /// marks "pinned-ids" has all its ids as Id attributes already, so it gets none.
    fn name_later(&mut self, id: NodeId) {
        let doc = self.doc;
        if doc.custom_attribute(id, "pinned-ids").is_some() {
            return;
        }
        match element_name(doc, id) {
            Some(name) => self.give(id, name, Chain::Own),
            None => self.name_own(id),
        }
    }

    /// Gives a node an id from its own name.
    fn name_own(&mut self, id: NodeId) {
        let mut own = std::mem::take(&mut self.a.work.own);
        own.clear();
        own_name(self.doc, id, &mut own);
        self.give(id, &own, Chain::Own);
        self.a.work.own = own;
    }

    /// Gives the ids the parser gives where it meets the node.
    fn name_at_start(&mut self, id: NodeId) {
        let doc = self.doc;
        match doc.kind(id) {
            NodeKind::Target => self.name_target(id),
            NodeKind::InlineTarget | NodeKind::Citation => {
                self.give(id, doc.node_text(id), Chain::Own)
            }
            NodeKind::Footnote => {
                let text = doc.node_text(id);
                let label = match text.strip_prefix('#') {
                    Some(rest) => rest,
                    None if text == "*" => "",
                    None => text,
                };
                self.give(id, label, Chain::Own);
            }
            NodeKind::FootnoteReference | NodeKind::CitationReference => {
                self.give(id, "", Chain::Own)
            }
            NodeKind::Link => {
                // `text <uri>`_ names a target Docutils does not write.
                if let Some(l) = doc.link_info(id).filter(|l| l.name.len > 0) {
                    self.give(INVALID_NODE, doc.text(l.name), Chain::Hidden);
                }
            }
            NodeKind::Reference => {
                // `text <name_>`_ names an indirect target, which Docutils does not write.
                if let Some(name) = doc.custom_attribute(id, "target-name") {
                    self.give(INVALID_NODE, name, Chain::Hidden);
                }
            }
            NodeKind::Contents => {
                let title = doc.attribute(id, AttributeKey::Title).unwrap_or(self.contents);
                self.give(id, title, Chain::Own);
            }
            NodeKind::Admonition => {
                // Sphinx makes each todo an explicit target, so a todolist can link back.
                let todo = doc.node(id).value == AdmonitionKind::Todo as i32;
                if todo && element_name(doc, id).is_none() {
                    self.give(id, "", Chain::Own);
                }
            }
            _ => {}
        }
    }

    /// A label gives its id to the element it names (or keeps it, when nothing follows
    /// that can take it); external, indirect, and anonymous external targets are not
    /// written, but their names take ids all the same.
    fn name_target(&mut self, id: NodeId) {
        let doc = self.doc;
        let anonymous = doc.node(id).flags.has(NodeFlag::Anonymous);
        let name = if anonymous { "" } else { doc.node_text(id) };
        let link = doc.link_info(id);
        let label = link.filter(|l| {
            l.kind == LinkKind::Internal
                && l.name.len == 0
                && l.destination.len == 0
                && l.resolved.index() < doc.node_count()
        });
        match label {
            // Docutils moves a label's id to the field list after it before that list
            // becomes the bibliographic fields, which lose it.
            None => self.give(INVALID_NODE, name, Chain::Hidden),
            Some(l) if doc.has_class(l.resolved, "docinfo") => {
                self.give(INVALID_NODE, name, Chain::Hidden)
            }
            Some(l) if l.resolved == id => self.give(id, name, Chain::Own),
            Some(l) => self.give(l.resolved, name, Chain::Label),
        }
    }

    /// Tells whether the parser names a node only after reading part of it, and after
    /// which node: a section after its title, a promoted title, a subtitle, and a heading
    /// after their text, and an element with a :name: option whose directive parses its
    /// title or content first (a topic or sidebar after its title).
    fn naming_point(&self, id: NodeId) -> Option<NodeId> {
        let doc = self.doc;
        let first = doc.first_child(ROOT);
        let named_after_self = |later: bool| if later { Some(id) } else { None };
        match doc.kind(id) {
            NodeKind::Section => {
                let titled =
                    id.index() + 1 < doc.node_count() && doc.kind(id + 1) == NodeKind::Heading;
                return Some(if titled { id + 1 } else { id });
            }
            NodeKind::Title => return named_after_self(first == Some(id)),
            NodeKind::Subtitle => {
                let after_title = first
                    .is_some_and(|f| doc.kind(f) == NodeKind::Title && doc.subtree_end(f) == id);
                return named_after_self(after_title);
            }
            NodeKind::Paragraph => return named_after_self(doc.has_class(id, "section-subtitle")),
            NodeKind::Heading => {
                let owned = id.0 > 0 && doc.kind(id - 1) == NodeKind::Section;
                return named_after_self(self.headings && !owned);
            }
            _ => {}
        }
        element_name(doc, id)?;
        match doc.kind(id) {
            NodeKind::Table | NodeKind::Rubric | NodeKind::Preformatted => Some(id),
            NodeKind::Topic | NodeKind::Sidebar => Some(last_title(doc, id)),
            _ => None,
        }
    }

    /// Gives the ids Docutils' transforms give, in their order.
    pub(crate) fn transform_order(&mut self) {
        let doc = self.doc;
        let ids = || (0..doc.node_count()).map(NodeId::from_index);
        if self.sphinx {
            for id in ids() {
                if self.a.index[id.index()] == NONE && numbered_with_title(doc, id) {
                    self.give(id, "", Chain::Own);
                }
            }
        }
        for id in ids().filter(|&id| doc.kind(id) == NodeKind::SubstitutionReference) {
            if doc.link_info(id).is_some_and(|l| l.kind == LinkKind::Internal) {
                continue;
            }
            self.give(INVALID_NODE, "", Chain::Hidden);
            self.give(id, "", Chain::Own);
        }
        self.anonymous_mismatch();
        for id in ids().filter(|&id| doc.kind(id) == NodeKind::Directive) {
            if doc.attribute(id, AttributeKey::Name) == Some("target-notes") {
                self.target_notes(id);
            }
        }
        for id in ids().filter(|&id| doc.kind(id) == NodeKind::Contents) {
            self.contents_entries(id);
        }
        for id in ids() {
            self.dangling_reference(id);
        }
    }

    /// DanglingReferences: an unresolved reference becomes problematic.
    fn dangling_reference(&mut self, id: NodeId) {
        let doc = self.doc;
        let k = doc.kind(id);
        if !matches!(
            k,
            NodeKind::Reference | NodeKind::FootnoteReference | NodeKind::CitationReference
        ) {
            return;
        }
        let link = doc.link_info(id);
        if link.is_some_and(|l| l.kind == LinkKind::Internal) {
            return;
        }
        // A node without a link row counts as unresolved: nothing has linked it yet.
        let kind = link.map_or(LinkKind::Unresolved, |l| l.kind);
        if k == NodeKind::Reference
            && (doc.node(id).flags.has(NodeFlag::Anonymous) || kind != LinkKind::Unresolved)
        {
            return;
        }
        self.give(INVALID_NODE, "", Chain::Hidden);
        if k == NodeKind::Reference {
            // Footnote and citation references keep their own ids.
            self.give(id, "", Chain::Own);
        }
    }

    /// When anonymous references and targets do not pair up, every anonymous reference
    /// is left unresolved and becomes problematic.
    fn anonymous_mismatch(&mut self) {
        let doc = self.doc;
        let mut first = true;
        for i in 0..doc.node_count() {
            let id = NodeId::from_index(i);
            let anonymous =
                doc.kind(id) == NodeKind::Reference && doc.node(id).flags.has(NodeFlag::Anonymous);
            let unresolved = doc.link_info(id).is_none_or(|l| l.kind == LinkKind::Unresolved);
            if !anonymous || !unresolved {
                continue;
            }
            if first {
                self.give(INVALID_NODE, "", Chain::Hidden);
            }
            first = false;
            self.give(id, "", Chain::Own);
        }
    }

    /// Gives an id to each entry of a table of contents: the sections in its scope (the
    /// enclosing section's for a local one), within its depth.
    fn contents_entries(&mut self, contents: NodeId) {
        let doc = self.doc;
        let (first, end, base) = contents_scope(doc, contents);
        let limit = doc.node(contents).value;
        let mut ends = std::mem::take(&mut self.a.work.ends);
        ends.clear();
        for i in 1..doc.node_count() {
            let id = NodeId::from_index(i);
            while ends.last().is_some_and(|&e| e <= id.0) {
                ends.pop();
            }
            if doc.kind(id) != NodeKind::Section {
                continue;
            }
            ends.push(doc.subtree_end(id).0);
            let depth = ends.len() as i64 - base as i64;
            if id < first || id >= end || (limit > 0 && depth > i64::from(limit)) {
                continue;
            }
            if id + 1 >= doc.subtree_end(id) || doc.kind(id + 1) != NodeKind::Heading {
                continue;
            }
            self.a.add_extra(AnchorExtra { owner: contents, item: id, ..AnchorExtra::default() });
        }
        self.a.work.ends = ends;
    }

    /// Gives ids to the footnotes a target-notes directive makes, one per external URI a
    /// reference names (through a named target) or an anonymous reference leads to, and
    /// to the footnote reference it adds after each such reference.
    fn target_notes(&mut self, directive: NodeId) {
        let doc = self.doc;
        let n = doc.node_count();
        let mut number = first_free_note_number(doc);
        for t in (0..n).map(NodeId::from_index) {
            if doc.kind(t) != NodeKind::Target || doc.node(t).flags.has(NodeFlag::Anonymous) {
                continue;
            }
            let Some(l) = doc.link_info(t) else { continue };
            let name = doc.node_text(t);
            if l.kind != LinkKind::External
                || l.destination.len == 0
                || name.is_empty()
                || !named_by_reference(doc, t, name)
            {
                continue;
            }
            let note = self.note_for(directive, t, &mut number);
            for r in (0..n).map(NodeId::from_index) {
                let named = doc.link_info(r).is_some_and(|rl| doc.text(rl.name) == name);
                if doc.kind(r) == NodeKind::Reference
                    && !doc.node(r).flags.has(NodeFlag::Anonymous)
                    && named
                {
                    self.add_note_reference(r, directive, note);
                }
            }
        }
        for r in (0..n).map(NodeId::from_index) {
            let anonymous =
                doc.kind(r) == NodeKind::Reference && doc.node(r).flags.has(NodeFlag::Anonymous);
            if anonymous && doc.link_info(r).is_some_and(|l| l.kind == LinkKind::External) {
                let note = self.note_for(directive, r, &mut number);
                self.add_note_reference(r, directive, note);
            }
        }
    }

    /// Returns the footnote listing the URI of `item`, making it when the directive has
    /// none yet for that URI.
    fn note_for(&mut self, directive: NodeId, item: NodeId, number: &mut i32) -> AnchorExtra {
        let doc = self.doc;
        let destination = |id: NodeId| doc.link_info(id).map_or("", |l| doc.text(l.destination));
        let uri = destination(item);
        let existing =
            self.a.extras.iter().find(|e| e.owner == directive && destination(e.item) == uri);
        if let Some(e) = existing {
            return *e;
        }
        let e = AnchorExtra { owner: directive, item, number: *number, ..AnchorExtra::default() };
        *number += 1;
        self.a.add_extra(e);
        e
    }

    fn add_note_reference(&mut self, reference: NodeId, directive: NodeId, note: AnchorExtra) {
        let e = AnchorExtra {
            owner: reference,
            item: directive,
            note: note.item,
            number: note.number,
            span: 0,
        };
        self.a.add_extra(e);
    }
}

impl Anchors {
    /// Sphinx's SortIds: a section whose first id starts with "id" and has others takes
    /// that id last. Pinned ids were sorted when they were read.
    pub(crate) fn sort_section_ids(&mut self, doc: Document<'_>) {
        for i in 0..doc.node_count() {
            let id = NodeId::from_index(i);
            let first = self.index[i];
            if doc.kind(id) != NodeKind::Section
                || first == NONE
                || self.next[first as usize] == NONE
            {
                continue;
            }
            if doc.custom_attribute(id, "pinned-ids").is_some() {
                continue;
            }
            let text = self.span_text(first);
            if !text.get(self.prefix_length.min(text.len())..).unwrap_or("").starts_with("id") {
                continue;
            }
            self.index[i] = self.next[first as usize];
            self.next[first as usize] = NONE;
            self.index[i] = self.append_span(self.index[i], first);
        }
    }
}

impl Positions {
    /// Measures every source of `doc` with its includes, and where each starts, reusing
    /// the tables. Sources come after the source including them.
    fn measure(&mut self, doc: Document<'_>) {
        let sources = doc.sources();
        let count = sources.len();
        let p = self;
        p.base.clear();
        p.base.resize(count, 0);
        p.expanded.clear();
        p.expanded.resize(count, 0);
        p.last = 0;
        for i in (0..count).rev() {
            p.expanded[i] = doc.source_str(source_id(i)).len() as u64;
            for c in i + 1..count {
                if included_in(doc, c, i) {
                    p.expanded[i] += p.expanded[c];
                }
            }
        }
        for (i, source) in sources.iter().enumerate() {
            let parent = source.parent.index();
            if parent < i {
                p.base[i] = p.place(doc, parent, source.included_at.start);
            }
        }
    }

    /// Where an offset of a source lies in the text read with includes in place: the
    /// sources it includes before that offset come first.
    fn place(&self, doc: Document<'_>, source: usize, offset: u32) -> u64 {
        let mut at = self.base[source] + u64::from(offset);
        for c in source + 1..doc.sources().len() {
            if included_in(doc, c, source) && doc.sources()[c].included_at.start < offset {
                at += self.expanded[c];
            }
        }
        at
    }

    /// Where a node starts (or ends) in the text Docutils reads. A node without a source
    /// position of its own is placed with the node before it.
    fn position(&mut self, doc: Document<'_>, id: NodeId, end: bool) -> u64 {
        let span = doc.node(id).span;
        let source = span.source.index();
        if source >= doc.sources().len() || (span.start == 0 && span.end == 0 && id != ROOT) {
            return self.last;
        }
        self.last = self.place(doc, source, if end { span.end } else { span.start });
        self.last
    }
}

fn source_id(i: usize) -> crate::types::SourceId {
    crate::types::SourceId(u16::try_from(i).unwrap_or(u16::MAX))
}

fn included_in(doc: Document<'_>, source: usize, parent: usize) -> bool {
    doc.sources()[source].parent.index() == parent && source != parent
}

/// The last Title or Subtitle that starts a topic or sidebar, or the node itself.
fn last_title(doc: Document<'_>, id: NodeId) -> NodeId {
    let mut by = id;
    for c in doc.children(id) {
        if !matches!(doc.kind(c), NodeKind::Title | NodeKind::Subtitle) {
            break;
        }
        by = c;
    }
    by
}

/// The parser may give the node an id where it meets it.
fn named_at_start(doc: Document<'_>, id: NodeId) -> bool {
    use NodeKind as K;
    let always = matches!(
        doc.kind(id),
        K::Target
            | K::InlineTarget
            | K::Citation
            | K::Footnote
            | K::FootnoteReference
            | K::CitationReference
            | K::Link
            | K::Reference
            | K::Contents
            | K::Admonition
    );
    always
        || element_name(doc, id).is_some()
        || doc.attributes(id).iter().any(|a| a.key == AttributeKey::Id)
}

/// An element's :name: option: Name, or Custom "name" where the Sphinx layer keeps it
/// (code blocks). Directive nodes use Name for the directive's name; on a glossary Term,
/// Name is the term itself, whose id the reader sets.
fn element_name<'d>(doc: Document<'d>, id: NodeId) -> Option<&'d str> {
    match doc.kind(id) {
        NodeKind::Directive => doc.custom_attribute(id, "name"),
        NodeKind::Term => None,
        _ => doc.attribute(id, AttributeKey::Name).or_else(|| doc.custom_attribute(id, "name")),
    }
}

/// The name a node carries by itself, as Docutils makes it from the title of a section
/// (a promoted title and subtitle included) and a heading.
fn own_name(doc: Document<'_>, id: NodeId, out: &mut String) {
    match doc.kind(id) {
        NodeKind::Section
            if id.index() + 1 < doc.node_count() && doc.kind(id + 1) == NodeKind::Heading =>
        {
            title_name(doc, id + 1, out);
        }
        NodeKind::Heading | NodeKind::Title | NodeKind::Subtitle | NodeKind::Paragraph => {
            title_name(doc, id, out)
        }
        _ => {}
    }
}

/// Writes a title's text as Docutils makes its name: the text of text, literal, math,
/// and raw nodes, a substitution reference's name, a citation reference's label, and a
/// numbered footnote reference's number, since names come from the title before the
/// transforms replace or number them.
fn title_name(doc: Document<'_>, id: NodeId, out: &mut String) {
    for i in id.0 + 1..doc.subtree_end(id).0 {
        let i = NodeId(i);
        match doc.kind(i) {
            NodeKind::Text
            | NodeKind::Code
            | NodeKind::Math
            | NodeKind::RawInline
            | NodeKind::SubstitutionReference
            | NodeKind::CitationReference => out.push_str(doc.node_text(i)),
            // Auto-numbered references get their number only later.
            NodeKind::FootnoteReference
                if !doc.node(i).flags.intersects(NodeFlags::AUTO | NodeFlags::SYMBOL) =>
            {
                out.push_str(doc.node_text(i));
            }
            NodeKind::SoftBreak | NodeKind::HardBreak => out.push(' '),
            _ => {}
        }
    }
}

/// Returns the range of nodes a table of contents lists and the number of sections
/// around that range.
pub fn contents_scope(doc: Document<'_>, contents: NodeId) -> (NodeId, NodeId, usize) {
    let (mut first, mut end, mut base) = (ROOT + 1, doc.subtree_end(ROOT), 0);
    if !doc.node(contents).flags.has(NodeFlag::Local) {
        return (first, end, base);
    }
    for s in (0..contents.index()).map(NodeId::from_index) {
        if doc.kind(s) != NodeKind::Section || doc.subtree_end(s) <= contents {
            continue;
        }
        first = s + 1;
        end = doc.subtree_end(s);
        base += 1;
    }
    (first, end, base)
}

/// Some reference, or another target, names this target.
fn named_by_reference(doc: Document<'_>, target: NodeId, name: &str) -> bool {
    (0..doc.node_count()).map(NodeId::from_index).any(|id| {
        let k = doc.kind(id);
        let link_like = k == NodeKind::Reference || k == NodeKind::Target;
        link_like
            && id != target
            && doc.link_info(id).is_some_and(|l| l.name.len > 0 && doc.text(l.name) == name)
    })
}

/// The number target-notes footnotes start from: after every numbered footnote.
fn first_free_note_number(doc: Document<'_>) -> i32 {
    let mut last = 0i32;
    for id in (0..doc.node_count()).map(NodeId::from_index) {
        if doc.kind(id) != NodeKind::Footnote || doc.node(id).flags.has(NodeFlag::Symbol) {
            continue;
        }
        last = last.max(doc.node(id).value);
        let text = doc.node_text(id);
        let number =
            if !text.is_empty() && text.len() <= 7 && text.bytes().all(|c| c.is_ascii_digit()) {
                text.parse::<i32>().unwrap_or(0)
            } else {
                0
            };
        last = last.max(number);
    }
    last + 1
}

/// An element Sphinx's standard domain numbers (a figure, a table, a code block) with a
/// caption or title, which AutoNumbering gives an id.
fn numbered_with_title(doc: Document<'_>, id: NodeId) -> bool {
    match doc.kind(id) {
        NodeKind::Figure => doc.children(id).any(|c| doc.kind(c) == NodeKind::Caption),
        NodeKind::Table => {
            doc.first_child(id).is_some_and(|first| doc.kind(first) == NodeKind::Title)
        }
        NodeKind::CodeBlock => doc.custom_attribute(id, "caption").is_some(),
        _ => false,
    }
}
