//! The pdf builder (builders.odin's write_pdf and write_book): one Typst book per
//! `pdf_documents` entry, one output unit each (`book:<file>`).

use std::collections::HashSet;
use std::path::Path;

use gd_conf::PdfDocument;
use gd_env::{BuilderKind, Digest};
use gd_host::{Ledger, Problem};
use gd_link::Worker;
use gd_outputs::{Generation, UnitRecord};
use gd_report::Report;
use gd_resolve::{BuildView, OutputBuilder};
use gd_typst::Backend;
use guidedoc::Labels;

use crate::compose::{Book, Composer, language_file};
use crate::failure::{BookSource, compile_failure};
use crate::fonts::bundled_font_dir;
use crate::inputs::{TypstSettings, book_inputs};
use crate::pages::pages_failed;
use crate::typst_root::{TypstRoot, typst_folders};

/// The `pdf` builder. Each book is `<file>-<language>.pdf` in the output folder, with
/// its generated Typst in `sources/` (and as `book-<language>.typ` when there is one
/// book), to inspect when Typst reports a problem. `labels` are the translated renderer
/// labels (gd-intl's); `None` writes them in English.
#[derive(Debug, Clone, Copy, Default)]
pub struct PdfBuilder<'a> {
    /// The renderer labels in the build's language.
    pub labels: Option<&'a Labels>,
}

impl OutputBuilder for PdfBuilder<'_> {
    fn kind(&self) -> BuilderKind {
        BuilderKind::Pdf
    }

    /// Typesets every book. Fails with `build.pdf.duplicate` when two books have one
    /// file name, `build.pdf.root` for a missing root document, `build.output` when a
    /// chapter could not be made, `typst.missing` without a Typst compiler, and
    /// `typst.compile` (Typst's diagnostics in `reports`) when Typst failed.
    fn write(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let c = view.config;
        let mut outputs = HashSet::new();
        for book in &c.pdf.documents {
            if !outputs.insert(language_file(&book.file, &c.language)) {
                return Err(Problem::new(
                    "build.pdf.duplicate",
                    "DUPLICATE PDF OUTPUT",
                    &book.file,
                    "Two books resolve to the same PDF filename after adding the language.",
                    "Give each pdf_documents entry a distinct output filename.",
                    2,
                ));
            }
        }
        for book in &c.pdf.documents {
            self.write_book(view, generation, book, reports)?;
        }
        Ok(())
    }
}

impl PdfBuilder<'_> {
    /// Composes one book and, when anything it is made from changed, typesets it.
    fn write_book(
        &self,
        view: &BuildView<'_>,
        generation: &mut Generation,
        book: &PdfDocument,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let Some(root) = view.doc_named(&book.root) else {
            return Err(Problem::new(
                "build.pdf.root",
                "BOOK ROOT NOT FOUND",
                &book.root,
                "pdf_documents names a root document that does not exist.",
                "Set root in pdf_documents to an existing document.",
                2,
            ));
        };
        // The book's text and the PDF Typst hands back are held of the build's budget
        // until the book is written.
        let ledger = Ledger::new(view.budget);
        let composer = Composer { view, labels: self.labels, ledger: &ledger };
        let composed = composer.compose(book, root, reports);
        let key = format!("book:{}", book.file);
        if gd_host::was_interrupted() {
            return Err(gd_host::interrupted_problem());
        }
        if composed.failed > 0 {
            generation.unit(&key).fail();
            return pages_failed(composed.failed, &generation.paths().out_dir);
        }
        let c = view.config;
        let Some(backend) = gd_typst::locate(&c.pdf.typst) else {
            return Err(backend_problem(&c.pdf.typst));
        };
        let settings = TypstSettings::of(c, bundled_font_dir(c));
        let inputs = book_inputs(view, &composed, &settings, &backend);
        if let Some(kept) = generation.reuse(&key, inputs, |_| Digest::default()) {
            reports.extend(kept.reports.iter().cloned());
            return Ok(());
        }
        let run = TypstRun { view, labels: self.labels, book: &composed, settings, backend };
        run.typeset(generation, &key, inputs, &ledger, reports)
    }
}

