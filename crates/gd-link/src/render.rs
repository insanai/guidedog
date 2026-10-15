//! Rendering a linked page (render_page.odin): a document's cached object is loaded,
//! linked for an output by the link pass, and rendered with a renderer of the
//! registry, on a worker that owns its workspaces.

use std::cell::{OnceCell, RefCell};

use gd_env::{BuilderKind, DocId, OutputFormat, Rendered};
use gd_graphdog::Context;
use gd_host::{Ledger, Problem};
use gd_report::{Report, Severity};
use gd_resolve::BuildView;
use guidedoc::{
    Builder, Document, MathStyle, Pass, PassOutcome, RenderOptions, Request, Workspace,
};

use crate::page::Page;

/// What one rendering thread owns: the workspace pages are linked and rendered in, a
/// second one for the documents a todolist copies from, and its Graphviz context,
/// started on the first graph. Make one per worker and reuse it for every page it
/// renders; its buffers keep their capacity.
#[derive(Default)]
pub struct Worker {
    ws: Workspace,
    spare: Workspace,
    graphs: OnceCell<Option<Context>>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker").field("graphs", &self.graphs.get().is_some()).finish()
    }
}

impl Worker {
    /// A worker with empty workspaces and no Graphviz context yet.
    pub fn new() -> Worker {
        Worker::default()
    }
}

/// One page to render. Fill it with struct-update syntax.
#[derive(Debug, Clone, Copy, Default)]
pub struct PageRequest<'a> {
    /// The document.
    pub doc: DocId,
    /// The output it is linked for.
    pub format: OutputFormat,
    /// The renderer's id in the build's registry (`html`, `typst`, `text`); empty only
    /// links the page, to check its references.
    pub renderer: &'a str,
    /// The renderer's settings (see [`render_options`]).
    pub options: RenderOptions<'a>,
    /// PDF: the document's toctree depth below the book's root (0 for the root).
    pub depth: u32,
}

/// Loads the document's object, links it for the output, and renders it. Never fails:
/// a page that cannot be made comes back `failed`, with reports saying why; problems in
/// the document's content are only reported. The body is empty when `renderer` is.
pub fn render_page(
    view: &BuildView<'_>,
    worker: &mut Worker,
    request: &PageRequest<'_>,
) -> Rendered {
    run_page(view, worker, request, false)
}

/// Renders a page again with a source map, and reports where in its sources the
/// rendered byte at `offset` came from. A book uses it when Typst rejects a chapter, to
/// point at the line of reStructuredText or Markdown behind the problem. `None` when
/// the page cannot be rendered or nothing was mapped before `offset`.
pub fn source_location(
    view: &BuildView<'_>,
    worker: &mut Worker,
    request: &PageRequest<'_>,
    offset: usize,
) -> Option<Report> {
    let rendered = run_page(view, worker, request, true);
    if rendered.failed {
        return None;
    }
    let ws = &worker.ws;
    let entry = ws.source_map().iter().rev().find(|e| e.offset as usize <= offset)?;
    let doc = ws.document()?;
    let span = entry.span;
    let mut report = Report { severity: Severity::Error, ..Report::default() };
    if let Some(source) = doc.source_entry(span.source) {
        report.path = gd_host::display_path(&source.name);
    }
    let text = doc.source_str(span.source);
    gd_report::set_span(&mut report, text, span.start as usize, span.end as usize);
    Some(report)
}

/// The renderer settings a page uses for an output: language, permalinks, section
/// number suffix, MathJax's delimiters in HTML, and in the book and singlehtml the
/// document's key before every anchor. The caller adds the highlighter and the
/// translated labels.
pub fn render_options<'b>(
    view: &BuildView<'b>,
    doc: DocId,
    format: OutputFormat,
) -> RenderOptions<'b> {
    let config = view.config;
    let html = &config.html;
    let mut options = RenderOptions {
        standalone: false,
        language: &config.language,
        permalinks: html.permalinks,
        permalink_icon: &html.permalinks_icon,
        secnumber_suffix: &html.secnumber_suffix,
        ..RenderOptions::default()
    };
    if format == OutputFormat::Pdf || view.kind == BuilderKind::Singlehtml {
        options.anchor_prefix = view.tables.keys.get(doc.index()).map_or("", String::as_str);
    }
    if format == OutputFormat::Html {
        let pair = |v: &'b [String]| {
            [v.first().map_or("", String::as_str), v.get(1).map_or("", String::as_str)]
        };
        options.math = MathStyle::Sphinx; // for MathJax, as sphinx.ext.mathjax writes it.
        options.math_inline = pair(&config.mathjax.inline);
        options.math_display = pair(&config.mathjax.display);
    }
    options
}

