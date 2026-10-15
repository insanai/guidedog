//! End to end: a small project with reStructuredText, MyST, Python autodoc, Odin and
//! Rust API pages read into an environment, then read again incrementally.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::{DocInfo, Environment, ObjectStore, PluginRun, SourceFile};
use gd_host::Budget;
use gd_intl::Intl;
use gd_read::{ReadContext, ReadOutcome, read_documents, reread_document};
use gd_report::Severity;
use guidedoc::{Limits, Policy, Registry};

const INDEX: &str = "Demo\n====\n\n.. toctree::\n\n   guide\n\n\
                     .. automodule:: pkg\n   :members:\n\n\
                     See :py:class:`pkg.Thing` and :odin:proc:`shapes.area`.\n";

const GUIDE: &str = "# Guide\n\nSome *text*.\n\n```{py:function} guide_helper(x)\n\
                     Helps.\n```\n\n## Usage\n\nSee {py:func}`guide_helper`.\n";

const PACKAGE_INIT: &str = "\"\"\"The package.\"\"\"\n\nfrom .core import Thing\n\n\
                            __all__ = [\"Thing\", \"helper\"]\n\n\n\
                            def helper(x, y=2):\n    \"\"\"Helps with x.\"\"\"\n";

const PACKAGE_CORE: &str = "class Thing:\n    \"\"\"A thing.\"\"\"\n\n    #: How many there are.\n\
                            \x20   count = 3\n\n    def run(self, fast: bool = False) -> None:\n\
                            \x20       \"\"\"Runs.\"\"\"\n";

const SHAPES: &str = "// Package shapes measures plane figures.\npackage shapes\n\n\
                      // Shape is a figure and its size.\nShape :: struct {\n    size: f32,\n}\n\n\
                      // Measures a shape.\narea :: proc(s: Shape) -> f32 {\n    \
                      return s.size\n}\n";

const LIB: &str = "//! A demo crate.\n\npub mod geometry;\n";

const GEOMETRY: &str = "//! Plane geometry.\n\n/// A point.\npub struct Point {\n    /// Across.\n\
                        \x20   pub x: f32,\n}\n\nimpl Point {\n    /// Makes one.\n    \
                        pub fn new(x: f32) -> Self {\n        Point { x }\n    }\n}\n";

/// A project in a fresh folder, removed on drop.
struct Project {
    root: PathBuf,
    config: Config,
    registry: Registry,
    store: ObjectStore,
    intl: Intl,
    plugins: PluginRun,
    budget: Budget,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

impl Project {
    fn new(name: &str) -> Project {
        let root = std::env::temp_dir().join(format!("gd-read-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        write(&root.join("docs/index.rst"), INDEX);
        write(&root.join("docs/guide.md"), GUIDE);
        write(&root.join("src/pkg/__init__.py"), PACKAGE_INIT);
        write(&root.join("src/pkg/core.py"), PACKAGE_CORE);
        write(&root.join("odin/shapes/shapes.odin"), SHAPES);
        write(&root.join("rust/demo/lib.rs"), LIB);
        write(&root.join("rust/demo/geometry.rs"), GEOMETRY);
        let mut config = Config::new(root.join("docs"), root.join("docs/_build"));
        config.source_suffix = vec![".rst".into(), ".md".into()];
        config.extensions.push("sphinx.ext.autodoc".into());
        config.autodoc.source_paths = vec!["../src".into()];
        config.autodoc.member_order = "bysource".into();
        config.odin_api.dirs = vec!["../odin".into()];
        config.rust_api.crates = vec!["../rust/demo".into()];
        let store = ObjectStore::new(gd_env::doctrees_dir(&config));
        Project {
            root,
            config,
            registry: guidedoc_defaults::registry(),
            store,
            intl: Intl::default(),
            plugins: PluginRun::default(),
            budget: Budget::new(gd_env::BUILD_BUDGET),
        }
    }

    fn context(&self, jobs: usize) -> ReadContext<'_> {
        ReadContext {
            config: &self.config,
            registry: &self.registry,
            store: &self.store,
            intl: &self.intl,
            plugins: &self.plugins,
            budget: &self.budget,
            policy: Policy::default(),
            limits: Limits::default(),
            settings: gd_env::reading_settings(&self.config),
            today: "Oct 15, 2026",
            date: "2026-10-15",
            jobs,
        }
    }

    /// The documents: the source folder's and the generated API pages.
    fn sources(&self) -> Vec<SourceFile> {
        let found = gd_env::discover(&self.config).unwrap();
        let mut reports = Vec::new();
        let pages = gd_read::api_sources(&self.config, &found, &mut reports);
        assert!(reports.is_empty(), "{reports:#?}");
        gd_env::with_generated(found, pages)
    }

    fn read(&self, previous: Environment, jobs: usize) -> ReadOutcome {
        read_documents(&self.context(jobs), previous, &self.sources()).unwrap()
    }
}

fn doc<'e>(env: &'e Environment, docname: &str) -> &'e DocInfo {
    env.find(docname).unwrap_or_else(|| panic!("no document {docname}"))
}

