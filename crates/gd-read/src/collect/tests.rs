//! The collector over documents the Sphinx reader reads.

use gd_env::{DocInfo, LabelKind, SourceKind};
use guidedoc::{Request, Workspace, default_request, read_document, read_extension};

use super::collect;

/// Reads `text` as reStructuredText with the Sphinx layer and collects it; returns the
/// document's info and its search lines.
fn collect_rst(text: &str) -> (DocInfo, String) {
    collect_with(guidedoc_sphinx::Config::default(), text)
}

#[test]
fn publication_fields_are_leading_and_keep_author_separators() {
    let (doc, _) = collect_rst(concat!(
        ":date: 2026-10-16\n:authors: Ada, Grace\n",
        ":tags: rust, publishing\n\nTitle\n=====\n\n:draft: true\n"
    ));
    assert_eq!(doc.page.authors, ["Ada", "Grace"]);
    assert_eq!(doc.page.tags, ["rust", "publishing"]);
    assert!(!doc.page.draft);
    assert!(doc.orphan);
}

fn collect_with(config: guidedoc_sphinx::Config, text: &str) -> (DocInfo, String) {
    let config = guidedoc_sphinx::Config { docname: "page".into(), ..config };
    let mut request: Request<'_> = default_request("page.rst", text, "sphinx", "");
    request.read.extension = Some(read_extension(&config));
    request.read.promote_title = false;
    let mut ws = Workspace::new();
    let doc = match read_document(&mut ws, &guidedoc_sphinx::reader(), &request) {
        Ok(doc) => doc,
        Err(fail) => panic!("read failed: {}", fail.diagnostic.formatted_message()),
    };
    let mut d = DocInfo { docname: "page".into(), kind: SourceKind::Rst, ..Default::default() };
    let words = collect(&mut d, doc);
    (d, words)
}

fn object<'d>(d: &'d DocInfo, fullname: &str) -> &'d gd_env::ObjectInfo {
    d.objects
        .iter()
        .find(|o| o.fullname == fullname)
        .unwrap_or_else(|| panic!("no object {fullname} in {:#?}", d.objects))
}

#[test]
fn sections_labels_and_the_title() {
    let text = ".. _intro:\n\nIntro *here*\n============\n\nText.\n\n\
                Sub\n---\n\n.. _fig:\n\n.. figure:: a.png\n\n   The caption.\n\n\
                See :ref:`intro`.\n";
    let (d, words) = collect_rst(text);
    assert_eq!(d.title, "Intro here");
    assert_eq!(d.sections.len(), 2);
    let (intro, sub) = (&d.sections[0], &d.sections[1]);
    assert_eq!((intro.level, intro.anchor.as_str(), intro.parent), (1, "intro-here", None));
    assert_eq!(intro.raw_title, "Intro *here*");
    assert_eq!(intro.ids, ["intro-here", "intro"]);
    assert_eq!((sub.parent, sub.anchor.as_str()), (Some(0), "sub"));
    let label = d.labels.iter().find(|l| l.name == "intro").unwrap();
    assert_eq!((label.anchor.as_str(), label.title.as_str()), ("intro", "Intro here"));
    assert_eq!(label.kind, LabelKind::Section);
    let fig = d.labels.iter().find(|l| l.name == "fig").unwrap();
    assert_eq!((fig.kind, fig.title.as_str()), (LabelKind::Figure, "The caption."));
    assert_eq!(d.numbered.len(), 1);
    assert_eq!(d.numbered[0].section, Some(1));
    assert_eq!(d.images, ["a.png"]);
    assert_eq!(d.refs[0].reftype, "ref");
    assert_eq!(d.refs[0].target, "intro");
    assert!(words.starts_with("intro-here\tIntro here\t"), "{words}");
    assert!(words.contains("sub\tSub\t"), "{words}");
}

#[test]
fn toctrees_metadata_and_only() {
    let text = ":orphan:\n:tocdepth: 2\n\nTitle\n=====\n\n.. toctree::\n   :maxdepth: 2\n   \
                :caption: Contents\n   :hidden:\n   :numbered:\n\n   guide\n   Other <other>\n\n\
                .. only:: html\n\n   Html\n   ----\n\n   .. only:: draft\n\n      Draft\n      \
                ~~~~~\n";
    let (d, _) = collect_rst(text);
    assert!(d.orphan);
    assert_eq!(d.tocdepth, 2);
    let t = &d.toctrees[0];
    assert_eq!((t.maxdepth, t.caption.as_str(), t.hidden, t.numbered), (2, "Contents", true, 999));
    assert_eq!((t.section, t.position, t.line), (Some(0), 1, 7));
    assert_eq!(t.entries.len(), 2);
    assert_eq!((t.entries[1].target.as_str(), t.entries[1].title.as_str()), ("other", "Other"));
    assert_eq!(d.sections[1].only, "(html)");
    assert_eq!(d.sections[2].only, "(html) and (draft)");
}

