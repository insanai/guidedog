//! Resolution runs once after parsing, over the finished snapshot. It records names in
//! maps, then fills link rows and values in place, and finally applies the structure
//! transforms (title promotion, subtitles, bibliographic fields, smart quotes).

mod docinfo;
mod docinfo_text;
mod edit;
mod footnotes;
mod references;
mod smartquotes;
mod smartquotes_text;
mod specialized;
mod structure;
mod substitutions;
mod tabs;
mod targets;

use guidedoc::{
    Arg, AttributeKey, Fail, LinkInfo, LinkKind, Message, NameMap, NodeFlags, NodeId, NodeKind,
    ReadContext, SourceSpan, Text, arg_span, normalize_label,
};

use crate::Options;
use crate::emit::check;

/// What kind of definition a name record comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordKind {
    /// Hyperlink target: external, indirect, or internal.
    Target,
    /// `` _`name` ``.
    InlineTarget,
    /// `` `text <uri>`_ `` names an external target.
    Link,
    /// `` `text <name_>`_ `` names an indirect target.
    Alias,
    /// Implicit target of a section title (or a table of contents).
    Section,
    /// An element with a `:name:` option.
    Named,
    Footnote,
    Citation,
}

/// Where following a record stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordState {
    Unvisited,
    Visiting,
    Done,
    Failed,
}

/// Where a target leads after following indirect targets.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Final {
    pub kind: LinkKind,
    pub node: NodeId,
    pub destination: Text,
}

impl Final {
    pub fn internal(node: NodeId) -> Final {
        Final { kind: LinkKind::Internal, node, destination: Text::default() }
    }
}

/// One name-bearing element.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Record {
    pub node: NodeId,
    pub kind: RecordKind,
    pub explicit: bool,
    pub state: RecordState,
    pub last: Final,
    /// A chained target leads where record `via` leads.
    pub via: Option<u32>,
}

/// What a name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Name {
    Record(u32),
    /// Only implicit targets define it several times; an explicit target may take it.
    Ambiguous,
    /// Explicit targets collided; the name stays unusable.
    AmbiguousExplicit,
}

/// A substitution definition and its edges for the cycle check.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Definition {
    pub node: NodeId,
    pub first_edge: usize,
    pub edge_count: usize,
    /// 0 unvisited, 1 on the search stack, 2 finished.
    pub color: u8,
}

/// The substitution definition `from` uses definition `to` at reference `at`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Edge {
    pub to: usize,
    pub at: NodeId,
}

/// A dialect's default substitutions (see `crate::resolve_with_substitutions`).
pub(crate) type Substitute<'r> = &'r dyn Fn(&str) -> Option<String>;

/// The resolver's tables, kept in the workspace's scratch between resolutions.
#[derive(Debug, Default)]
struct ResolverWork {
    records: Vec<Record>,
    names: NameMap<Name>,
    pending: Vec<u32>,
    anon_targets: Vec<u32>,
    anon_refs: Vec<NodeId>,
    auto_notes: Vec<NodeId>,
    symbol_notes: Vec<NodeId>,
    subs: NameMap<NodeId>,
    subs_folded: NameMap<NodeId>,
    defs: Vec<Definition>,
    edges: Vec<Edge>,
    key: String,
    plain: String,
    escaped: Vec<bool>,
}

/// The state of one resolution.
pub(crate) struct Resolver<'r, 'a> {
    pub ctx: &'r mut ReadContext<'a>,
    pub options: &'r Options,
    pub substitute: Option<Substitute<'r>>,
    pub records: Vec<Record>,
    pub names: NameMap<Name>,
    pub pending: Vec<u32>,
    pub anon_targets: Vec<u32>,
    pub anon_refs: Vec<NodeId>,
    /// Auto-numbered footnotes, labelled or not.
    pub auto_notes: Vec<NodeId>,
    pub symbol_notes: Vec<NodeId>,
    pub auto_cursor: usize,
    pub symbol_used: usize,
    pub auto_full: bool,
    pub symbol_full: bool,
    pub subs: NameMap<NodeId>,
    pub subs_folded: NameMap<NodeId>,
    pub defs: Vec<Definition>,
    pub edges: Vec<Edge>,
    /// Room for a folded name, reused (`key_buffer`).
    pub key: String,
    /// Room for a name before folding, or a text being rewritten, reused.
    pub plain: String,
    /// Which bytes of a text were escaped (smart quotes), reused.
    pub escaped: Vec<bool>,
}