fn has_object(d: &DocInfo, domain: &str, fullname: &str) -> bool {
    d.objects.iter().any(|o| o.domain == domain && o.fullname == fullname)
}

fn depends_on(d: &DocInfo, file: &str) -> bool {
    d.deps.iter().any(|dep| dep.path.ends_with(file))
}

/// No document failed or reported an error.
fn assert_clean(env: &Environment) {
    for d in &env.docs {
        let errors: Vec<_> = d.reports.iter().filter(|r| r.severity == Severity::Error).collect();
        assert!(!d.failed && errors.is_empty(), "{}: {:#?}", d.docname, d.reports);
    }
}

#[test]
fn a_project_reads_into_an_environment() {
    let p = Project::new("env");
    let outcome = p.read(Environment::default(), 4);
    let env = &outcome.env;
    let names: Vec<&str> = env.docs.iter().map(|d| d.docname.as_str()).collect();
    assert_eq!(
        names,
        [
            "api/index",
            "api/shapes",
            "guide",
            "index",
            "rustapi/demo",
            "rustapi/demo/geometry",
            "rustapi/index"
        ]
    );
    assert_clean(env);
    assert_eq!((outcome.read, outcome.reused, outcome.unread), (7, 0, 0));
    let index = doc(env, "index");
    assert_eq!(index.title, "Demo");
    assert!(index.written && index.object.is_some() && index.search.is_some());
    assert!(has_object(index, "py", "pkg.Thing"), "{:#?}", index.objects);
    assert!(has_object(index, "py", "pkg.Thing.run"));
    assert!(has_object(index, "py", "pkg.helper"));
    assert!(depends_on(index, "src/pkg/core.py") && depends_on(index, "src/pkg/__init__.py"));
    assert_eq!(index.toctrees[0].entries[0].target, "guide");
    let guide = doc(env, "guide");
    assert_eq!((guide.title.as_str(), guide.sections.len()), ("Guide", 2));
    assert!(has_object(guide, "py", "guide_helper"), "{:#?}", guide.objects);
    let shapes = doc(env, "api/shapes");
    assert!(shapes.generated);
    assert!(has_object(shapes, "odin", "shapes.area"), "{:#?}", shapes.objects);
    assert!(depends_on(shapes, "odin/shapes/shapes.odin"));
    let geometry = doc(env, "rustapi/demo/geometry");
    assert!(has_object(geometry, "rust", "demo::geometry::Point"), "{:#?}", geometry.objects);
    assert!(has_object(geometry, "rust", "demo::geometry::Point::new"));
    assert!(has_object(geometry, "rust", "demo::geometry"));
    assert!(
        depends_on(geometry, "rust/demo/geometry.rs") && depends_on(geometry, "rust/demo/lib.rs")
    );
    assert!(doc(env, "rustapi/index").toctrees[0].hidden);
}

#[test]
fn reading_again_parses_only_what_changed() {
    let p = Project::new("incremental");
    let first = p.read(Environment::default(), 2);
    let second = p.read(first.env.clone(), 2);
    assert_eq!((second.read, second.reused), (0, 7));
    let mut kept = first.env.clone();
    kept.docs.iter_mut().for_each(|d| d.written = false);
    assert_eq!(second.env, kept, "a reused reading is the previous one");

    // A Python file the index's autodoc read: only the index is read again.
    write(&p.root.join("src/pkg/core.py"), &PACKAGE_CORE.replace("Runs.", "Runs quickly."));
    let third = p.read(second.env, 2);
    assert_eq!((third.read, third.reused), (1, 6));
    assert!(doc(&third.env, "index").written);

    // A Rust file: the crate's pages (each made from every file of the crate).
    write(&p.root.join("rust/demo/geometry.rs"), &GEOMETRY.replace("A point.", "A spot."));
    let fourth = p.read(third.env, 2);
    assert_eq!((fourth.read, fourth.reused), (3, 4));
    assert!(doc(&fourth.env, "rustapi/demo/geometry").written);

    // A source of the folder, and a document whose object is gone.
    write(&p.root.join("docs/guide.md"), &GUIDE.replace("Some", "More"));
    let object = doc(&fourth.env, "api/index").object.clone().unwrap();
    std::fs::remove_file(p.store.path_of(&object)).unwrap();
    let fifth = p.read(fourth.env, 2);
    assert_eq!((fifth.read, fifth.reused), (2, 5));
    assert_clean(&fifth.env);
}

