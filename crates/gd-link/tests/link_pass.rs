//! The link pass over small projects: the rewriter's copy, `only`, toctrees,
//! references, todolists and graphs.

#![allow(clippy::unwrap_used)]

mod common;

use common::Project;
use gd_env::{OutputFormat, Rendered, SectionInfo, ToctreeInfo, TodoInfo};
use gd_link::{Action, PageRequest, Rewriter, Rule, Worker, render_options, render_page};
use gd_resolve::TreeEntry;
use guidedoc::{Document, Fail, NodeId, NodeKind, Pass, Workspace};

fn page(project: &Project, docname: &str, renderer: &str, format: OutputFormat) -> Rendered {
    let view = project.view();
    let doc = project.id(docname);
    let request = PageRequest {
        doc,
        format,
        renderer,
        options: render_options(&view, doc, format),
        ..PageRequest::default()
    };
    render_page(&view, &mut Worker::new(), &request)
}

fn html(project: &Project, docname: &str) -> String {
    let rendered = page(project, docname, "html", OutputFormat::Html);
    assert!(!rendered.failed, "{:?}", rendered.reports);
    rendered.body
}

/// A rule that drops emphasis-free paragraphs saying "drop", unwraps `only`, and
/// replaces headings by a paragraph; everything else is copied.
struct Sample;

impl Rule for Sample {
    fn visits(&self, doc: Document<'_>, id: NodeId) -> bool {
        matches!(doc.kind(id), NodeKind::Paragraph | NodeKind::Only | NodeKind::Heading)
    }

    fn decide(&mut self, doc: Document<'_>, id: NodeId) -> Action {
        match doc.kind(id) {
            NodeKind::Only => Action::Unwrap,
            NodeKind::Heading => Action::Replace,
            NodeKind::Paragraph if doc.plain_text_string(id) == "drop" => Action::Drop,
            _ => Action::Copy,
        }
    }

    fn replace(&mut self, rw: &mut Rewriter<'_, '_, '_>, id: NodeId) -> Result<(), Fail> {
        let span = rw.doc.node(id).span;
        let heading = rw.open(NodeKind::Heading, span)?;
        rw.b.set_value(heading, 1);
        rw.add_plain("Replaced", span)?;
        rw.close(heading)
    }
}

trait PlainText {
    fn plain_text_string(&self, id: NodeId) -> String;
}

impl PlainText for Document<'_> {
    fn plain_text_string(&self, id: NodeId) -> String {
        let mut text = String::new();
        self.plain_text(id, &mut text);
        text
    }
}

/// Reads `text` with the test reader and runs `rule` over it; returns the tree dump.
fn rewritten(text: &str, rule: Option<&mut dyn Rule>) -> String {
    let mut ws = Workspace::new();
    let request = guidedoc::Request {
        source: guidedoc::Source { name: "t.rst", text },
        ..guidedoc::Request::default()
    };
    guidedoc::read_document(&mut ws, &common::READER, &request).unwrap();
    if let Some(rule) = rule {
        let rule = std::cell::RefCell::new(rule);
        let run = |doc: Document<'_>, b: &mut guidedoc::Builder<'_>| {
            gd_link::rewrite_document(doc, b, *rule.borrow_mut())
        };
        let limits = guidedoc::Limits::default();
        guidedoc::transform_document(&mut ws, &[Pass { name: "t", run: &run }], &limits).unwrap();
    }
    guidedoc::dump_to_string(ws.document().unwrap())
}

#[test]
fn the_rewriter_copies_what_its_rule_ignores_whole() {
    let text = "Title\n=====\n\nkeep\n\ndrop\n\nonly html: inside\n\n:a: b\n";
    let original = rewritten(text, None);
    struct Nothing;
    impl Rule for Nothing {
        fn visits(&self, _: Document<'_>, _: NodeId) -> bool {
            false
        }
        fn replace(&mut self, _: &mut Rewriter<'_, '_, '_>, _: NodeId) -> Result<(), Fail> {
            Ok(())
        }
    }
    assert_eq!(rewritten(text, Some(&mut Nothing)), original);
    let changed = rewritten(text, Some(&mut Sample));
    assert!(changed.contains("Replaced"), "{changed}");
    assert!(!changed.contains("\"drop\""), "{changed}");
    assert!(!changed.contains("only"), "{changed}");
    assert!(changed.contains("inside"));
    assert!(changed.contains("keep"));
}

