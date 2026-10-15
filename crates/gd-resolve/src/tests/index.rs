//! The general index, the module index, and objects.inv.

use gd_env::{IndexInfo, LabelKind, ModuleInfo};

use super::*;
use crate::{create_index, has_module_index, module_index};

fn entry(entries: &str, anchor: &str) -> IndexInfo {
    IndexInfo { entries: entries.into(), anchor: anchor.into(), ..IndexInfo::default() }
}

#[test]
fn index_entries_group_sort_and_fold() {
    let mut a = doc("index", "Home");
    a.toctrees.push(toctree(&["b"]));
    a.index.push(entry("single: beta; sub\npair: alpha; omega\n!zulu, _under", "ix-1"));
    a.index.push(entry("see: gamma; beta\nmodule: os\n#hash", "ix-2"));
    let mut b = doc("b", "B");
    b.index.push(entry("single: f() (in module b)\nsingle: f() (in module a)", "ix-3"));
    let f = Fixture::new(vec![a, b]);
    let groups = create_index(&f.view());
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Symbols", "_", "A", "B", "F", "G", "M", "O", "Z"]);
    let beta = &groups[3].terms[0];
    assert_eq!((beta.name.as_str(), beta.subs[0].name.as_str()), ("beta", "sub"));
    assert_eq!(beta.subs[0].links[0].uri, "index.html#ix-1");
    let fs = &groups[4].terms;
    assert_eq!(fs.len(), 1, "{fs:?}");
    assert_eq!(fs[0].name, "f() (in module a)");
    assert_eq!(fs[0].subs[0].name, "(in module b)");
    let gamma = &groups[5].terms[0];
    assert_eq!(gamma.subs[0].name, "see beta");
    assert!(gamma.subs[0].links.is_empty());
    let zulu = &groups[8].terms[0];
    assert!(zulu.links[0].main);
    let os = &groups[7].terms[1];
    assert_eq!((os.name.as_str(), os.subs[0].name.as_str()), ("os", "module"));
}

fn module(name: &str) -> ModuleInfo {
    ModuleInfo {
        name: name.into(),
        domain: "py".into(),
        anchor: format!("module-{name}"),
        ..ModuleInfo::default()
    }
}

#[test]
fn the_module_index_groups_packages() {
    let mut config = Config::new("/p", "/p/_build");
    config.html.modindex_common_prefix.push("pkg.".into());
    let mut d = doc("index", "Home");
    for name in ["zlib", "pkg.io", "pkg.io.files", "Array", "email.mime"] {
        d.modules.push(module(name));
    }
    let f = Fixture::with_config(config, vec![d]);
    assert!(has_module_index(&f.view()));
    let letters = module_index(&f.view());
    let rows: Vec<(String, u8)> =
        letters.iter().flat_map(|l| l.entries.iter().map(|e| (e.name.clone(), e.group))).collect();
    let expected = [
        ("Array", 0),
        ("email", 1),
        ("email.mime", 2),
        ("pkg.io", 1),
        ("pkg.io.files", 2),
        ("zlib", 0),
    ];
    let expected: Vec<(String, u8)> = expected.iter().map(|(n, g)| (n.to_string(), *g)).collect();
    assert_eq!(rows, expected);
    assert_eq!(
        letters.iter().map(|l| l.letter.as_str()).collect::<Vec<_>>(),
        ["a", "e", "i", "z"]
    );
    assert_eq!(letters[1].entries[0].doc, None);
}

#[test]
fn objects_inv_lists_documents_objects_labels_and_terms() {
    let mut d = doc("index", "Home");
    d.labels.push(label("intro", "intro", "Intro", LabelKind::Section));
    d.labels.push(label("eq", "eq", "", LabelKind::Equation));
    d.objects.push(object("py", "module", "pkg"));
    d.objects.push(object("py", "function", "pkg.f"));
    d.objects.push(object("std", "cmdoption", "tool -v"));
    d.objects.push(object("c", "member", "S.@anon.x"));
    d.terms.push(gd_env::TermInfo { name: "thing".into(), anchor: "term-thing".into() });
    let f = Fixture::new(vec![d]);
    let mut bytes = Vec::new();
    crate::write_inventory(&f.view(), &mut bytes);
    let inv = gd_inventory::load_inventory(&bytes, "").unwrap();
    assert_eq!(inv.project, "Python");
    assert_eq!(inv.find("py:module", "pkg").unwrap().priority, 0);
    assert_eq!(inv.find("py:function", "pkg.f").unwrap().priority, 1);
    assert!(inv.find("std:cmdoption", "tool.-v").is_some());
    assert_eq!(inv.find("std:doc", "index").unwrap().display, "Home");
    assert_eq!(inv.find("std:label", "intro").unwrap().uri, "index.html#intro");
    assert!(inv.find("std:label", "eq").is_none());
    assert!(inv.find("std:label", "genindex").is_some());
    assert_eq!(inv.find("std:term", "thing").unwrap().priority, -1);
    assert_eq!(inv.find("c:member", "S.@anon.x").unwrap().display, "S.[anonymous].x");
}
