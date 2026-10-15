//! End-to-end builds of small projects made by hand: reStructuredText documents read
//! with guidedoc-rst, saved to an object store, and an environment and tables filled
//! as gd-read and gd-resolve would (gd-read is not ported yet). The PDF builds run the
//! embedded Typst compiler; their pages and text are checked on what Typst laid out,
//! as the builder hands it back.

#![allow(clippy::unwrap_used)]

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
                readers: vec![guidedoc_rst::reader()],
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
        let doc = guidedoc::read_document(&mut ws, &guidedoc_rst::reader(), &request).unwrap();
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
