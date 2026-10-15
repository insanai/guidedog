//! Incremental builds: a page is rendered again only when its inputs or the answers to
//! its lookups change; a kept page reports its warnings again. The renderer counts its
//! calls, so this file holds a single test (the count is per process).

#![allow(clippy::unwrap_used)]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::{Project, section, toctree};
use gd_env::{BuilderKind, LabelInfo};
use guidedoc::{Fail, RenderContext, Renderer};

static RENDERS: AtomicUsize = AtomicUsize::new(0);

fn counting_render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    RENDERS.fetch_add(1, Ordering::SeqCst);
    guidedoc_html::render(ctx)
}

const COUNTING: Renderer = Renderer { render: counting_render, ..guidedoc_html::RENDERER };

fn renders() -> usize {
    RENDERS.swap(0, Ordering::SeqCst)
}

const INDEX: &str =
    "Home\n====\n\nSee :ref:`the widgets <widgets>`.\n\n.. toctree::\n\n   intro\n   api\n";
const API: &str = "API\n===\n\nSee :doc:`missing`.\n";

fn intro(text: &str) -> String {
    format!("Intro\n=====\n\n.. _widgets:\n\n{text}\n")
}

fn project() -> Project {
    let mut p = Project::with_renderer("incremental", COUNTING);
    let index = p.add("index.rst", "Home", INDEX);
    index.sections = vec![section(1, "Home", "home", None)];
    index.toctrees = vec![toctree(Some(0), 1, &["intro", "api"])];
    add_intro(&mut p, "Widgets are small.", "widgets");
    p.add("api.rst", "API", API).sections = vec![section(1, "API", "api", None)];
    p.words("api", "api\tAPI\tSee missing.\n");
    p.resolve();
    p
}

fn add_intro(p: &mut Project, text: &str, anchor: &str) {
    let intro = p.add("intro.rst", "Intro", &intro(text));
    intro.sections = vec![section(1, "Intro", "intro", None)];
    intro.labels =
        vec![LabelInfo { name: "widgets".into(), anchor: anchor.into(), ..LabelInfo::default() }];
}

fn warnings(reports: &[gd_report::Report]) -> usize {
    reports.iter().filter(|r| r.code == "build.xref.unresolved").count()
}

#[test]
fn only_changed_pages_and_pages_whose_lookups_changed_are_rendered_again() {
    let mut p = project();
    let html = BuilderKind::Html;
    let first = p.build(html).unwrap();
    assert_eq!(renders(), 3);
    assert_eq!(warnings(&first), 1, "{first:?}");
    let index_page = p.read(html, "index.html");
    assert!(index_page.contains("href=\"intro.html#widgets\""), "{index_page}");

    // Nothing changed: every page and the search index are kept, warnings repeated.
    let second = p.build(html).unwrap();
    assert_eq!(renders(), 0);
    assert_eq!(warnings(&second), 1, "a kept page reports its warnings again");
    assert_eq!(p.read(html, "index.html"), index_page);

    // The text of one page changed: only that page is rendered.
    add_intro(&mut p, "Widgets are tiny.", "widgets");
    p.resolve();
    p.build(html).unwrap();
    assert_eq!(renders(), 1);
    assert!(p.read(html, "intro.html").contains("Widgets are tiny."));

    // The label moved: the page that refers to it is rendered too.
    add_intro(&mut p, "Widgets are tiny.", "widgets-moved");
    p.resolve();
    p.build(html).unwrap();
    assert_eq!(renders(), 1, "only the referring page: intro's own inputs are unchanged");
    assert!(p.read(html, "index.html").contains("href=\"intro.html#widgets-moved\""));

    // A title changed: the navigation of every page changes.
    p.doc("api").title = "Reference".into();
    p.resolve();
    p.build(html).unwrap();
    assert_eq!(renders(), 3);
    assert!(p.read(html, "index.html").contains(">Reference</a>"));
    let search = p.read(html, "_static/searchindex.js");
    assert!(search.contains("[\"Reference\",\"api.html\"]"), "{search}");
}
