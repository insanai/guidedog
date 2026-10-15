//! The project tables: toctrees, reading order, labels, numbering, the C++ tree.

use gd_env::{CppDeclInfo, LabelKind, NumberedInfo, TocEntry};

use super::*;
use crate::CppSymbolId;

fn project() -> Vec<DocInfo> {
    let mut index = doc("index", "Home");
    index.toctrees.push(toctree(&["guide/intro", "guide/*", "genindex", "https://x.org"]));
    index.toctrees[0].glob = true;
    let mut intro = doc("guide/intro", "Intro");
    intro.toctrees.push(toctree(&["usage", "/index", "missing", "instal"]));
    vec![
        index,
        intro,
        doc("guide/usage", "Usage"),
        doc("guide/zeta", "Zeta"),
        doc("install", "Install"),
        DocInfo { orphan: true, ..doc("lonely", "Lonely") },
    ]
}

#[test]
fn toctrees_resolve_names_globs_and_generated_pages() {
    let f = Fixture::new(project());
    let index = &f.tables.trees[f.id("index").index()][0];
    let docs: Vec<Option<DocId>> = index.iter().map(|e| e.doc).collect();
    let expected = [
        Some(f.id("guide/intro")),
        Some(f.id("guide/usage")),
        Some(f.id("guide/zeta")),
        None,
        None,
    ];
    assert_eq!(docs, expected);
    assert_eq!(
        (index[3].url.as_str(), index[3].title.as_str(), index[3].generated),
        ("genindex.html", "Index", true)
    );
    assert_eq!(index[4].url, "https://x.org");
    let names: Vec<&str> =
        f.tables.order.iter().map(|&id| f.env.doc(id).docname.as_str()).collect();
    assert_eq!(names, ["index", "guide/intro", "guide/usage", "guide/zeta"]);
    assert_eq!(f.tables.parent[f.id("guide/usage").index()], Some(f.id("guide/intro")));
    assert_eq!(f.tables.parent[f.id("index").index()], None);
}

#[test]
fn toctree_problems_are_warned_about_with_hints() {
    let f = Fixture::new(project());
    let codes = f.codes();
    // "/index" leads back to the root; "missing" and "instal" name nothing; install is
    // reached by no toctree, lonely is an orphan.
    assert!(codes.contains(&"build.toctree.cycle"), "{codes:?}");
    assert_eq!(codes.iter().filter(|c| **c == "build.toctree.missing").count(), 2);
    let typo = f.reports.iter().find(|r| r.message.contains("instal,")).unwrap();
    assert!(typo.hints[0].contains("Did you mean install?"), "{:?}", typo.hints);
    let unreached: Vec<&str> = f
        .reports
        .iter()
        .filter(|r| r.code == "build.toctree.orphan")
        .map(|r| r.path.as_str())
        .collect();
    assert_eq!(unreached, ["install.rst"]);
}

#[test]
fn a_missing_root_document_is_an_error() {
    let f = Fixture::new(vec![doc("other", "Other")]);
    let root = f.reports.iter().find(|r| r.code == "build.root.missing").unwrap();
    assert_eq!(root.severity, gd_report::Severity::Error);
}

#[test]
fn labels_keep_the_later_titled_one_and_warn() {
    let mut a = doc("index", "Home");
    a.toctrees.push(toctree(&["b"]));
    a.labels.push(label("intro", "intro", "Intro A", LabelKind::Section));
    let mut b = doc("b", "B");
    b.labels.push(label("intro", "intro-b", "Intro B", LabelKind::Section));
    b.labels.push(label("blank", "blank", "", LabelKind::Other));
    let f = Fixture::new(vec![a, b]);
    assert_eq!(f.tables.labels["intro"].anchor, "intro");
    assert!(f.codes().contains(&"build.label.duplicate"));
    assert_eq!(f.tables.keys, ["b", "index"]);
}

#[test]
fn autosectionlabel_names_sections_by_title() {
    let mut config = Config::new("/p", "/p/_build");
    config.extensions.push("sphinx.ext.autosectionlabel".into());
    config.autosectionlabel_prefix_document = true;
    let mut d = doc("index", "Home Page");
    d.sections.push(section("Getting  Started", "getting-started", Some(0)));
    let f = Fixture::with_config(config, vec![d]);
    assert_eq!(f.tables.labels["index:getting started"].anchor, "getting-started");
    assert_eq!(f.tables.labels["index:home page"].kind, LabelKind::Section);
}

