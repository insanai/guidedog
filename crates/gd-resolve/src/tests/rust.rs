//! Rust references: full `::` paths, the current module first, `crate::`,
//! `self::` and `super::`, suffix search, inventories, and objects.inv priorities.

use gd_env::LookupKind;
use gd_inventory::{Inventory, Item};

use super::*;
use crate::rust_xref::candidate_paths;

fn project() -> Vec<DocInfo> {
    let mut index = doc("index", "Home");
    index.toctrees.push(toctree(&["api"]));
    let mut api = doc("api", "API");
    api.objects.push(object("rust", "crate", "kit"));
    api.objects.push(object("rust", "module", "kit::parser"));
    api.objects.push(object("rust", "struct", "kit::parser::Parser"));
    api.objects.push(object("rust", "method", "kit::parser::Parser::new"));
    api.objects.push(object("rust", "function", "kit::parse"));
    api.objects.push(object("rust", "enum", "kit::Kind"));
    api.objects.push(object("rust", "variant", "kit::Kind::Word"));
    vec![index, api]
}

fn in_module(f: &Fixture, reftype: &str, target: &str, module: &str) -> Option<String> {
    let (r, _) = f.resolve("index", reftype, target, |q| q.module = module.into());
    r.ok().map(|r| r.anchor)
}

#[test]
fn rust_paths_resolve_from_the_current_module_outward() {
    let f = Fixture::new(project());
    let parser = "rust-kit--parser--Parser";
    assert_eq!(in_module(&f, "rust:struct", "kit::parser::Parser", "").as_deref(), Some(parser));
    assert_eq!(in_module(&f, "rust:struct", "Parser", "kit::parser").as_deref(), Some(parser));
    assert_eq!(in_module(&f, "rust:struct", "parser::Parser", "kit").as_deref(), Some(parser));
    // An enclosing module's items are found from a submodule.
    assert_eq!(
        in_module(&f, "rust:fn", "parse", "kit::parser").as_deref(),
        Some("rust-kit--parse")
    );
    assert_eq!(
        in_module(&f, "rust:fn", "super::parse", "kit::parser").as_deref(),
        Some("rust-kit--parse")
    );
    assert_eq!(
        in_module(&f, "rust:fn", "crate::parse", "kit::parser").as_deref(),
        Some("rust-kit--parse")
    );
    assert_eq!(
        in_module(&f, "rust:struct", "self::Parser", "kit::parser").as_deref(),
        Some(parser)
    );
    assert_eq!(in_module(&f, "rust:struct", "~kit::parser::Parser", "").as_deref(), Some(parser));
    assert_eq!(in_module(&f, "rust:mod", "kit", "").as_deref(), Some("rust-kit"));
    assert!(in_module(&f, "rust:struct", "Parser", "").is_none());
}

#[test]
fn a_leading_dot_and_rust_any_search_by_suffix() {
    let f = Fixture::new(project());
    let (r, trace) = f.resolve("index", "rust:fn", ".Parser::new", |_| {});
    assert_eq!(r.unwrap().anchor, "rust-kit--parser--Parser--new");
    let last = trace.lookups().last().unwrap();
    assert_eq!(last.kind, LookupKind::Suffix);
    assert_eq!(last.suffix_parts(), Some(("rust:", "::Parser::new", "rust:fn")));
    // The suffix search keeps to the types the role names.
    assert!(f.resolve("index", "rust:struct", ".new", |_| {}).0.is_err());
    assert_eq!(in_module(&f, "rust:any", "Word", "").as_deref(), Some("rust-kit--Kind--Word"));
    assert_eq!(in_module(&f, "any", "kit::Kind", "").as_deref(), Some("rust-kit--Kind"));
}

#[test]
fn unresolved_rust_references_suggest_a_path() {
    let mut f = Fixture::new(project());
    f.options.nitpicky = true;
    let (r, _) = f.resolve("index", "rust:struct", "Parsr", |q| q.module = "kit::parser".into());
    let why = r.unwrap_err();
    assert_eq!(why.suggestion.as_deref(), Some("Parser"));
    assert!(why.hint.contains("rust:module"));
    assert_eq!(why.category, "ref.struct");
}

#[test]
fn rust_any_falls_back_to_the_inventories() {
    let mut f = Fixture::new(project());
    let mut std = Inventory::new("Rust", "1.90");
    std.add(Item {
        name: "std::vec::Vec".into(),
        kind: "rust:struct".into(),
        priority: 1,
        uri: "https://doc.rust/std/vec/struct.Vec.html".into(),
        display: "-".into(),
    });
    f.intersphinx.main = std.clone();
    f.intersphinx.named.insert("std".into(), std);
    f.config.extensions.push("sphinx.ext.intersphinx".into());
    let (r, _) = f.resolve("api", "rust:any", "std::vec::Vec", |_| {});
    let r = r.unwrap();
    assert_eq!(r.url, "https://doc.rust/std/vec/struct.Vec.html");
    assert_eq!(r.external, "(in Rust v1.90)");
    assert!(r.keep_content);
    let (r, _) = f.resolve("api", "rust:struct", "Vec", |q| q.module = "std::vec".into());
    assert_eq!(r.unwrap().external, "(in Rust v1.90)");
}

#[test]
fn rust_roles_name_their_types() {
    use crate::role_takes;
    assert!(role_takes("rust:fn", "method") && role_takes("rust:fn", "function"));
    assert!(role_takes("rust:type", "struct") && role_takes("rust:type", "associatedtype"));
    assert!(role_takes("rust:mod", "crate") && !role_takes("rust:struct", "enum"));
    assert!(role_takes("rust:any", "macro") && role_takes("rust:const", "associatedconst"));
}

#[test]
fn relative_paths_read_as_in_rust() {
    assert_eq!(candidate_paths("a::b", "C"), ["a::b::C", "a::C", "C"]);
    assert_eq!(candidate_paths("a::b", "super::super::C"), ["C"]);
    assert_eq!(candidate_paths("", "crate::C"), ["C"]);
    assert_eq!(candidate_paths("a::b", "self::C"), ["a::b::C"]);
}

#[test]
fn the_inventory_ranks_rust_modules_first() {
    let f = Fixture::new(project());
    let mut bytes = Vec::new();
    crate::write_inventory(&f.view(), &mut bytes);
    let inv = gd_inventory::load_inventory(&bytes, "").unwrap();
    assert_eq!(inv.find("rust:module", "kit::parser").unwrap().priority, 0);
    assert_eq!(inv.find("rust:crate", "kit").unwrap().priority, 0);
    let parser = inv.find("rust:struct", "kit::parser::Parser").unwrap();
    assert_eq!((parser.priority, parser.uri.as_str()), (1, "api.html#rust-kit--parser--Parser"));
}

/// A field and a method of one name live in different Rust namespaces: describing both
/// is no duplicate. A second method of that name is.
#[test]
fn a_field_and_a_method_of_one_name_are_not_duplicates() {
    let mut docs = project();
    let mut field = object("rust", "field", "kit::parser::Parser::new");
    field.anchor = "rust-field-new".into();
    docs[1].objects.insert(4, field);
    let f = Fixture::new(docs.clone());
    assert!(!f.codes().contains(&"build.object.duplicate"), "{:?}", f.codes());
    let mut again = object("rust", "method", "kit::parser::Parser::new");
    again.anchor = "rust-again".into();
    docs[1].objects.push(again);
    let f = Fixture::new(docs);
    assert!(f.codes().contains(&"build.object.duplicate"), "{:?}", f.codes());
}
