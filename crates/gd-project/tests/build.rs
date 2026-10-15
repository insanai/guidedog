//! End to end: a small project built with every builder, rebuilt incrementally, and
//! built `--untrusted`.

#![allow(clippy::unwrap_used)]

mod common;

use std::time::SystemTime;

use common::{assert_built, build, demo, load, report_with};
use gd_project::{BuildOptions, BuildStage, BuilderKind};

#[test]
fn html_pages_link_to_each_other() {
    let dir = demo("html");
    let r = build(&load(&dir.0), BuilderKind::Html);
    assert_built(&r);
    assert_eq!((r.read, r.reused, r.unread), (3, 0, 0));
    assert_eq!(r.reached, BuildStage::Written);
    assert_eq!(r.output, dir.path("_build/html"));
    let index = dir.read("_build/html/index.html");
    assert!(index.contains("href=\"guide.html\""), "{index}");
    assert!(index.contains("href=\"notes.html\""), "{index}");
    let notes = dir.read("_build/html/notes.html");
    assert!(notes.contains("guide.html#usage"), "{notes}");
    for file in ["search.html", "genindex.html", "objects.inv", "_static/searchindex.js"] {
        assert!(dir.exists(&format!("_build/html/{file}")), "{file}");
    }
}

#[test]
fn dirhtml_gives_each_page_a_folder() {
    let dir = demo("dirhtml");
    assert_built(&build(&load(&dir.0), BuilderKind::Dirhtml));
    assert!(dir.exists("_build/dirhtml/index.html"));
    let guide = dir.read("_build/dirhtml/guide/index.html");
    assert!(guide.contains("Use it."), "{guide}");
}

#[test]
fn singlehtml_puts_every_document_on_one_page() {
    let dir = demo("singlehtml");
    let r = build(&load(&dir.0), BuilderKind::Singlehtml);
    assert_built(&r);
    assert_eq!(r.written, 1);
    let page = dir.read("_build/singlehtml/index.html");
    assert!(page.contains("Use it.") && page.contains("A note about"), "{page}");
    assert!(!dir.exists("_build/singlehtml/guide.html"));
}

#[test]
fn text_writes_one_file_per_document() {
    let dir = demo("text");
    let r = build(&load(&dir.0), BuilderKind::Text);
    assert_built(&r);
    assert_eq!(r.written, 3);
    assert!(dir.read("_build/text/guide.txt").contains("Use it."));
}

#[test]
fn gettext_extracts_message_templates() {
    let dir = demo("gettext");
    let r = build(&load(&dir.0), BuilderKind::Gettext);
    assert_built(&r);
    let pot = dir.read("_build/gettext/guide.pot");
    assert!(pot.contains("msgid \"Use it.\""), "{pot}");
}

#[test]
fn dummy_checks_references_and_writes_nothing() {
    let dir = demo("dummy");
    dir.put("guide.rst", "Guide\n=====\n\nSee :ref:`nowhere`.\n");
    let r = build(&load(&dir.0), BuilderKind::Dummy);
    assert_eq!(r.exit, 0, "{:#?}", r.problem);
    assert!(report_with(&r, "build.xref.unresolved").is_some(), "{:#?}", r.reports);
    assert!(!dir.exists("_build/dummy/guide.html"));
}

