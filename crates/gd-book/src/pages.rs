//! What the builders share: the digests of what pages are made from, rendering pages on
//! worker threads, and the problem of pages that could not be made.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use gd_env::{Digest, Digester, DocId, Lookup, OutputFormat, Rendered, Trace};
use gd_host::Problem;
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

/// What every document's output shares: the output settings, the translations and
/// reading settings (the environment's digest), the inventories, every document's
/// docname and title, and in toctree order each document's name, anchor key, parent,
/// toctree count and sections (title, anchor, number).
pub(crate) fn document_settings(view: &BuildView<'_>) -> Digest {
    let mut d = Digester::new();
    d.digest(&view.settings).digest(&view.env.settings).digest(&view.intersphinx.digest);
    for doc in &view.env.docs {
        d.text(&doc.docname).text(&doc.title);
    }
    for &id in &view.tables.order {
        let doc = view.doc(id);
        let parent = view.tables.parent.get(id.index()).copied().flatten();
        d.text(&doc.docname).text(view.doc_key(id));
        d.number(parent.map_or(-1, |p| i64::from(p.0))).number(doc.toctrees.len() as i64);
        let numbers = view.tables.secnumbers.get(id.index());
        for (i, section) in doc.sections.iter().enumerate() {
            let number = numbers.and_then(|n| n.get(i)).map_or("", String::as_str);
            d.text(&section.title).text(&section.anchor).text(number);
        }
    }
    d.finish()
}

/// A document page's own inputs on top of `common`: its source and object digests, its
/// figure numbers, what it included, and for a todolist every document with todos.
/// What its references find is in its lookups.
pub(crate) fn page_inputs(view: &BuildView<'_>, id: DocId, common: &Digest) -> Digest {
    let doc = view.doc(id);
    let mut d = Digester::new();
    d.digest(common).digest(&doc.digest);
    d.digest(&doc.object.as_ref().map_or(Digest::default(), |o| o.digest));
    if let Some(numbers) = view.tables.fignumbers.get(id.index()) {
        d.texts(numbers);
    }
    for dep in &doc.deps {
        d.digest(&dep.digest);
    }
    if doc.todolist {
        for other in view.env.docs.iter().filter(|o| !o.todos.is_empty()) {
            d.digest(&other.digest);
        }
    }
    d.finish()
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

/// Fails the build when `failed` outputs could not be made; the reports above say
/// why, and the previous output stays in place.
pub(crate) fn pages_failed(failed: usize, out_dir: &Path) -> Result<(), Problem> {
    if failed == 0 {
        return Ok(());
    }
    Err(Problem::new(
        "build.output",
        "PAGES NOT WRITTEN",
        &out_dir.display().to_string(),
        format!(
            "{failed} document(s) could not be written, so this build's output was not \
             published. The previous output is unchanged."
        ),
        "Fix the errors reported above for each page, then run the build again.",
        1,
    ))
}
