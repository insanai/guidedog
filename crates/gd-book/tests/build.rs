//! End-to-end builds of projects read by the Sphinx-aware rst reader and saved to an
//! object store. Their environment and tables follow gd-read and gd-resolve, so the
//! builders are tested alone. PDF builds run the embedded Typst compiler and check
//! the laid-out pages and text returned by the builder.

#![allow(clippy::unwrap_used)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gd_book::{DummyBuilder, PdfBuilder, TextBuilder, TypesetBook};
use gd_conf::{Config, Digest, PdfDocument};
use gd_env::{
    BuildOptions, BuilderKind, DocId, DocInfo, EnglishUi, Environment, ObjectStore, PluginRun,
    SourceKind,
};
use gd_host::Problem;
use gd_outputs::{CommitRecords, Generation, GenerationOptions};
use gd_report::Report;
use gd_resolve::{BuildView, Dates, Intersphinx, OutputBuilder, Tables, chapter_key};
use guidedoc::{ContentPolicy, Limits, Policy, Registry, Request, Workspace};

/// A temporary folder removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-book-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir.canonicalize().unwrap())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project whose documents form one toctree below `index`, in the order given.
struct Project {
    _dir: TempDir,
    config: Config,
    options: BuildOptions,
    env: Environment,
    tables: Tables,
    intersphinx: Intersphinx,
    objects: ObjectStore,
    registry: Registry,
    policy: Policy,
    limits: Limits,
    dates: Dates,
    plugins: PluginRun,
    budget: gd_host::Budget,
    roots: Vec<PathBuf>,
}

impl Project {
    fn new(name: &str, docs: &[(&str, &str)]) -> Project {
        let dir = TempDir::new(name);
        let source = dir.0.join("docs");
        std::fs::create_dir_all(&source).unwrap();
        let mut config = Config::new(source.clone(), source.join("_build"));
        config.project = "Demo".into();
        config.pdf.documents = vec![PdfDocument {
            root: "index".into(),
            file: "demo.pdf".into(),
            title: "Demo Book".into(),
            author: "Ada".into(),
        }];
        config.pdf.fonts = "embedded".into();
        config.pdf.packages = "offline".into();
        let mut project = Project {
            roots: gd_host::canonical_roots(&gd_conf::readable_roots(&config)),
            objects: ObjectStore::new(gd_env::doctrees_dir(&config)),
            _dir: dir,
            config,
            options: BuildOptions::default(),
            env: Environment::default(),
            tables: Tables::default(),
            intersphinx: Intersphinx::default(),
            registry: Registry {
                readers: guidedoc_defaults::registry().readers,
                renderers: vec![guidedoc_typst::RENDERER, guidedoc_text::RENDERER],
            },
            policy: Policy { raw: ContentPolicy::Allow, ..Policy::default() },
            limits: Limits::default(),
            dates: Dates { today: "2026-10-15".into(), ..Dates::default() },
            plugins: PluginRun::default(),
            budget: gd_host::Budget::new(1 << 30),
        };
        for (docname, text) in docs {
            project.add(docname, text);
        }
        project.index_tables(docs);
        project
    }

    fn source(&self) -> &Path {
        &self.config.source_dir
    }

    /// Reads a document with the rst reader, saves its object, and records it.
    fn add(&mut self, docname: &str, text: &str) {
        let relative = format!("{docname}.rst");
        let path = self.source().join(&relative);
        std::fs::write(&path, text).unwrap();
        let name = path.display().to_string();
        let mut ws = Workspace::new();
        let request = Request {
            source: guidedoc::Source { name: &name, text },
            policy: self.policy,
            ..Request::default()
        };
        let reader = self.registry.readers.iter().find(|reader| reader.id == "sphinx").unwrap();
        let doc = guidedoc::read_document(&mut ws, reader, &request).unwrap();
        let object = self.objects.save_object(docname, doc).unwrap();
        let title = text.lines().find(|l| !l.is_empty() && !l.starts_with("..")).unwrap();
        self.env.docs.push(DocInfo {
            docname: docname.into(),
            relative,
            kind: SourceKind::Rst,
            digest: Digest::of_text(text),
            object: Some(object),
            title: title.into(),
            ..DocInfo::default()
        });
        self.env.docs.sort_by(|a, b| a.docname.cmp(&b.docname));
    }