#[test]
fn pdf_typesets_one_book() {
    let dir = demo("pdf");
    dir.put(
        "conf.toml",
        "project = \"Demo\"\nauthor = \"Ada\"\nsource_suffix = [\".rst\", \".md\"]\n\
         extensions = [\"myst_parser\"]\npdf_fonts = \"embedded\"\npdf_packages = \"offline\"\n",
    );
    let r = build(&load(&dir.0), BuilderKind::Pdf);
    assert_built(&r);
    let pdf = std::fs::read(dir.path("_build/pdf/demo-en.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    assert!(dir.exists("_build/pdf/book-en.typ"));
}

/// One changed document is read again and its page rewritten; the other pages stay.
#[test]
fn a_rebuild_rewrites_only_what_changed() {
    let dir = demo("incremental");
    let project = load(&dir.0);
    assert_built(&build(&project, BuilderKind::Html));
    let modified = |name: &str| -> SystemTime {
        std::fs::metadata(dir.path(name)).unwrap().modified().unwrap()
    };
    let guide_before = modified("_build/html/guide.html");
    let index_before = modified("_build/html/index.html");
    let again = build(&project, BuilderKind::Html);
    assert_built(&again);
    assert_eq!((again.read, again.reused, again.written), (0, 3, 0));
    std::thread::sleep(std::time::Duration::from_millis(20));
    dir.put("notes.md", "# Notes\n\nA changed note about {ref}`usage`.\n");
    let changed = build(&project, BuilderKind::Html);
    assert_built(&changed);
    assert_eq!((changed.read, changed.reused), (1, 2));
    // The notes page, and the search index its words are in.
    assert_eq!((changed.written, changed.unchanged), (2, 2));
    assert!(dir.read("_build/html/notes.html").contains("A changed note"));
    assert_eq!(modified("_build/html/guide.html"), guide_before);
    assert_eq!(modified("_build/html/index.html"), index_before);
    let fresh = gd_project::build(
        &project,
        BuilderKind::Html,
        &BuildOptions { fresh: true, all: true, ..Default::default() },
    );
    assert_built(&fresh);
    assert_eq!((fresh.read, fresh.written), (3, 4));
}

/// A build with -W publishes nothing when it warns, and keeps the previous output.
#[test]
fn warnings_as_errors_keep_the_previous_output() {
    let dir = demo("strict");
    let project = load(&dir.0);
    assert_built(&build(&project, BuilderKind::Html));
    dir.put("guide.rst", "Guide\n=====\n\nSee :ref:`nowhere`.\n");
    let strict = BuildOptions { warnings_as_errors: true, ..Default::default() };
    let r = gd_project::build(&project, BuilderKind::Html, &strict);
    assert_eq!(r.problem.as_ref().map(|p| p.code), Some("build.warnings"));
    assert_eq!(r.exit, 1);
    assert!(!r.published);
    assert!(dir.read("_build/html/guide.html").contains("Use it."));
    // suppress_warnings silences the reference's category, and the build publishes.
    let quiet = gd_project::LoadOptions {
        config: gd_conf::LoadOptions {
            overrides: vec!["suppress_warnings=[\"ref\"]".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    let project = gd_project::load(&dir.0, &quiet).unwrap();
    let r = gd_project::build(&project, BuilderKind::Html, &strict);
    assert_built(&r);
}

#[test]
fn untrusted_builds_leave_out_raw_content_and_refuse_plugins() {
    let dir = demo("untrusted");
    dir.put(
        "guide.rst",
        "Guide\n=====\n\n.. raw:: html\n\n   <script>alert(1)</script>\n\nText.\n",
    );
    let project = load(&dir.0);
    let untrusted = BuildOptions { untrusted: true, ..Default::default() };
    let r = gd_project::build(&project, BuilderKind::Html, &untrusted);
    assert_built(&r);
    assert!(!dir.read("_build/html/guide.html").contains("<script>alert"));
    let trusted = build(&project, BuilderKind::Html);
    assert_built(&trusted);
    assert!(dir.read("_build/html/guide.html").contains("<script>alert"));
    // A project that names include roots has them dropped, with a warning.
    dir.put(
        "conf.toml",
        "project = \"Demo\"\nsource_suffix = [\".rst\", \".md\"]\n\
         extensions = [\"myst_parser\"]\ninclude_roots = [\"..\"]\n",
    );
    let r = gd_project::build(&load(&dir.0), BuilderKind::Html, &untrusted);
    assert!(report_with(&r, "build.untrusted").is_some(), "{:#?}", r.reports);
    // Plugins never start in an untrusted build.
    dir.put(
        "conf.toml",
        "project = \"Demo\"\nsource_suffix = [\".rst\", \".md\"]\n\
         extensions = [\"myst_parser\"]\nplugins = [\"badge\"]\n",
    );
    let r = gd_project::build(&load(&dir.0), BuilderKind::Html, &untrusted);
    assert_eq!(r.problem.as_ref().map(|p| p.code), Some("plugins.untrusted"));
    assert_eq!(r.exit, 2);
    assert!(!r.published);
}

/// The project is never changed by a build: an untrusted build confines only itself.
#[test]
fn builds_do_not_change_the_project() {
    let dir = demo("unchanged");
    dir.put(
        "conf.toml",
        "project = \"Demo\"\nsource_suffix = [\".rst\", \".md\"]\n\
         extensions = [\"myst_parser\"]\ninclude_roots = [\"..\"]\n",
    );
    let project = load(&dir.0);
    let before = format!("{:?}", project.config);
    let untrusted = BuildOptions { untrusted: true, ..Default::default() };
    let _ = gd_project::build(&project, BuilderKind::Html, &untrusted);
    assert_eq!(format!("{:?}", project.config), before);
}

/// The sphinx-build form: explicit source and output folders.
#[test]
fn an_explicit_output_folder_is_used() {
    let dir = demo("explicit");
    let out = dir.path("site");
    let project = load(&dir.0);
    let options = BuildOptions { output_dir: Some(out.clone()), ..Default::default() };
    let r = gd_project::build(&project, BuilderKind::Html, &options);
    assert_built(&r);
    assert_eq!(r.output, out);
    assert!(out.join("index.html").is_file());
}

#[test]
fn clean_empties_the_build_folder() {
    let dir = demo("clean");
    let project = load(&dir.0);
    assert_built(&build(&project, BuilderKind::Html));
    gd_project::clean(&project).unwrap();
    assert!(dir.path("_build").is_dir());
    assert_eq!(std::fs::read_dir(dir.path("_build")).unwrap().count(), 0);
    gd_project::clean(&project).unwrap();
}