#[test]
fn only_keeps_content_for_its_output() {
    let mut project =
        Project::new("only", &[("index", "only html: for pages\n\nonly latex: for books\n")]);
    let body = html(&project, "index");
    assert!(body.contains("for pages"));
    assert!(!body.contains("for books"));
    project.kind = gd_env::BuilderKind::Text;
    let text = page(&project, "index", "text", OutputFormat::Text).body;
    assert!(!text.contains("for pages"));
}

#[test]
fn toctrees_become_navigation() {
    let mut project = Project::new(
        "toctree",
        &[("index", "Home\n====\n\ntoctree:\n"), ("guide", "Guide\n=====\n\nText.\n")],
    );
    let index = project.id("index").index();
    let guide = project.id("guide");
    project.env.docs[index]
        .toctrees
        .push(ToctreeInfo { caption: "Contents".into(), ..ToctreeInfo::default() });
    project.env.docs[guide.index()].sections.push(SectionInfo {
        level: 1,
        title: "Guide".into(),
        anchor: "guide".into(),
        ..SectionInfo::default()
    });
    project.index_tables();
    project.tables.trees[index][0].push(TreeEntry { doc: Some(guide), ..TreeEntry::default() });
    let body = html(&project, "index");
    assert!(body.contains("toctree-wrapper compound"), "{body}");
    assert!(body.contains("Contents"));
    assert!(body.contains("href=\"guide.html\""), "{body}");
    assert!(body.contains(">Guide</a>"), "{body}");
    // The book has no in-page navigation.
    let book = page(&project, "index", "typst", OutputFormat::Pdf).body;
    assert!(!book.contains("Contents"));
}

#[test]
fn references_become_links_or_warnings() {
    let project = Project::new(
        "xref",
        &[("index", "xref doc guide\n\nxref doc missing\n"), ("guide", "Guide\n=====\n")],
    );
    let rendered = page(&project, "index", "html", OutputFormat::Html);
    assert!(!rendered.failed, "{:?}", rendered.reports);
    assert!(rendered.body.contains("href=\"guide.html\""), "{}", rendered.body);
    assert!(rendered.body.contains(">Guide</a>"), "{}", rendered.body);
    let warning = rendered.reports.iter().find(|r| r.message.contains("missing"));
    let warning = warning.unwrap_or_else(|| panic!("no warning: {:?}", rendered.reports));
    assert_eq!(warning.line, 3);
    assert!(!warning.hints.is_empty());
    assert!(rendered.trace.len() >= 2, "the lookups are noted");
}

#[test]
fn todolists_collect_every_todo() {
    let mut project = Project::new(
        "todo",
        &[("index", "todolist\n"), ("notes", "Notes\n=====\n\ntodo: write more\n")],
    );
    project.config.todo_include_todos = true;
    let notes = project.id("notes").index();
    // The todo admonition is the fourth node: Document, Section, Heading, Text, ...
    project.env.docs[notes].todos.push(TodoInfo { node: 4, anchor: String::new(), line: 4 });
    let body = html(&project, "index");
    assert!(body.contains("write more"), "{body}");
    assert!(body.contains("todo-source"), "{body}");
    assert!(body.contains("notes.rst, line 4"), "{body}");
    project.config.todo_include_todos = false;
    let hidden = html(&project, "notes");
    assert!(!hidden.contains("write more"));
}

#[test]
fn graphs_are_drawn_as_images() {
    let project = Project::new("graph", &[("index", "graph: digraph { a -> b }\n")]);
    let rendered = page(&project, "index", "html", OutputFormat::Html);
    assert!(!rendered.failed, "{:?}", rendered.reports);
    if rendered.reports.iter().any(|r| r.title == "GRAPHVIZ DID NOT START") {
        return; // a machine without Graphviz's plugins: the graph stays code.
    }
    assert!(rendered.body.contains("<img"), "{}", rendered.body);
    assert!(rendered.body.contains("_images/graphviz-"), "{}", rendered.body);
    let asset = &rendered.assets[0];
    assert!(asset.target.starts_with("_images/graphviz-"));
    let svg = std::fs::read_to_string(&asset.source).unwrap();
    assert!(svg.contains("<svg"));
}