#[test]
fn threads_do_not_change_the_environment() {
    let p = Project::new("threads");
    let one = p.read(Environment::default(), 1);
    let many = p.read(Environment::default(), 8);
    assert_eq!(one, many);
}

#[test]
fn an_environment_of_other_settings_is_not_reused() {
    let p = Project::new("settings");
    let first = p.read(Environment::default(), 2);
    let mut other = first.env;
    other.settings = gd_conf::Digest::of_text("other settings");
    let again = p.read(other, 2);
    assert_eq!((again.read, again.reused), (7, 0));
}

#[test]
fn a_stale_object_is_read_again() {
    let p = Project::new("stale");
    let first = p.read(Environment::default(), 2);
    let sources = p.sources();
    let source = sources.iter().find(|s| s.docname == "index").unwrap();
    let old = doc(&first.env, "index");
    let fresh = reread_document(&p.context(1), source, old).unwrap();
    assert_eq!((fresh.digest, &fresh.object), (old.digest, &old.object), "the same object");
    let warning = gd_read::stale_warning(&gd_host::interrupted_problem(), old);
    assert_eq!(warning.code, "build.object.stale");
    // The source changed since the build read it: the new reading cannot stand in.
    write(&p.root.join("docs/index.rst"), &INDEX.replace("Demo", "Changed"));
    let problem = reread_document(&p.context(1), source, old).unwrap_err();
    assert_eq!(problem.code, "build.object.stale");
}

#[test]
fn missing_objects_and_readers_are_reported() {
    let p = Project::new("missing");
    write(&p.root.join("docs/index.rst"), "Demo\n====\n\n.. autoclass:: pkg.Missing\n");
    let outcome = p.read(Environment::default(), 2);
    let index = doc(&outcome.env, "index");
    let report = index.reports.iter().find(|r| r.code == "autodoc.import").unwrap();
    assert_eq!(report.category, "autodoc.import_object");
    assert_eq!(report.title, "PYTHON OBJECT NOT FOUND");
    assert!(index.deps.iter().any(|d| d.path.to_string_lossy().contains("not found")));
    // A document whose object was not found is read every build.
    let again = p.read(outcome.env, 2);
    assert_eq!(again.read, 1);

    let mut bare = Project::new("bare");
    bare.registry = Registry { readers: vec![guidedoc_rst::reader()], renderers: Vec::new() };
    let problem = read_documents(&bare.context(1), Environment::default(), &bare.sources());
    assert_eq!(problem.unwrap_err().code, "build.reader");
}

#[test]
fn documents_are_read_in_the_build_language() {
    let mut p = Project::new("intl");
    let po = "msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\n\
              msgid \"Demo\"\nmsgstr \"Vorführung\"\n";
    write(&p.root.join("docs/locales/de/LC_MESSAGES/index.po"), po);
    p.config.language = "de".into();
    p.config.intl.locale_dirs = vec!["locales".into()];
    let mut reports = Vec::new();
    p.intl = gd_intl::load_intl(&p.config, gd_env::BuilderKind::Html, &p.budget, &mut reports);
    assert!(reports.is_empty(), "{reports:#?}");
    let outcome = p.read(Environment::default(), 2);
    let index = doc(&outcome.env, "index");
    assert_eq!(index.title, "Vorführung");
    assert!(depends_on(index, "LC_MESSAGES/index.po"), "{:#?}", index.deps);
    // The catalog is a dependency: changing it reads the document again.
    write(
        &p.root.join("docs/locales/de/LC_MESSAGES/index.po"),
        &po.replace("Vorführung", "Schau"),
    );
    p.intl = gd_intl::load_intl(&p.config, gd_env::BuilderKind::Html, &p.budget, &mut reports);
    let again = p.read(outcome.env, 2);
    assert_eq!(again.read, 1);
    assert_eq!(doc(&again.env, "index").title, "Schau");
}
