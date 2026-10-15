//! The text and dummy builders (builders.odin's write_text and write_dummy, over
//! html_builder.odin's write_documents): one output unit per document, kept when its
//! inputs and the answers to its lookups are unchanged.

use gd_env::{BuilderKind, Digest, DocId, OutputFormat};
use gd_host::Problem;
use gd_outputs::{Generation, UnitRecord};
use gd_report::Report;
use gd_resolve::{BuildView, OutputBuilder, document_settings, lookup_answer, page_inputs};
use guidedoc::Labels;

use crate::pages::{Job, PageStyle, answers, render_pages};

/// The `text` builder: one `<docname>.txt` per document, rendered by the registry's
/// `text` renderer. `labels` are the translated renderer labels (gd-intl's); `None`
/// writes them in English.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextBuilder<'a> {
    /// The renderer labels in the build's language.
    pub labels: Option<&'a Labels>,
}

/// The `dummy` builder: links every document without writing anything, as Sphinx's
/// dummy builder resolves every document, so its references are checked and reported
/// as for any output.
#[derive(Debug, Clone, Copy, Default)]
pub struct DummyBuilder;

impl OutputBuilder for TextBuilder<'_> {
    fn kind(&self) -> BuilderKind {
        BuilderKind::Text
    }

    /// Fails with `build.builder` when the registry has no text renderer, and with
    /// `build.output` when a page could not be made.
    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        if view.registry.find_renderer("text").is_none() {
            return Err(Problem::new(
                "build.builder",
                "TEXT BUILDER UNAVAILABLE",
                "text",
                "This build of guidedog has no text renderer.",
                "Use the html or pdf builder.",
                2,
            ));
        }
        let style =
            PageStyle { format: OutputFormat::Text, renderer: "text", labels: self.labels };
        write_documents(view, generation, reports, style, Some(".txt"))
    }
}

impl OutputBuilder for DummyBuilder {
    fn kind(&self) -> BuilderKind {
        BuilderKind::Dummy
    }

    /// Fails with `build.output` when a page could not be linked.
    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let style = PageStyle { format: OutputFormat::Text, renderer: "", labels: None };
        write_documents(view, generation, reports, style, None)
    }
}

/// Renders every document that was read and is not kept from the previous output, on
/// worker threads, then writes each as `<docname><suffix>` (nothing with no suffix)
/// in docname order, one unit each.
fn write_documents(
    view: &BuildView<'_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
    style: PageStyle<'_>,
    suffix: Option<&str>,
) -> Result<(), Problem> {
    let common = document_settings(view);
    let mut pending: Vec<(DocId, Digest)> = Vec::new();
    for (id, doc) in view.env.iter().filter(|(_, d)| !d.failed) {
        let inputs = page_inputs(view, id, &common);
        let answer = |lookup: &_| lookup_answer(view, &doc.docname, lookup);
        match generation.reuse(&doc.docname, inputs, answer) {
            Some(kept) => reports.extend(kept.reports.iter().cloned()),
            None => pending.push((id, inputs)),
        }
    }
    let jobs: Vec<Job> = pending.iter().map(|&(doc, _)| Job { doc, depth: 0 }).collect();
    let pages = render_pages(view, &jobs, style);
    if gd_host::was_interrupted() {
        return Err(gd_host::interrupted_problem());
    }
    for ((id, inputs), page) in pending.into_iter().zip(pages) {
        let docname = &view.doc(id).docname;
        reports.extend(page.reports.iter().cloned());
        let mut unit = generation.unit(docname);
        if page.failed {
            unit.fail();
            continue;
        }
        if let Some(suffix) = suffix {
            unit.emit_text(&format!("{docname}{suffix}"), &page.body)?;
        }
        unit.finish(UnitRecord {
            inputs,
            assets: page.assets,
            lookups: answers(view, docname, page.trace),
            reports: page.reports,
        });
    }
    generation.check_written()
}