    /// The tables gd-resolve would make for one toctree: `index` holds the others, in
    /// the order the documents were given.
    fn index_tables(&mut self, docs: &[(&str, &str)]) {
        let n = self.env.docs.len();
        let t = &mut self.tables;
        t.by_name = self.env.iter().map(|(id, d)| (d.docname.clone(), id)).collect();
        t.order = docs.iter().map(|(name, _)| t.by_name[*name]).collect();
        let root = t.by_name["index"];
        t.parent = (0..n).map(|i| (DocId(i as u32) != root).then_some(root)).collect();
        t.trees = vec![vec![]; n];
        t.secnumbers =
            self.env.docs.iter().map(|d| vec![String::new(); d.sections.len()]).collect();
        t.fignumbers =
            self.env.docs.iter().map(|d| vec![String::new(); d.numbered.len()]).collect();
        t.keys = self.env.docs.iter().map(|d| chapter_key(&d.docname)).collect();
    }

    fn view(&self, kind: BuilderKind) -> BuildView<'_> {
        BuildView {
            config: &self.config,
            options: &self.options,
            kind,
            env: &self.env,
            tables: &self.tables,
            intersphinx: &self.intersphinx,
            objects: &self.objects,
            registry: &self.registry,
            policy: &self.policy,
            limits: &self.limits,
            ui: &EnglishUi,
            dates: &self.dates,
            plugins: &self.plugins,
            budget: &self.budget,
            roots: &self.roots,
            settings: Digest::default(),
        }
    }

    /// Runs a builder into a generation and publishes it. Returns the output folder,
    /// the reports, and the builder's problem.
    fn build(&self, builder: &dyn OutputBuilder) -> (PathBuf, Vec<Report>, Option<Problem>) {
        let view = self.view(builder.kind());
        let paths = gd_outputs::output_paths(&self.config, builder.kind(), None).unwrap();
        let out = paths.out_dir.clone();
        let mut reports = Vec::new();
        let options = GenerationOptions { all: false, budget: &self.budget };
        let mut generation = Generation::begin(paths, options, &mut reports).unwrap();
        if let Err(problem) = builder.write(&view, &mut generation, &mut reports) {
            return (out, reports, Some(problem));
        }
        let records = CommitRecords { environment: &self.env, store: &self.objects };
        let outcome = generation.commit(records);
        assert!(outcome.problem.is_none(), "{:?}", outcome.problem);
        (out, reports, None)
    }
}

const INDEX: &str = "Demo Manual\n===========\n\nWelcome to the demo manual.\n";

const GUIDE: &str = "\
Getting Started
===============

Install the tool with care.

.. image:: logo.svg

Next steps
----------

Read the code::

    let answer = 42;
";

const LOGO: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"10\">\
                    <rect width=\"20\" height=\"10\" fill=\"#126d78\"/></svg>\n";

impl Project {
    /// Runs the pdf builder like [`Project::build`], and returns the books Typst
    /// typeset in this build as well.
    fn typeset(&self) -> (PathBuf, Vec<Report>, Option<Problem>, Vec<TypesetBook>) {
        let books = Mutex::new(Vec::new());
        let builder = PdfBuilder { typeset: Some(&books), ..PdfBuilder::default() };
        let (out, reports, problem) = self.build(&builder);
        (out, reports, problem, books.into_inner().unwrap())
    }
}

fn errors(reports: &[Report]) -> Vec<&Report> {
    reports.iter().filter(|r| r.severity == gd_report::Severity::Error).collect()
}

