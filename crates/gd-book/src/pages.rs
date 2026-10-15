//! What the builders share: rendering pages on worker threads, and the answers to the
//! lookups each page made.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use gd_env::{Digest, DocId, Lookup, OutputFormat, Rendered, Trace};
use gd_link::{PageRequest, Worker, render_options, render_page};
use gd_resolve::{BuildView, lookup_answer};
use guidedoc::{Labels, RenderOptions};
use guidedoc_defaults::TypstHighlighter;

/// One page to render: a document, and in the book its depth below the book's root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Job {
    pub doc: DocId,
    pub depth: u32,
}

/// How a builder's pages are rendered.
#[derive(Clone, Copy)]
pub(crate) struct PageStyle<'a> {
    /// The format the link pass rewrites for.
    pub format: OutputFormat,
    /// The renderer's id; empty only links.
    pub renderer: &'a str,
    /// The translated labels renderers write ("Note", "Contents").
    pub labels: Option<&'a Labels>,
}

/// The request for one page, with the renderer settings its output uses: the text
/// output takes only the labels, as Odin's text builder did; the book adds the
/// highlighter that draws code in Guidedog's colours.
pub(crate) fn page_request<'a>(
    view: &BuildView<'a>,
    job: Job,
    style: PageStyle<'a>,
    highlighter: &'a TypstHighlighter,
) -> PageRequest<'a> {
    let options = if style.format == OutputFormat::Text {
        RenderOptions { labels: style.labels, ..RenderOptions::default() }
    } else {
        let mut options = render_options(view, job.doc, style.format);
        options.labels = style.labels;
        if style.format == OutputFormat::Pdf {
            options.highlighter = Some(highlighter);
        }
        options
    };
    PageRequest {
        doc: job.doc,
        format: style.format,
        renderer: style.renderer,
        options,
        depth: job.depth,
    }
}

/// Renders every job on `-j` worker threads, each with its own workspaces and
/// highlighter, and returns the pages in the jobs' order. A job left when the build is
/// interrupted comes back failed; the caller checks `gd_host::was_interrupted`.
pub(crate) fn render_pages(
    view: &BuildView<'_>,
    jobs: &[Job],
    style: PageStyle<'_>,
) -> Vec<Rendered> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Rendered)>> = Mutex::new(Vec::with_capacity(jobs.len()));
    let threads = view.options.jobs.threads(jobs.len());
    gd_host::run_workers(threads, |_| {
        let mut worker = Worker::new();
        let highlighter = TypstHighlighter::new();
        let mut mine = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(&job) = jobs.get(i) else { break };
            if gd_host::was_interrupted() {
                break;
            }
            let request = page_request(view, job, style, &highlighter);
            mine.push((i, render_page(view, &mut worker, &request)));
        }
        done.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend(mine);
    });
    let done = done.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut pages: Vec<Rendered> =
        (0..jobs.len()).map(|_| Rendered { failed: true, ..Rendered::default() }).collect();
    for (i, page) in done {
        pages[i] = page;
    }
    pages
}

/// Each lookup a page made, with the digest of its answer now, for the manifest.
pub(crate) fn answers(view: &BuildView<'_>, docname: &str, trace: Trace) -> Vec<(Lookup, Digest)> {
    trace
        .into_lookups()
        .into_iter()
        .map(|lookup| {
            let answer = lookup_answer(view, docname, &lookup);
            (lookup, answer)
        })
        .collect()
}