#[test]
fn numbered_toctrees_number_sections_and_figures() {
    let mut config = Config::new("/p", "/p/_build");
    config.numfig = true;
    config.numfig_secnum_depth = 1;
    let mut index = doc("index", "Home");
    index.toctrees.push(ToctreeInfo { numbered: 2, ..toctree(&["a", "b"]) });
    let mut a = doc("a", "A");
    a.sections.push(section("A one", "a-one", Some(0)));
    a.numbered.push(NumberedInfo {
        kind: LabelKind::Figure,
        anchor: "fig".into(),
        ids: vec!["fig".into()],
        section: Some(1),
        toctrees: 0,
    });
    let mut b = doc("b", "B");
    b.numbered.push(NumberedInfo {
        kind: LabelKind::Figure,
        anchor: "fig-b".into(),
        ids: vec!["fig-b".into()],
        section: Some(0),
        toctrees: 0,
    });
    let f = Fixture::with_config(config, vec![index, a, b]);
    let a = f.id("a");
    assert_eq!(f.tables.secnumbers[a.index()], ["1", "1.1"]);
    assert_eq!(f.tables.secnumbers[f.id("b").index()], ["2"]);
    assert_eq!(f.tables.fignumbers[a.index()], ["1.1"]);
    assert_eq!(f.tables.fignumbers[f.id("b").index()], ["2.1"]);
    assert_eq!(f.view().item_number(a, "fig"), "1.1");
    assert_eq!(f.view().section_number(a, "a-one"), "1.1");
}

#[test]
fn equations_count_per_document_without_math_numfig() {
    let mut d = doc("index", "Home");
    for anchor in ["e1", "e2"] {
        d.numbered.push(NumberedInfo {
            kind: LabelKind::Equation,
            anchor: anchor.into(),
            ids: vec![anchor.into()],
            ..NumberedInfo::default()
        });
    }
    let f = Fixture::new(vec![d]);
    assert_eq!(f.tables.fignumbers[0], ["1", "2"]);
}

#[test]
fn the_api_index_joins_the_root_toctree() {
    let mut config = Config::new("/p", "/p/_build");
    config.rust_api.add_toctree_entry = true;
    config.rust_api.root = "rustapi".into();
    let mut index = doc("index", "Home");
    index.toctrees.push(toctree(&[]));
    let api = DocInfo { generated: true, ..doc("rustapi/index", "API") };
    let f = Fixture::with_config(config, vec![index, api]);
    let first = &f.tables.trees[f.id("index").index()][0];
    assert_eq!(first[0].doc, Some(f.id("rustapi/index")));
}

fn cpp_element(ident: &str, id4: &str, decl: &str) -> String {
    let fields = [ident, "", "", ident, "\x1e", "\x1e", "\x1e", id4, decl];
    fields.join("\x1f")
}

fn cpp_decl(path: &[String], objtype: &str, id: &str, group: u32) -> CppDeclInfo {
    CppDeclInfo {
        path: path.join("\n"),
        objtype: objtype.into(),
        directive: objtype.into(),
        id: id.into(),
        group,
        ..CppDeclInfo::default()
    }
}

#[test]
fn cpp_declarations_build_a_tree_and_resolve() {
    let ns = cpp_element("ns", "2ns", "");
    let class = cpp_element("Widget", "6Widget", "_CPPv4N2ns6WidgetE");
    let mut index = doc("index", "Home");
    index.cpp.push(cpp_decl(&[ns.clone(), class.clone()], "class", "_CPPv4N2ns6WidgetE", 0));
    let mut other = DocInfo { orphan: true, ..doc("other", "Other") };
    other.cpp.push(cpp_decl(&[ns.clone(), class.clone()], "class", "_CPPv4N2ns6WidgetE", 0));
    let f = Fixture::new(vec![index, other]);
    let t = &f.tables.cpp;
    let widget = t.find_path(&[ns.clone(), class].join("\n")).unwrap();
    assert_eq!(t.full_name(widget, false), "ns::Widget");
    assert_eq!(t.symbol(widget).declaration.as_ref().unwrap().doc, f.id("index"));
    assert!(f.codes().contains(&"build.cpp.duplicate"));
    assert_eq!(t.find_path(""), Some(CppSymbolId(0)));
    let (found, trace) = f.resolve("index", "cpp:class", "Widget", |q| {
        q.context.push(("cpp:name".into(), "Widget\x1f".into()));
        q.context.push(("cpp:parent".into(), ns.clone()));
    });
    let found = found.unwrap();
    assert_eq!(found.anchor, "_CPPv4N2ns6WidgetE");
    assert!(found.warning.is_none());
    assert_eq!(trace.lookups()[0].kind, gd_env::LookupKind::Cpp);
    let (wrong, _) = f.resolve("index", "cpp:func", "ns::Widget", |q| {
        q.context.push(("cpp:name".into(), "ns\x1f\nWidget\x1f".into()));
    });
    assert_eq!(wrong.unwrap().warning.unwrap().code, "build.cpp.role");
    let (std, _) = f.resolve("index", "cpp:class", "std::vector", |q| {
        q.context.push(("cpp:name".into(), "std\x1f\nvector\x1f".into()));
    });
    assert!(std.unwrap().no_link);
}

#[test]
fn a_toctree_title_and_unknown_entry_are_kept_apart() {
    let mut index = doc("index", "Home");
    index.toctrees.push(ToctreeInfo {
        entries: vec![TocEntry { target: "self".into(), title: "Me".into() }],
        ..toctree(&[])
    });
    let f = Fixture::new(vec![index]);
    let me = &f.tables.trees[0][0][0];
    assert_eq!((me.doc, me.title.as_str()), (Some(f.id("index")), "Me"));
}