#[test]
fn a_small_book_is_typeset_and_kept_when_unchanged() {
    let project = Project::new("pdf", &[("index", INDEX), ("guide", GUIDE)]);
    std::fs::write(project.source().join("logo.svg"), LOGO).unwrap();
    let (out, reports, problem, books) = project.typeset();
    assert!(problem.is_none(), "{problem:?}\n{reports:#?}");
    assert!(errors(&reports).is_empty(), "{reports:#?}");
    let pdf = out.join("demo-en.pdf");
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let source = std::fs::read_to_string(out.join("sources/demo-en.pdf.typ")).unwrap();
    assert_eq!(std::fs::read_to_string(out.join("book-en.typ")).unwrap(), source);
    assert!(source.contains("#show: book.with(title: \"Demo Book\", author: \"Ada\""));
    assert!(source.contains("#metadata(none) <guide>"));
    assert!(source.contains("\"/logo.svg\""), "the image is named by its book path");
    assert_eq!(books.len(), 1, "{books:#?}");
    assert_eq!(books[0].file, "demo-en.pdf");
    let pages = books[0].pages.as_ref().expect("the embedded compiler reports its pages");
    assert!(pages.len() >= 2, "a cover and the chapters: {pages:#?}");
    assert!(pages[0].contains("Demo Book"), "the cover: {pages:#?}");
    let text = pages.join("\n");
    assert!(text.contains("Getting Started"), "{text}");
    assert!(text.contains("Install the tool with care."), "{text}");
    assert!(text.contains("let answer = 42;"), "{text}");
    // Nothing changed: the book is kept, not typeset again.
    let before = std::fs::metadata(&pdf).unwrap().modified().unwrap();
    let (_, _, problem, books) = project.typeset();
    assert!(problem.is_none(), "{problem:?}");
    assert!(books.is_empty(), "{books:#?}");
    assert_eq!(std::fs::metadata(&pdf).unwrap().modified().unwrap(), before);
    // A changed image is a changed input.
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(project.source().join("logo.svg"), LOGO.replace("126d78", "aa0000")).unwrap();
    let (_, _, problem, books) = project.typeset();
    assert!(problem.is_none(), "{problem:?}");
    assert_eq!(books.len(), 1, "{books:#?}");
    assert_ne!(std::fs::metadata(&pdf).unwrap().modified().unwrap(), before);
}

#[test]
fn typst_errors_point_at_the_document_source() {
    let bad = "Broken\n======\n\nFine text.\n\n.. raw:: typst\n\n   #nope-not-defined\n";
    let project = Project::new("pdf-error", &[("index", INDEX), ("bad", bad)]);
    let (out, reports, problem, books) = project.typeset();
    assert!(books.is_empty(), "a failed book is not typeset: {books:#?}");
    let problem = problem.expect("Typst fails");
    assert_eq!(problem.code, "typst.compile");
    let error = reports.iter().find(|r| r.code == "typst.compiler").expect("{reports:#?}");
    assert!(error.path.ends_with("bad.rst"), "{error:#?}");
    // The source map places the raw block (lines 6 to 8), which holds the bad call.
    assert_eq!((error.line, error.end_line), (6, 8), "{error:#?}");
    assert!(error.message.contains("which Guidedog generated from this source"), "{error:#?}");
    assert!(error.hints[0].contains("failed"), "{error:#?}");
    let kept = gd_env::doctrees_dir(&project.config).join("failed/book-en.typ");
    assert!(std::fs::read_to_string(kept).unwrap().contains("#nope-not-defined"));
    assert!(!out.join("demo-en.pdf").exists(), "nothing is published");
}