#[test]
fn python_objects_their_modules_and_aliases() {
    let text = ".. py:module:: pkg\n   :synopsis: The package.\n\n\
                .. py:class:: Thing\n   :canonical: pkg.core.Thing\n\n   A thing.\n\n   \
                .. py:method:: run()\n\n.. py:function:: f(x)\n              f(x, y)\n\n\
                .. py:function:: hidden()\n   :no-index:\n";
    let (d, _) = collect_rst(text);
    let module = &d.modules[0];
    assert_eq!((module.name.as_str(), module.domain.as_str()), ("pkg", "py"));
    assert_eq!(module.synopsis, "The package.");
    assert_eq!(object(&d, "pkg").objtype, "module");
    let thing = object(&d, "pkg.Thing");
    assert_eq!((thing.objtype.as_str(), thing.anchor.as_str()), ("class", "pkg.Thing"));
    assert_eq!(thing.line, 4);
    assert!(object(&d, "pkg.core.Thing").aliased);
    assert_eq!(object(&d, "pkg.Thing.run").objtype, "method");
    // An overload's second signature has no target and notes nothing.
    assert_eq!(d.objects.iter().filter(|o| o.fullname == "pkg.f").count(), 1);
    assert!(!d.objects.iter().any(|o| o.fullname.ends_with("hidden")));
    let names: Vec<&str> = d.toc_objects.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(names, ["Thing", "Thing.run()", "f()"]);
    assert_eq!(d.toc_objects[1].parent, Some(0));
    assert_eq!(d.toc_objects[0].anchor, "", "the first entry has no anchor");
}

#[test]
fn rust_objects_join_their_module_with_colons() {
    let text = ".. rust:crate:: demo\n   :synopsis: A demo.\n\n\
                .. rust:module:: demo::geometry\n\n\
                .. rust:struct:: pub struct Point\n\n   .. rust:field:: pub x: f32\n\n   \
                .. rust:method:: pub fn new(x: f32) -> Self\n\n\
                .. rust:function:: pub fn area(p: &Point) -> f32\n";
    let (d, _) = collect_rst(text);
    let point = object(&d, "demo::geometry::Point");
    assert_eq!((point.domain.as_str(), point.objtype.as_str()), ("rust", "struct"));
    assert_eq!(point.anchor, "rust-demo.geometry.Point");
    assert_eq!(object(&d, "demo::geometry::Point::x").objtype, "field");
    assert_eq!(object(&d, "demo::geometry::Point::new").objtype, "method");
    assert_eq!(object(&d, "demo::geometry::area").objtype, "function");
    for module in ["demo", "demo::geometry"] {
        let m = object(&d, module);
        assert_eq!((m.domain.as_str(), m.objtype.as_str()), ("rust", "module"));
    }
    assert_eq!(d.modules.len(), 2);
    assert!(d.modules.iter().all(|m| m.domain == "rust"));
}

#[test]
fn options_productions_terms_and_c_enumerators() {
    let text = ".. program:: rm\n\n.. option:: -r, --recursive\n\n   Recursive.\n\n\
                .. productionlist:: py\n   expr: `atom` | expr \"+\" atom\n\n\
                .. glossary::\n\n   Big  Word\n      A term.\n\n\
                .. c:enum:: color\n\n   .. c:enumerator:: red\n";
    let (d, _) = collect_rst(text);
    let short = object(&d, "rm -r");
    let long = object(&d, "rm --recursive");
    assert_eq!((short.domain.as_str(), short.objtype.as_str()), ("std", "cmdoption"));
    assert_ne!(short.anchor, long.anchor);
    assert_eq!(object(&d, "py:expr").objtype, "token");
    assert_eq!(d.terms[0].name, "big word");
    assert_eq!(object(&d, "color.red").objtype, "enumerator");
    assert_eq!(object(&d, "red").anchor, object(&d, "color.red").anchor);
}

#[test]
fn todos_meta_and_downloads() {
    let text = "Title\n=====\n\n.. todo:: Write more.\n\n.. todolist::\n\n\
                .. meta::\n   :description: A page.\n\n:download:`data <files/data.csv>`\n";
    let config = guidedoc_sphinx::Config { todo_include_todos: true, ..Default::default() };
    let (d, _) = collect_with(config, text);
    assert_eq!(d.todos.len(), 1);
    assert_eq!(d.todos[0].line as usize, text.find(".. todo::").unwrap(), "an offset here");
    assert!(d.todolist);
    assert_eq!(d.meta, [("description".to_string(), "A page.".to_string())]);
    assert_eq!(d.downloads, ["files/data.csv"]);
}

#[test]
fn names_are_normalized_as_sphinx_normalizes_them() {
    use gd_env::{normalize_name, option_key};
    assert_eq!(normalize_name("  Big \n Word "), "big word");
    assert_eq!(option_key("git  commit", "-m"), "git-commit -m");
    assert_eq!(option_key("", "-m"), "-m");
}
