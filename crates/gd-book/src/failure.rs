//! Typst's diagnostics, placed where they can be fixed: a problem in a chapter is shown at the
//! line of reStructuredText or Markdown it was generated from, one in the template or
//! preamble at that file or the generated book.

use std::path::{Path, PathBuf};

use gd_env::OutputFormat;
use gd_host::Problem;
use gd_link::{Worker, source_location};
use gd_report::Report;
use gd_resolve::BuildView;
use gd_typst::Diagnostic;
use guidedoc::Labels;
use guidedoc_defaults::TypstHighlighter;

use crate::compose::ChapterSpan;
use crate::pages::{Job, PageStyle, page_request};

/// A typeset book's source and where its pieces came from.
pub(crate) struct BookSource<'a, 'b> {
    pub view: &'a BuildView<'b>,
    pub labels: Option<&'a Labels>,
    /// The generated Typst.
    pub source: &'a str,
    /// Where each chapter lies in it.
    pub chapters: &'a [ChapterSpan],
    /// Where the generated book can be inspected (beside the PDF, or the failed copy).
    pub shown_as: &'a Path,
    /// The folder Typst's root-relative paths (`/_templates/book.typ`) start at.
    pub root: &'a Path,
}

impl BookSource<'_, '_> {
    /// The report of one Typst diagnostic, at its origin. Errors get Guidedog's code,
    /// title and hint for a book that was not built; warnings keep Typst's.
    pub fn report(&self, worker: &mut Worker, d: &Diagnostic) -> Report {
        let mut report = if d.stdin { self.in_book(worker, d) } else { self.in_file(d) };
        if d.severity == gd_typst::Severity::Error {
            report.code = "typst.compiler".into();
            report.title = "TYPST COULD NOT BUILD THE BOOK".into();
            report.hints.insert(
                0,
                format!(
                    "The location above is in your sources, or in the generated book kept \
                     at {} to inspect: fix the source or template it comes from, then \
                     build again.",
                    self.shown_as.display()
                ),
            );
        }
        report
    }

    /// A position in the generated text: in a chapter, at the document source it came
    /// from (rendering the chapter again with a source map); elsewhere at the book.
    fn in_book(&self, worker: &mut Worker, d: &Diagnostic) -> Report {
        let offset = gd_typst::offset_of(self.source, d.line, d.column.max(1));
        let chapter = self.chapters.iter().find(|c| offset >= c.start && offset < c.end);
        if let Some(ch) = chapter.filter(|_| d.line > 0)
            && let Some(place) = self.chapter_place(worker, ch, offset - ch.start)
        {
            let mut report = d.report(None);
            report.message = format!(
                "{}\n\n(Line {} of {}, which Guidedog generated from this source.)",
                report.message,
                d.line,
                self.shown_as.display()
            );
            report.path = place.path;
            (report.line, report.column) = (place.line, place.column);
            (report.end_line, report.end_column) = (place.end_line, place.end_column);
            report.excerpt = place.excerpt;
            return report;
        }
        let mut report = d.report(Some(self.source));
        report.path = gd_host::display_path(&self.shown_as.display().to_string());
        report
    }

    /// Where in its document a chapter's byte `offset` came from.
    fn chapter_place(
        &self,
        worker: &mut Worker,
        ch: &ChapterSpan,
        offset: usize,
    ) -> Option<Report> {
        let highlighter = TypstHighlighter::new();
        let style =
            PageStyle { format: OutputFormat::Pdf, renderer: "typst", labels: self.labels };
        let job = Job { doc: ch.doc, depth: ch.depth };
        let request = page_request(self.view, job, style, &highlighter);
        source_location(self.view, worker, &request, offset)
    }

    /// A position in a file Typst read (the project's template, a package): shown with
    /// an excerpt when the file is the project's.
    fn in_file(&self, d: &Diagnostic) -> Report {
        let Some(relative) = d.file.strip_prefix('/') else { return d.report(None) };
        let path: PathBuf = self.root.join(relative);
        let text = std::fs::read_to_string(&path).ok();
        let mut report = d.report(text.as_deref());
        report.path = gd_host::display_path(&path.display().to_string());
        report
    }
}

/// Explains a book Typst did not build, adding Typst's diagnostics (placed at their
/// origins) and the launch failure to `reports`. A failed build publishes nothing, so
/// the generated book is kept for inspection at `kept`, outside the output.
pub(crate) fn compile_failure(
    book: &BookSource<'_, '_>,
    error: gd_typst::Error,
    out: &Path,
    kept: &Path,
    reports: &mut Vec<Report>,
) -> Problem {
    let shown = out.display().to_string();
    if let gd_typst::Error::OutputTooLarge { bytes } = error {
        let what = "Keeping the compiled book";
        return gd_host::budget_problem(what, &shown, bytes, Some(book.view.budget));
    }
    let kept_problem = gd_host::publish(kept, book.source.as_bytes(), true).err();
    match &error {
        gd_typst::Error::Compile { diagnostics, .. } => {
            let mut worker = Worker::new();
            reports.extend(diagnostics.iter().map(|d| book.report(&mut worker, d)));
        }
        gd_typst::Error::Launch { message, .. } => {
            reports.push(launch_problem(&shown, message).report())
        }
        other => reports.push(other.report()),
    }
    if let Some(problem) = kept_problem {
        reports.push(problem.report());
    }
    Problem::new(
        "typst.compile",
        "THE PDF BOOK WAS NOT BUILT",
        &shown,
        "Typst reported the problems above. The previous PDF is unchanged.",
        "Fix the source or template named above and build again.",
        4,
    )
}

fn launch_problem(out: &str, message: &str) -> Problem {
    Problem::new(
        "typst.launch",
        "TYPST DID NOT FINISH",
        out,
        message,
        "Check that the typst program runs (set typst in conf.toml to its path), check its \
         access to the output folder and the machine's free memory, then run the build \
         again. Typst's own memory is outside --budget.",
        4,
    )
}