#[test]
fn books_need_distinct_files_and_existing_roots() {
    let mut project = Project::new("pdf-config", &[("index", INDEX)]);
    let book = project.config.pdf.documents[0].clone();
    project.config.pdf.documents.push(PdfDocument { file: "demo-en.pdf".into(), ..book });
    let (_, _, problem) = project.build(&PdfBuilder::default());
    assert_eq!(problem.unwrap().code, "build.pdf.duplicate");
    project.config.pdf.documents.truncate(1);
    project.config.pdf.documents[0].root = "missing".into();
    let (_, _, problem) = project.build(&PdfBuilder::default());
    assert_eq!(problem.unwrap().code, "build.pdf.root");
}

#[test]
fn the_text_builder_writes_one_file_per_document() {
    let project = Project::new("text", &[("index", INDEX), ("guide", GUIDE)]);
    let (out, reports, problem) = project.build(&TextBuilder::default());
    assert!(problem.is_none(), "{problem:?}\n{reports:#?}");
    let guide = std::fs::read_to_string(out.join("guide.txt")).unwrap();
    assert!(guide.contains("Getting Started"), "{guide}");
    assert!(guide.contains("let answer = 42;"), "{guide}");
    assert!(std::fs::read_to_string(out.join("index.txt")).unwrap().contains("Welcome"));
    // Unchanged pages are kept with their reports.
    let before = std::fs::metadata(out.join("guide.txt")).unwrap().modified().unwrap();
    let (_, again, problem) = project.build(&TextBuilder::default());
    assert!(problem.is_none(), "{problem:?}");
    assert_eq!(again.len(), reports.len());
    let after = std::fs::metadata(out.join("guide.txt")).unwrap().modified().unwrap();
    assert_eq!(before, after);
}

#[test]
fn the_dummy_builder_writes_nothing() {
    let project = Project::new("dummy", &[("index", INDEX), ("guide", GUIDE)]);
    let (out, _, problem) = project.build(&DummyBuilder);
    assert!(problem.is_none(), "{problem:?}");
    assert_eq!(std::fs::read_dir(&out).map_or(0, |d| d.count()), 0);
}

#[test]
fn typst_reads_only_the_folders_a_build_may_read() {
    let peek = "Peek\n====\n\n.. raw:: typst\n\n   #read(\"/secret.txt\")\n";
    let mut project = Project::new("pdf-confined", &[("index", INDEX), ("peek", peek)]);
    // The build folder beside the source folder makes their parent the book's root.
    let parent = project.source().parent().unwrap().to_path_buf();
    project.config.build_dir = parent.join("build");
    project.objects = ObjectStore::new(gd_env::doctrees_dir(&project.config));
    for doc in project.env.docs.clone() {
        let text = std::fs::read_to_string(project.source().join(&doc.relative)).unwrap();
        project.env.docs.retain(|d| d.docname != doc.docname);
        project.add(&doc.docname, &text);
    }
    std::fs::create_dir_all(gd_env::doctrees_dir(&project.config).join("graphs")).unwrap();
    std::fs::write(parent.join("secret.txt"), "secret").unwrap();
    let (_, reports, problem, books) = project.typeset();
    assert!(books.is_empty(), "{books:#?}");
    assert_eq!(problem.expect("the read is refused").code, "typst.compile");
    let error = reports.iter().find(|r| r.code == "typst.compiler").expect("{reports:#?}");
    assert!(error.message.contains("denied"), "{error:#?}");
    assert!(error.path.ends_with("peek.rst"), "{error:#?}");
}

