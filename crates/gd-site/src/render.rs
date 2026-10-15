//! Rendering document bodies on worker threads: each worker owns a gd-link `Worker`
//! (its workspaces and Graphviz context) and an HTML highlighter and takes the next
//! document; `gd_host::map_ordered` returns the results in docname order however the
//! threads interleave.

use gd_env::{DocId, OutputFormat, Rendered};
use gd_link::{PageRequest, Worker, render_options, render_page};
use gd_resolve::BuildView;
use guidedoc::Labels;
use guidedoc_defaults::HtmlHighlighter;

/// Renders the bodies of `docs` for HTML, on `-j` threads, in the order of `docs`. A
/// document is `None` when the build was interrupted before it was rendered.
pub(crate) fn render_bodies(
    view: &BuildView<'_>,
    labels: Option<&Labels>,
    docs: &[DocId],
) -> Vec<Option<Rendered>> {
    let threads = view.options.jobs.threads(docs.len());
    let init = || (Worker::new(), HtmlHighlighter::new());
    gd_host::map_ordered(threads, docs, init, |(worker, highlighter), &doc| {
        if gd_host::was_interrupted() {
            return None;
        }
        let mut options = render_options(view, doc, OutputFormat::Html);
        options.highlighter = Some(&*highlighter);
        options.labels = labels;
        let request =
            PageRequest { doc, format: OutputFormat::Html, renderer: "html", options, depth: 0 };
        Some(render_page(view, worker, &request))
    })
}