/// See `crate::resolve`.
pub(crate) fn resolve_document(
    ctx: &mut ReadContext<'_>,
    options: &Options,
    substitute: Option<Substitute<'_>>,
) -> Result<(), Fail> {
    check(&mut ctx.b)?;
    let w: ResolverWork = ctx.take_scratch();
    let mut r = Resolver {
        ctx,
        options,
        substitute,
        records: w.records,
        names: w.names,
        pending: w.pending,
        anon_targets: w.anon_targets,
        anon_refs: w.anon_refs,
        auto_notes: w.auto_notes,
        symbol_notes: w.symbol_notes,
        auto_cursor: 0,
        symbol_used: 0,
        auto_full: false,
        symbol_full: false,
        subs: w.subs,
        subs_folded: w.subs_folded,
        defs: w.defs,
        edges: w.edges,
        key: w.key,
        plain: w.plain,
        escaped: w.escaped,
    };
    r.clear();
    let result = r.run();
    r.finish();
    result
}

impl Resolver<'_, '_> {
    /// Empties the tables taken from the workspace, keeping their storage.
    fn clear(&mut self) {
        self.records.clear();
        self.names.clear();
        self.pending.clear();
        self.anon_targets.clear();
        self.anon_refs.clear();
        self.auto_notes.clear();
        self.symbol_notes.clear();
        self.subs.clear();
        self.subs_folded.clear();
        self.defs.clear();
        self.edges.clear();
    }

    /// Gives the tables back to the workspace for the next resolution.
    fn finish(self) {
        let w = ResolverWork {
            records: self.records,
            names: self.names,
            pending: self.pending,
            anon_targets: self.anon_targets,
            anon_refs: self.anon_refs,
            auto_notes: self.auto_notes,
            symbol_notes: self.symbol_notes,
            subs: self.subs,
            subs_folded: self.subs_folded,
            defs: self.defs,
            edges: self.edges,
            key: self.key,
            plain: self.plain,
            escaped: self.escaped,
        };
        self.ctx.put_scratch(w);
    }

    /// Takes the reusable key buffer, cleared; give it back with `self.key = key`.
    pub fn key_buffer(&mut self) -> String {
        let mut key = std::mem::take(&mut self.key);
        key.clear();
        key
    }

    /// Finds what `name` refers to once folded as reference names compare.
    pub fn lookup_folded(&mut self, name: &str) -> Option<Name> {
        let mut key = self.key_buffer();
        normalize_label(&mut key, name);
        let found = self.lookup(&key);
        self.key = key;
        found
    }

    /// Runs the resolution's steps in order.
    fn run(&mut self) -> Result<(), Fail> {
        let r = self;
        r.specialize_roles()?;
        let used = r.used();
        r.ctx.b.spend(used as u64)?;
        r.records.reserve(used / 4);
        r.collect()?;
        r.resolve_targets()?;
        r.number_footnotes()?;
        r.resolve_references()?;
        r.resolve_anonymous()?;
        r.check_substitutions()?;
        r.unbreak_code();
        r.expand_tabs()?;
        r.restructure()?;
        check(&mut r.ctx.b)
    }
}

impl Resolver<'_, '_> {
    /// The number of nodes.
    pub fn used(&self) -> usize {
        self.ctx.b.snap.nodes.len()
    }

    pub fn kind_of(&self, id: NodeId) -> NodeKind {
        self.ctx.b.snap.nodes[id].kind
    }

    pub fn flags_of(&self, id: NodeId) -> NodeFlags {
        self.ctx.b.snap.nodes[id].flags
    }

    /// The node's text.
    pub fn text_of(&self, id: NodeId) -> &str {
        self.ctx.b.pool_string(self.ctx.b.snap.nodes[id].text)
    }

    /// The end of a node's subtree; the open root ends at the last node.
    pub fn node_end(&self, id: NodeId) -> usize {
        let end = self.ctx.b.snap.nodes[id].subtree_end;
        if end == guidedoc::BUILDER_OPEN { self.used() } else { end as usize }
    }

