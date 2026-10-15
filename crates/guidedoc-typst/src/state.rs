//! The state of one render: the document, the output, the anchor table, per-node marks,
//! and the modes nested content inherits (page furniture, heading content,
//! preformatted text).

use guidedoc::{
    Anchors, Arg, AttributeKey, Diagnostic, Document, Fail, Message, NodeId, NodeKind, ROOT,
    Region, RenderContext, RenderOptions, Severity, SourceSpan, StackGuard, arg_int,
    build_anchors, diagnostic, kept_message,
};

use crate::messages::DEPTH_LIMIT;

/// Per-node rendering state, so footnotes render once and labels are unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark {
    /// Footnote: some reference outside page furniture points at it.
    Referenced = 1,
    /// Footnote: its body has been rendered.
    Done = 2,
    /// The node's label has been emitted.
    Labelled = 4,
    /// SubstitutionDefinition: being expanded (the recursion guard).
    Expanding = 8,
    /// Inside a heading, whose content Typst repeats in the outline.
    InHeading = 16,
}

/// What the body needs to know before it starts: the title and the page furniture.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Scan {
    /// The document's Title node.
    pub title: Option<NodeId>,
    /// A `title` Directive.
    pub title_directive: Option<NodeId>,
    /// The first Heading, the title of last resort.
    pub heading: Option<NodeId>,
    pub header: Option<NodeId>,
    pub footer: Option<NodeId>,
    pub sectnum: Option<NodeId>,
}

/// One render. It borrows the render context for the call.
pub(crate) struct State<'r, 'a> {
    pub ctx: &'r mut RenderContext<'a>,
    pub doc: Document<'a>,
    pub options: &'a RenderOptions<'a>,
    pub anchors: Anchors,
    marks: Vec<u8>,
    depth: usize,
    max_depth: usize,
    /// Bounds the machine stack of the recursion (`enter`).
    stack: StackGuard,
    /// Page header or footer: no labels and no footnotes.
    pub quiet: bool,
    /// Heading content, repeated by the outline: no labels.
    pub copied: bool,
    /// Preformatted: keep spaces and line breaks.
    pub pre: bool,
    /// The preformatted column, for tab expansion.
    pub column: usize,
    /// The first error reported; it decides the render's result.
    failure: Option<Diagnostic>,
    /// Reused for translated math.
    pub math: String,
    /// Reused for anchor keys.
    pub key: String,
    /// Every Footnote, Citation, and SubstitutionDefinition, in document order.
    pub notes: Vec<NodeId>,
}

impl<'r, 'a> State<'r, 'a> {
    /// Starts a render: builds the anchor table and the per-node marks.
    pub fn new(ctx: &'r mut RenderContext<'a>, stack: StackGuard) -> Result<Self, Fail> {
        let doc = ctx.doc;
        let options = ctx.options;
        let anchors = build_anchors(doc, options, true);
        let mut marks = Vec::new();
        if marks.try_reserve_exact(doc.node_count()).is_err() {
            return Err(Fail::capacity(Region::Scratch, doc.node_count() as u64, true));
        }
        marks.resize(doc.node_count(), 0);
        let notes = (0..doc.node_count())
            .map(NodeId::from_index)
            .filter(|&i| {
                matches!(
                    doc.kind(i),
                    NodeKind::Footnote | NodeKind::Citation | NodeKind::SubstitutionDefinition
                )
            })
            .collect();
        let max_depth = ctx.limits.max_depth.max(32) + 64;
        Ok(State {
            ctx,
            doc,
            options,
            anchors,
            marks,
            depth: 0,
            max_depth,
            stack,
            quiet: false,
            copied: false,
            pre: false,
            column: 0,
            failure: None,
            math: String::new(),
            key: String::new(),
            notes,
        })
    }

    /// Ends the render: the first error fails it.
    pub fn finish(self) -> Result<(), Fail> {
        match self.failure {
            Some(d) => Err(Fail::from_diagnostic(d)),
            None => Ok(()),
        }
    }

    /// Records a diagnostic; under `keep_documents_with_errors` content errors become
    /// warnings. An error decides the render's result.
    pub fn report(&mut self, d: Diagnostic) {
        if d.severity == Severity::Error && self.failure.is_none() {
            self.failure = Some(d.clone());
        }
        self.ctx.reports().report(d);
    }

