//! Rendering document bodies on worker threads: each worker owns a gd-link `Worker`
//! (its workspaces and Graphviz context) and an HTML highlighter, takes the next
//! document, and leaves the result in that document's slot, so the caller lays the
//! pages out in docname order however the threads interleave.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    let slots: Vec<Mutex<Option<Rendered>>> = docs.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let threads = view.options.jobs.threads(docs.len());
    gd_host::run_workers(threads, |_| {
        let mut worker = Worker::new();
        let highlighter = HtmlHighlighter::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(&doc) = docs.get(i) else { break };
            if gd_host::was_interrupted() {
                break;
            }
            let mut options = render_options(view, doc, OutputFormat::Html);
            options.highlighter = Some(&highlighter);
            options.labels = labels;
            let request = PageRequest {
                doc,
                format: OutputFormat::Html,
                renderer: "html",
                options,
                depth: 0,
            };
            let rendered = render_page(view, &mut worker, &request);
            *slots[i].lock().unwrap_or_else(|p| p.into_inner()) = Some(rendered);
        }
    });
    slots.into_iter().map(|s| s.into_inner().unwrap_or_else(|p| p.into_inner())).collect()
}