/// One run of Typst over a composed book.
struct TypstRun<'a, 'b> {
    view: &'a BuildView<'b>,
    labels: Option<&'a Labels>,
    book: &'a Book,
    settings: TypstSettings,
    backend: Backend,
}

impl TypstRun<'_, '_> {
    /// Typesets the book into its unit: the PDF (staged, so a failure leaves the
    /// previous one), and its generated Typst beside it. Typst's warnings are reported
    /// and kept with the unit.
    fn typeset(
        &self,
        generation: &mut Generation,
        key: &str,
        inputs: Digest,
        ledger: &Ledger<'_>,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let c = self.view.config;
        let file = language_file(key.trim_start_matches("book:"), &c.language);
        let source_name = format!("{file}.typ");
        let single = c.pdf.documents.len() == 1;
        let failed_name =
            if single { language_file("book.typ", &c.language) } else { source_name.clone() };
        let failed_copy = gd_env::doctrees_dir(c).join("failed").join(&failed_name);
        let out = generation.paths().out_dir.join(&file);
        let mut unit = generation.unit(key);
        let stage = unit.stage_path(&file)?;
        let root = self.root()?;
        let reserve = |bytes: usize| ledger.charge(bytes);
        let read_roots = if self.backend.is_embedded() { typst_folders(c) } else { Vec::new() };
        let options = self.settings.options(read_roots, &reserve);
        let outcome =
            gd_typst::compile_text(&self.backend, &self.book.source, root.dir(), &stage, &options);
        if gd_host::was_interrupted() {
            return Err(gd_host::interrupted_problem());
        }
        let shown_as = if single {
            out.with_file_name(language_file("book.typ", &c.language))
        } else {
            out.with_file_name("sources").join(&source_name)
        };
        let compiled = match outcome {
            Ok(compiled) => compiled,
            Err(error) => {
                let source = self.book_source(&failed_copy, root.dir());
                let problem = compile_failure(&source, error, &out, &failed_copy, reports);
                return Err(problem);
            }
        };
        let source = self.book_source(&shown_as, root.dir());
        let mut worker = Worker::new();
        let warnings: Vec<Report> =
            compiled.diagnostics.iter().map(|d| source.report(&mut worker, d)).collect();
        reports.extend(warnings.iter().cloned());
        // The book built, so a book kept from a failed build is out of date.
        let failed = gd_env::doctrees_dir(c).join("failed");
        let _ = std::fs::remove_file(failed.join(&source_name));
        let _ = std::fs::remove_file(&failed_copy);
        if single {
            unit.emit_text(&language_file("book.typ", &c.language), &self.book.source)?;
        }
        unit.emit_text(&format!("sources/{source_name}"), &self.book.source)?;
        unit.finish(UnitRecord { inputs, reports: warnings, ..UnitRecord::default() });
        Ok(())
    }

    /// The folder Typst runs in: the book's root for the embedded compiler, which
    /// confines its reads to `read_roots`; a private folder of links for the program.
    fn root(&self) -> Result<TypstRoot, Problem> {
        if self.backend.is_embedded() {
            Ok(TypstRoot::shared(self.view.config))
        } else {
            TypstRoot::for_program(self.view.config)
        }
    }

    /// The book's source as Typst saw it, shown as `shown_as`, in `root`.
    fn book_source<'s>(&'s self, shown_as: &'s Path, root: &'s Path) -> BookSource<'s, 's> {
        BookSource {
            view: self.view,
            labels: self.labels,
            source: &self.book.source,
            chapters: &self.book.chapters,
            shown_as,
            root,
        }
    }
}

fn backend_problem(program: &str) -> Problem {
    Problem::new(
        "typst.missing",
        "TYPST WAS NOT FOUND",
        program,
        "PDF output uses the Typst compiler, which this build of guidedog does not embed and \
         which is not installed.",
        "Install Typst 0.15.1, or set typst in conf.toml to the program's path.",
        4,
    )
}