#[test]
fn an_untrusted_build_uses_guidedogs_template() {
    let mut project = Project::new("pdf-untrusted", &[("index", INDEX)]);
    let templates = project.source().join("_templates");
    std::fs::create_dir_all(&templates).unwrap();
    std::fs::write(templates.join("book.typ"), "#let book(..args, body) = body\n").unwrap();
    let (out, reports, problem, books) = project.typeset();
    assert!(problem.is_none(), "{problem:?}\n{reports:#?}");
    let source = std::fs::read_to_string(out.join("book-en.typ")).unwrap();
    assert!(source.contains("#import \"/_templates/book.typ\": book"));
    // The project's template sets the body alone: no cover with the book's title.
    let pages = books[0].pages.as_ref().unwrap();
    assert!(!pages[0].contains("Demo Book"), "{pages:#?}");
    assert!(pages.join("\n").contains("Welcome to the demo manual."), "{pages:#?}");
    project.options.untrusted = true;
    let (out, reports, problem, books) = project.typeset();
    assert!(problem.is_none(), "{problem:?}\n{reports:#?}");
    let pages = books[0].pages.as_ref().unwrap();
    assert!(pages[0].contains("Demo Book"), "Guidedog's cover: {pages:#?}");
    assert!(reports.iter().any(|r| r.code == "build.untrusted"), "{reports:#?}");
    let source = std::fs::read_to_string(out.join("book-en.typ")).unwrap();
    assert!(!source.contains("_templates/book.typ"));
}

/// The typst program runs in a private folder of links to the readable folders; it
/// is skipped when the program is not installed.
#[test]
fn the_typst_program_typesets_in_a_private_root() {
    let Some(program) = gd_typst::find_program("typst") else { return };
    let mut project = Project::new("pdf-program", &[("index", INDEX), ("guide", GUIDE)]);
    std::fs::write(project.source().join("logo.svg"), LOGO).unwrap();
    project.config.pdf.typst = program.display().to_string();
    project.config.build_dir = project.source().parent().unwrap().join("build");
    let (out, reports, problem, books) = project.typeset();
    assert!(problem.is_none(), "{problem:?}\n{reports:#?}");
    let pdf = out.join("demo-en.pdf");
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    // The program hands back only the file, so the text is checked in the embedded
    // builds above; here the book is recorded as typeset without pages.
    assert_eq!(books.len(), 1, "{books:#?}");
    assert_eq!(books[0].pages, None);
    let temp = std::fs::read_dir(std::env::temp_dir()).unwrap().flatten();
    let pid = format!("guidedog-typst-{}-", std::process::id());
    let left: Vec<_> =
        temp.filter(|e| e.file_name().to_string_lossy().starts_with(&pid)).collect();
    assert!(left.is_empty(), "the private root is removed: {left:?}");
}