    /// Returns a node's link row and its index, if it has one.
    pub fn row(&self, id: NodeId) -> Option<(LinkInfo, u32)> {
        let p = self.ctx.b.snap.nodes[id].payload;
        let link = self.ctx.b.snap.links.get(p as usize)?;
        let has_link = matches!(
            self.kind_of(id),
            NodeKind::Reference
                | NodeKind::Link
                | NodeKind::Target
                | NodeKind::InlineTarget
                | NodeKind::FootnoteReference
                | NodeKind::CitationReference
                | NodeKind::SubstitutionReference
                | NodeKind::TitleReference
                | NodeKind::Image
                | NodeKind::Xref
        );
        (p != guidedoc::NONE && has_link).then_some((*link, p))
    }

    /// Returns a node's link row, adding one if the reader made none.
    pub fn ensure_row(&mut self, id: NodeId) -> Result<u32, Fail> {
        if let Some((_, p)) = self.row(id) {
            return Ok(p);
        }
        self.ctx.b.set_link(id, LinkInfo::UNRESOLVED)
    }

    /// The value of node `id`'s first attribute with `key`.
    pub fn attribute(&self, id: NodeId, key: AttributeKey) -> Option<&str> {
        let snap = &*self.ctx.b.snap;
        let range = snap.nodes[id].attributes.range();
        let a = snap.attributes.get(range)?.iter().find(|a| a.key == key)?;
        Some(snap.pool_str(a.value))
    }

    /// The value of node `id`'s Custom attribute `name`.
    pub fn custom_attribute(&self, id: NodeId, name: &str) -> Option<&str> {
        let snap = &*self.ctx.b.snap;
        let range = snap.nodes[id].attributes.range();
        let a = snap
            .attributes
            .get(range)?
            .iter()
            .find(|a| a.key == AttributeKey::Custom && snap.pool_str(a.name) == name)?;
        Some(snap.pool_str(a.value))
    }

    /// An element's `:name:` option, which makes it an explicit target. Directive
    /// nodes use Name for the directive name and keep `:name:` as Custom "name".
    pub fn element_name(&self, id: NodeId) -> Option<&str> {
        if self.kind_of(id) == NodeKind::Directive {
            self.custom_attribute(id, "name")
        } else {
            self.attribute(id, AttributeKey::Name)
        }
    }

    /// Finds what a normalized name refers to; both ambiguities read `Ambiguous`.
    pub fn lookup(&self, name: &str) -> Option<Name> {
        if name.is_empty() {
            return None;
        }
        match self.names.get(name)? {
            Name::AmbiguousExplicit => Some(Name::Ambiguous),
            other => Some(*other),
        }
    }

    pub fn mark_referenced(&mut self, id: NodeId) {
        let kind = self.kind_of(id);
        if matches!(
            kind,
            NodeKind::Target | NodeKind::Footnote | NodeKind::Citation | NodeKind::InlineTarget
        ) {
            self.ctx.b.add_flags(id, NodeFlags::REFERENCED);
        }
    }

    pub fn node_span(&self, id: NodeId) -> SourceSpan {
        self.ctx.b.snap.nodes[id].span
    }

    /// The smallest meaningful part of a node for a message to quote: a section's
    /// title, and otherwise the node's first line.
    pub fn quote_span(&self, id: NodeId) -> SourceSpan {
        let nodes = &self.ctx.b.snap.nodes;
        let mut target = id;
        let next = id + 1;
        if nodes[id].kind == NodeKind::Section
            && next.index() < nodes.len()
            && nodes[next].kind == NodeKind::Heading
        {
            target = next;
        }
        let mut sp = nodes[target].span;
        let text = self.ctx.source_of(sp.source);
        let end = (sp.end as usize).min(text.len());
        if let Some(k) = text
            .as_bytes()
            .get(sp.start as usize..end)
            .and_then(|s| s.iter().position(|&c| c == b'\n' || c == b'\r'))
        {
            sp.end = sp.start + k as u32;
        }
        sp
    }

    /// Reports `m` about node `id`'s span (quoted).
    pub fn report_at(&mut self, m: Message, span: SourceSpan, b: Arg) {
        self.ctx.report(m, span, arg_span(span), b);
    }
}
