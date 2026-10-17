//! Excerpts use the normal link pass in their source document's context.

use gd_env::{DocId, Lookup, Rendered, Trace};
use gd_resolve::{BuildView, Resolved};
use guidedoc::{
    Anchors, AttributeKey, Builder, Document, Fail, LinkInfo, LinkKind, NodeId, NodeKind,
    PassOutcome, ROOT, build_anchors,
};

use crate::page::Page;
use crate::render::{PageRequest, Worker, render_options, run_page};
use crate::rewrite::{Action, Rewriter, Rule, rewrite_document};

/// A listing's output file and an occurrence-specific anchor namespace.
#[derive(Debug, Clone, Copy)]
pub struct Excerpt<'a> {
    /// The listing's output path, relative to the output root.
    pub destination: &'a str,
    /// A unique prefix for this occurrence of the excerpt on the listing.
    pub prefix: &'a str,
}

/// Links and renders an explicit excerpt for a listing. Failures and dependencies
/// are returned like a page's; lookups retain the source article's context.
/// Callers provide the plain summary fallback when the document has no boundary.
pub fn render_excerpt(
    view: &BuildView<'_>,
    worker: &mut Worker,
    request: &PageRequest<'_>,
    excerpt: &Excerpt<'_>,
) -> Rendered {
    run_page(view, worker, request, false, Some(excerpt))
}

pub(crate) fn source_trace(source: &str, trace: Trace) -> Trace {
    let mut scoped = Trace::new();
    for lookup in trace.into_lookups() {
        scoped.note(Lookup::excerpt(source, &lookup));
    }
    scoped
}

pub(crate) fn link(page: &mut Page<'_>, doc: Document<'_>, b: &mut Builder<'_>) -> PassOutcome {
    let end = (0..doc.node_count())
        .map(NodeId::from_index)
        .find(|&id| {
            doc.kind(id) == NodeKind::Directive
                && doc.attribute(id, AttributeKey::Name) == Some("more")
        })
        .unwrap_or(ROOT);
    let mut options = render_options(&page.view, page.id, page.format);
    // link_destination adds the document key for books and singlehtml.
    options.anchor_prefix = "";
    let anchors = build_anchors(doc, &options, true);
    let root = gd_env::excerpt_root(doc);
    rewrite_document(doc, b, &mut Introduction { page, end, root, anchors })
}

struct Introduction<'a, 'p> {
    page: &'a mut Page<'p>,
    end: NodeId,
    root: NodeId,
    anchors: Anchors,
}

impl Introduction<'_, '_> {
    fn excluded(&self, doc: Document<'_>, id: NodeId) -> bool {
        id >= self.end
            || id == ROOT
            || id == self.root
            || matches!(doc.kind(id), NodeKind::Title | NodeKind::Subtitle | NodeKind::Heading)
                && doc.parent_of(id).is_some_and(|p| p == ROOT || p == self.root)
    }
}

impl Rule for Introduction<'_, '_> {
    fn visits(&self, _: Document<'_>, _: NodeId) -> bool {
        true
    }

    fn decide(&mut self, doc: Document<'_>, id: NodeId) -> Action {
        if id >= self.end {
            return Action::Drop;
        }
        let root = self.root;
        if id == root && root != ROOT {
            return Action::Unwrap;
        }
        if doc.kind(id) == NodeKind::Heading && doc.parent_of(id) == Some(root) {
            return Action::Drop;
        }
        if matches!(doc.kind(id), NodeKind::Title | NodeKind::Subtitle | NodeKind::FieldList)
            && doc.parent_of(id) == Some(ROOT)
        {
            return Action::Drop;
        }
        if doc.kind(id) == NodeKind::Toctree
            || doc.kind(id) == NodeKind::Directive
                && matches!(doc.attribute(id, AttributeKey::Name), Some("postlist" | "todolist"))
        {
            return Action::Drop;
        }
        if doc
            .link_info(id)
            .is_some_and(|l| l.kind == LinkKind::Internal && self.excluded(doc, l.resolved))
        {
            return Action::Replace;
        }
        self.page.decide(doc, id)
    }

    fn replace(&mut self, rw: &mut Rewriter<'_, '_, '_>, id: NodeId) -> Result<(), Fail> {
        let link = rw.doc.link_info(id);
        if let Some(link) = link.filter(|l| l.kind == LinkKind::Internal) {
            let anchor = self.anchors.anchor_of(link.resolved).unwrap_or("");
            let target = Resolved {
                doc: Some(self.page.id),
                anchor: anchor.to_owned(),
                ..Default::default()
            };
            let (url, kind) = crate::xref::link_destination(self.page, &target);
            let copy = rw.open(rw.doc.kind(id), rw.doc.node(id).span)?;
            rw.b.set_value(copy, rw.doc.node(id).value);
            rw.b.add_flags(copy, rw.doc.node(id).flags);
            let text = rw.b.add_text(rw.doc.node_text(id))?;
            rw.b.set_text(copy, text);
            rw.copy_attributes(id, copy, |_, _| true)?;
            let destination = rw.b.add_text(&url)?;
            rw.b.set_link(copy, LinkInfo { destination, kind, ..LinkInfo::default() })?;
            rw.rewrite_children(self, id)?;
            rw.close(copy)
        } else {
            self.page.replace(rw, id)
        }
    }

    fn decorate(
        &mut self,
        rw: &mut Rewriter<'_, '_, '_>,
        source: NodeId,
        copy: NodeId,
    ) -> Result<(), Fail> {
        self.page.decorate(rw, source, copy)
    }
}

pub(crate) fn append(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    worker: &mut Worker,
    doc: DocId,
    occurrence: NodeId,
) -> Result<(), Fail> {
    let prefix = format!("excerpt-{}-{}-", occurrence.0, page.view.doc_key(doc));
    let destination = page.view.page_name(&page.info.docname);
    let excerpt = Excerpt { destination: &destination, prefix: &prefix };
    let mut options = render_options(&page.view, page.id, page.format);
    options.permalinks = false;
    let request = PageRequest { doc, format: page.format, options, ..Default::default() };
    let rendered = render_excerpt(&page.view, worker, &request, &excerpt);
    page.assets.extend(rendered.assets);
    page.reports.extend(rendered.reports);
    for lookup in rendered.trace.into_lookups() {
        page.trace.note(lookup);
    }
    if rendered.failed {
        return Err(Fail::new(guidedoc::Status::InvalidInput));
    }
    if let Some(doc) = worker.ws.document() {
        let span = rw.doc.node(occurrence).span;
        rw.copy_foreign_document(doc, span)?;
    }
    Ok(())
}