/// Checks pagination and every row on the pages returned by the embedded compiler.
fn assert_long_block(name: &str, source: &str, copied_template: bool, header: Option<&str>) {
    let project = Project::new(name, &[("index", source)]);
    if copied_template {
        let templates = project.source().join("_templates");
        std::fs::create_dir_all(&templates).unwrap();
        std::fs::write(templates.join("book.typ"), gd_theme::BOOK_TYP).unwrap();
        std::fs::write(templates.join("theme.typ"), gd_theme::PDF_THEME_TYP).unwrap();
    }
    let (_, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    assert_eq!(books.len(), 1);
    let pages = books[0].pages.as_ref().expect("the embedded compiler reports its pages");
    assert_paginated_entries(pages, header);
}

fn assert_paginated_entries(pages: &[String], header: Option<&str>) {
    let first = pages.iter().position(|page| page.contains("entry001")).unwrap();
    let last = pages.iter().position(|page| page.contains("entry120")).unwrap();
    assert!(first < last, "the long block must span pages, not overflow one page: {pages:#?}");
    for row in 1..=120 {
        let marker = format!("entry{row:03}");
        let count: usize = pages.iter().map(|page| page.matches(&marker).count()).sum();
        assert_eq!(count, 1, "{marker} must appear exactly once");
    }
    if let Some(header) = header {
        for page in &pages[first..=last] {
            assert!(page.contains(header), "the table header must repeat: {page}");
        }
    }
}

#[test]
fn long_tables_continue_across_pages_with_repeated_headers() {
    for caption in ["A long table", ""] {
        let source = long_table_source(caption);
        for copied in [false, true] {
            let name = format!("long-table-{}-{copied}", !caption.is_empty());
            assert_long_block(&name, &source, copied, Some("Repeatedheader"));
        }
    }
}

#[test]
fn long_captioned_listings_continue_across_pages() {
    let source = long_listing_source();
    for copied in [false, true] {
        assert_long_block(&format!("long-listing-{copied}"), &source, copied, None);
    }
}

fn long_table_source(caption: &str) -> String {
    let mut source =
        format!("Long table\n==========\n\n.. list-table:: {caption}\n   :header-rows: 1\n\n");
    source.push_str("   * - Repeatedheader\n     - Value\n");
    for row in 1..=120 {
        writeln!(source, "   * - entry{row:03}\n     - value{row:03}").unwrap();
    }
    source
}

fn long_listing_source() -> String {
    let mut source = String::from(
        "Long listing\n============\n\n.. code-block:: text\n   :caption: A long listing\n\n",
    );
    for row in 1..=120 {
        writeln!(source, "   entry{row:03}").unwrap();
    }
    source
}

#[test]
fn standalone_conversion_paginates_long_captioned_content() {
    let sources = [
        (long_table_source("A long table"), Some("Repeatedheader")),
        (long_listing_source(), None),
    ];
    for (index, (source, header)) in sources.into_iter().enumerate() {
        let dir = TempDir::new(&format!("standalone-long-{index}"));
        let mut workspace = Workspace::new();
        let registry = guidedoc_defaults::registry();
        let typst = guidedoc::convert_text(&mut workspace, &registry, "sphinx", "typst", &source)
            .expect("the document converts to standalone Typst");
        let options = gd_typst::Options {
            offline: true,
            embedded_fonts_only: true,
            ..gd_typst::Options::default()
        };
        let backend = gd_typst::locate("typst").expect("the embedded compiler");
        let output = dir.0.join("long.pdf");
        let compiled = gd_typst::compile_text(&backend, typst, &dir.0, &output, &options)
            .expect("the standalone document compiles");
        assert_paginated_entries(compiled.pages.as_ref().unwrap(), header);
    }
}

const DISCUSSION: &str = ":eds-number: 0007\n:eds-title: Cache policy\n\
    :eds-state: accepted\n:eds-type: Standards Track\n\
    :eds-authors: [\"Ada\", \"René\"]\n:eds-created: 2026-10-01\n\
    :eds-updated: 2026-10-16\n:eds-labels: [\"architecture\"]\n\
    :eds-discussion: https://example.org/review\n\n\
    Cache policy\n============\n\nAbstract\n--------\n\n\
    A bounded cache keeps the previous generation usable.\n\n\
    Specification\n-------------\n\nSee `Abstract`_.\n";

#[test]
fn discussions_start_with_their_content_and_structured_front_matter() {
    let mut project = Project::new("discussion-layout", &[("index", DISCUSSION)]);
    let templates = project.source().join("_templates");
    std::fs::create_dir_all(&templates).unwrap();
    // Old catalogs may carry a copied book template. Records have their own override.
    std::fs::write(templates.join("book.typ"), "#panic(\"book template used\")").unwrap();
    let (out, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    let pages = books[0].pages.as_ref().unwrap();
    assert_eq!(pages.len(), 1, "a short discussion needs no cover or blank pages: {pages:#?}");
    for text in ["EDS 0007", "Cache policy", "Ada, René", "accepted", "Abstract", "bounded cache"]
    {
        assert!(pages[0].contains(text), "missing {text:?}: {pages:#?}");
    }
    let source = std::fs::read_to_string(out.join("book-en.typ")).unwrap();
    assert!(source.contains("record: (id: \"EDS 0007\""));
    assert!(!source.contains("#gd-fields("));
    assert!(!source.contains("#import \"/_templates/book.typ\""));
    // The selected template is an input: editing it invalidates a cached PDF.
    let copied = gd_theme::DISCUSSION_TYP.replace("size: 10.5pt", "size: 10pt");
    std::fs::write(templates.join("discussion.typ"), copied).unwrap();
    std::fs::write(templates.join("theme.typ"), gd_theme::PDF_THEME_TYP).unwrap();
    let (_, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    assert_eq!(books.len(), 1, "a new discussion template must rebuild the PDF");
    let theme = gd_theme::PDF_THEME_TYP.replace("#126d78", "#196b62");
    std::fs::write(templates.join("theme.typ"), theme).unwrap();
    let (_, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    assert_eq!(books.len(), 1, "editing the shared theme must rebuild the PDF");
    std::fs::write(templates.join("discussion.typ"), "#panic(\"untrusted template used\")")
        .unwrap();
    std::fs::write(templates.join("theme.typ"), "#panic(\"untrusted theme used\")").unwrap();
    project.options.untrusted = true;
    let (_, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    assert!(reports.iter().any(|report| report.code == "build.untrusted"));
    assert!(books[0].pages.as_ref().unwrap()[0].contains("EDS 0007"));
}

#[test]
fn discussion_tables_and_listings_continue_on_following_pages() {
    let cases =
        [(long_table_source("Long table"), Some("Repeatedheader")), (long_listing_source(), None)];
    for (index, (source, header)) in cases.into_iter().enumerate() {
        let text = format!("{DISCUSSION}\n{source}");
        for copied in [false, true] {
            let name = format!("discussion-long-{index}-{copied}");
            let project = Project::new(&name, &[("index", &text)]);
            if copied {
                let templates = project.source().join("_templates");
                std::fs::create_dir_all(&templates).unwrap();
                std::fs::write(templates.join("discussion.typ"), gd_theme::DISCUSSION_TYP)
                    .unwrap();
                std::fs::write(templates.join("theme.typ"), gd_theme::PDF_THEME_TYP).unwrap();
            }
            let (_, reports, problem, books) = project.typeset();
            assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
            assert_paginated_entries(books[0].pages.as_ref().unwrap(), header);
        }
    }
}

#[test]
fn partial_discussion_metadata_keeps_the_book_layout() {
    let text = ":eds-number: 7\n\nOrdinary manual\n===============\n\nWelcome.\n";
    let project = Project::new("ordinary-layout", &[("index", text)]);
    let (_, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    let pages = books[0].pages.as_ref().unwrap();
    assert!(pages[0].contains("Demo Book"), "the book keeps its cover: {pages:#?}");
    assert!(!pages[0].contains("Welcome"));
}

#[test]
fn imported_pdf_modules_invalidate_cached_books() {
    let project = Project::new("pdf-modules", &[("index", INDEX)]);
    let templates = project.source().join("_templates");
    std::fs::create_dir_all(templates.join("parts")).unwrap();
    std::fs::write(
        templates.join("book.typ"),
        "#import \"parts/style.typ\": decorate\n#let book(..args, body) = decorate(body)",
    )
    .unwrap();
    let module = templates.join("parts/style.typ");
    std::fs::write(&module, "#let decorate(body) = [First module #body]").unwrap();
    let (_, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    assert!(books[0].pages.as_ref().unwrap()[0].contains("First module"));
    let (_, _, _, books) = project.typeset();
    assert!(books.is_empty(), "an unchanged imported module keeps the cached PDF");
    std::fs::write(&module, "#let decorate(body) = [Other module #body]").unwrap();
    let (_, reports, problem, books) = project.typeset();
    assert!(problem.is_none() && errors(&reports).is_empty(), "{problem:?} {reports:#?}");
    assert!(books[0].pages.as_ref().unwrap()[0].contains("Other module"));
    std::fs::remove_file(module).unwrap();
    let (_, _, problem, _) = project.typeset();
    assert_eq!(problem.unwrap().code, "typst.compile", "missing modules must not reuse a PDF");
}