/// Links and renders one page in the worker's workspace, which keeps the result (and
/// the source map, when asked) for the caller.
fn run_page(
    view: &BuildView<'_>,
    worker: &mut Worker,
    request: &PageRequest<'_>,
    source_map: bool,
) -> Rendered {
    let mut out = Rendered::default();
    let info = view.env.doc(request.doc);
    let renderer = match request.renderer {
        "" => None,
        id => match view.registry.find_renderer(id) {
            Some(r) => Some(r),
            None => return failed(out, no_renderer(id)),
        },
    };
    let Some(object) = &info.object else {
        return failed(out, no_object(&info.relative));
    };
    let Worker { ws, spare, graphs } = worker;
    if let Err(e) = view.objects.load_object(ws, object, view.limits) {
        return failed(out, e.problem);
    }
    let ledger = Ledger::new(view.budget);
    let page = RefCell::new(Page::new(
        *view,
        request.doc,
        request.format,
        request.depth,
        spare,
        graphs,
        &ledger,
    ));
    let run =
        |doc: Document<'_>, b: &mut Builder<'_>| -> PassOutcome { page.borrow_mut().link(doc, b) };
    let linked =
        guidedoc::transform_document(ws, &[Pass { name: "link", run: &run }], view.limits).is_ok();
    let page = page.into_inner();
    out.assets = page.assets;
    out.trace = page.trace;
    out.reports = page.reports;
    let rendered = match renderer {
        Some(renderer) if linked => {
            let render = Request {
                render: request.options,
                policy: *view.policy,
                limits: *view.limits,
                source_map,
                ..Request::default()
            };
            guidedoc::render_document(ws, renderer, &render).map(str::to_string).ok()
        }
        _ => linked.then(String::new),
    };
    let mut rendered_reports = gd_host::describe_all(ws);
    if rendered.is_some() {
        page_warnings(&mut rendered_reports, &info.reports);
    }
    out.reports.extend(rendered_reports);
    match rendered {
        Some(body) => out.body = body,
        None => out.failed = true,
    }
    out
}

/// What rendering a page that was made found, the way Sphinx reports it: each problem
/// once, and as a warning. A reference the reader already reported unresolved is left
/// out (the renderer meets the same link again), and the renderer's errors are
/// warnings, as Sphinx logs Docutils' errors, so `-W` fails the build.
fn page_warnings(reports: &mut Vec<Report>, read: &[Report]) {
    let unresolved = |r: &Report| r.code.ends_with(".reference.unresolved");
    let known = |r: &Report| {
        read.iter().any(|k| unresolved(k) && k.line == r.line && k.column == r.column)
    };
    reports.retain(|r| !(unresolved(r) && known(r)));
    for report in reports.iter_mut().filter(|r| r.severity == Severity::Error) {
        report.severity = Severity::Warning;
    }
}

/// A page that could not be made, with the problem that says why.
fn failed(mut out: Rendered, problem: Problem) -> Rendered {
    out.reports.push(problem.report());
    out.failed = true;
    out
}

fn no_renderer(id: &str) -> Problem {
    Problem::new(
        "build.renderer",
        "NO RENDERER",
        id,
        "This build has no renderer for the output.",
        "Load the project with a registry that has this renderer, such as \
         guidedoc_defaults::registry().",
        5,
    )
}

fn no_object(relative: &str) -> Problem {
    Problem::new(
        "build.object",
        "DOCUMENT WAS NOT READ",
        relative,
        "This document has no cached object to render: reading it failed.",
        "Fix the problems reported for the document, then build again.",
        5,
    )
}