    /// Reports message `m` at `span` with one argument, as the policy keeps it.
    pub fn fail(&mut self, m: Message, span: SourceSpan, a: Arg) {
        let m = kept_message(&self.ctx.policy, m);
        self.report(diagnostic(m, span, a, Arg::None));
    }

    /// Fails the render for a node its handler does not write: a renderer bug.
    pub fn unhandled(&mut self, id: NodeId) {
        let kind = guidedoc::kind_name(self.doc.kind(id));
        let span = self.doc.node(id).span;
        self.fail(guidedoc::CORE_UNHANDLED_KIND, span, guidedoc::arg_static(kind));
    }

    /// Guards recursion; every nested block, inline, and expansion passes through it.
    /// Past the depth or stack bound it reports once and returns false.
    pub fn enter(&mut self, id: NodeId) -> bool {
        let span = self.doc.node(id).span;
        if self.depth >= self.max_depth {
            if self.failure.is_none() {
                let depth = i64::try_from(self.max_depth).unwrap_or(i64::MAX);
                self.fail(DEPTH_LIMIT, span, arg_int(depth));
            }
            return false;
        }
        if self.stack.is_exhausted() {
            if self.failure.is_none() {
                self.report(self.stack.diagnostic(span));
            }
            return false;
        }
        self.depth += 1;
        true
    }

    /// Leaves a level `enter` entered.
    pub fn leave(&mut self) {
        self.depth -= 1;
    }

    pub fn has_mark(&self, id: NodeId, m: Mark) -> bool {
        self.marks.get(id.index()).is_some_and(|&b| b & m as u8 != 0)
    }

    pub fn set_mark(&mut self, id: NodeId, m: Mark) {
        if let Some(b) = self.marks.get_mut(id.index()) {
            *b |= m as u8;
        }
    }

    pub fn clear_mark(&mut self, id: NodeId, m: Mark) {
        if let Some(b) = self.marks.get_mut(id.index()) {
            *b &= !(m as u8);
        }
    }

    // Output.

    pub fn emit(&mut self, s: &str) {
        self.ctx.out.emit(s);
    }

    pub fn byte(&mut self, c: u8) {
        self.ctx.out.emit_byte(c);
    }

    pub fn int(&mut self, v: i64) {
        self.ctx.out.emit_int(v);
    }

    /// Records that output from here on comes from node `id` (the source map).
    pub fn map_node(&mut self, id: NodeId) {
        let span = self.doc.node(id).span;
        self.ctx.out.emit_mark(span);
    }

    /// Returns the Custom attribute `name` of `id`, or "".
    pub fn custom(&self, id: NodeId, name: &str) -> &'a str {
        self.doc.custom_attribute(id, name).unwrap_or("")
    }

    /// Returns the Name attribute of `id`, or "".
    pub fn name_of(&self, id: NodeId) -> &'a str {
        self.doc.attribute(id, AttributeKey::Name).unwrap_or("")
    }

    /// Finds the title and page furniture and marks referenced footnotes.
    pub fn scan_document(&mut self) -> Scan {
        let doc = self.doc;
        let mut scan = Scan {
            title: doc.children(ROOT).find(|&c| doc.kind(c) == NodeKind::Title),
            ..Scan::default()
        };
        let mut furniture_end = 0u32; // references in headers and footers do not count.
        for i in 0..doc.node_count() {
            let id = NodeId::from_index(i);
            let kind = doc.kind(id);
            if kind == NodeKind::Heading {
                scan.heading.get_or_insert(id);
                for inner in id.0 + 1..doc.subtree_end(id).0 {
                    self.set_mark(NodeId(inner), Mark::InHeading);
                }
            } else if kind == NodeKind::Directive {
                let name = self.name_of(id);
                let slot = match name {
                    "title" => &mut scan.title_directive,
                    "sectnum" | "section-numbering" => &mut scan.sectnum,
                    "header" => &mut scan.header,
                    "footer" => &mut scan.footer,
                    _ => continue,
                };
                slot.get_or_insert(id);
                if matches!(name, "header" | "footer") {
                    furniture_end = furniture_end.max(doc.subtree_end(id).0);
                }
            } else if kind == NodeKind::FootnoteReference
                && id.0 >= furniture_end
                && let Some(note) = self.find_footnote(id)
            {
                self.set_mark(note, Mark::Referenced);
            }
        }
        scan
    }
}
