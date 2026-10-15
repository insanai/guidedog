//! Other projects' inventories: loading (local, fetched, cached, untrusted) and
//! resolving through them.

use gd_conf::IntersphinxProject;
use gd_inventory::{Inventory, Item};

use super::*;
use crate::{IntersphinxOptions, load_intersphinx};

fn item(name: &str, kind: &str, uri: &str, display: &str) -> Item {
    Item {
        name: name.into(),
        kind: kind.into(),
        priority: 1,
        uri: uri.into(),
        display: display.into(),
    }
}

fn python() -> Inventory {
    let mut inv = Inventory::new("Python", "3.12");
    inv.add(item(
        "json.dumps",
        "py:function",
        "https://docs.python.org/3/library/json.html#json.dumps",
        "-",
    ));
    inv.add(item(
        "json.JSONEncoder",
        "py:class",
        "library/json.html#json.JSONEncoder",
        "JSONEncoder",
    ));
    inv.add(item(
        "tutorial/index",
        "std:doc",
        "https://docs.python.org/3/tutorial/",
        "The Tutorial",
    ));
    inv.add(item("Debugging", "std:label", "https://docs.python.org/3/debug.html", "Debugging"));
    inv
}

fn fixture() -> Fixture {
    let mut index = doc("index", "Home");
    index.toctrees.push(toctree(&["guide/page"]));
    let mut f = Fixture::new(vec![index, doc("guide/page", "Page")]);
    f.intersphinx.main = python();
    f.intersphinx.named.insert("python".into(), python());
    f
}

#[test]
fn unresolved_references_try_the_inventories() {
    let f = fixture();
    let (r, _) = f.resolve("index", "py:func", "json.dumps", |_| {});
    let r = r.unwrap();
    assert_eq!(r.external, "(in Python v3.12)");
    assert!(r.keep_content && r.doc.is_none());
    // Qualified by the current module, and relative locations from a nested page.
    let (r, _) = f.resolve("guide/page", "py:class", "JSONEncoder", |q| q.module = "json".into());
    let r = r.unwrap();
    assert_eq!(
        (r.url.as_str(), r.title.as_str()),
        ("../library/json.html#json.JSONEncoder", "JSONEncoder")
    );
    assert!(!r.keep_content);
    // Labels match ignoring case; std:doc is disabled unless the inventory is named.
    assert!(f.resolve("index", "ref", "debugging", |_| {}).0.is_ok());
    assert!(f.resolve("index", "doc", "tutorial/index", |_| {}).0.is_err());
    let (r, _) = f.resolve("index", "doc", "python:tutorial/index", |_| {});
    assert_eq!(r.unwrap().title, "The Tutorial");
    let (r, _) = f.resolve("index", "py:func", "json.dumps", |q| q.explicit = true);
    assert!(r.unwrap().keep_content);
}

#[test]
fn external_references_search_only_the_inventories() {
    let f = fixture();
    let (r, _) = f.resolve("index", "py:func", "json.dumps", |q| {
        q.external = true;
        q.inventory = "python".into();
    });
    assert!(r.is_ok());
    let (r, _) = f.resolve("index", "py:func", "json.dumps", |q| {
        q.external = true;
        q.inventory = "pyton".into();
    });
    let why = r.unwrap_err();
    assert_eq!(
        (why.title.as_str(), why.suggestion.as_deref()),
        ("UNKNOWN INVENTORY", Some("python"))
    );
    let (r, _) = f.resolve("index", "py:func", "json.nothing", |q| q.external = true);
    let why = r.unwrap_err();
    assert!(why.warn);
    assert_eq!((why.code.as_str(), why.category.as_str()), ("build.xref.external", "ref.func"));
}

fn config_with(dir: &TempDir, locations: Vec<String>, uri: &str) -> Config {
    let mut config = Config::new(dir.0.join("docs"), dir.0.join("_build"));
    config.extensions.push("sphinx.ext.intersphinx".into());
    config.intersphinx.mapping.push(IntersphinxProject {
        name: "python".into(),
        uri: uri.into(),
        locations,
    });
    config
}

fn inventory_bytes() -> Vec<u8> {
    let items: Vec<Item> = python().items().to_vec();
    gd_inventory::write_inventory("Python", "3.12", &items)
}

#[test]
fn local_inventories_load_and_untrusted_builds_stay_inside() {
    let dir = TempDir::new("local-inv");
    dir.put("docs/python.inv", &inventory_bytes());
    let outside = dir.put("elsewhere/python.inv", &inventory_bytes());
    let budget = Budget::new(64 << 20);
    let trusted = IntersphinxOptions { network: true, budget: &budget };
    let config = config_with(&dir, vec!["python.inv".into()], "https://docs.python.org/3/");
    let mut reports = Vec::new();
    let it = load_intersphinx(&config, trusted, &mut reports).unwrap();
    assert!(reports.is_empty(), "{reports:?}");
    assert!(it.main.find("py:function", "json.dumps").is_some());
    let encoder = it.named["python"].find("py:class", "json.JSONEncoder").unwrap();
    assert_eq!(encoder.uri, "https://docs.python.org/3/library/json.html#json.JSONEncoder");
    assert_eq!(it.origin(encoder).map(|i| i.project.as_str()), Some("Python"));
    assert_ne!(it.digest, gd_env::Digest::default());
    // An untrusted build reads no file outside the roots, and fetches nothing.
    let untrusted = IntersphinxOptions { network: false, budget: &budget };
    let path = outside.to_string_lossy().into_owned();
    let config = config_with(&dir, vec![path, String::new()], "https://docs.python.org/3/");
    let it = load_intersphinx(&config, untrusted, &mut reports).unwrap();
    assert!(it.named.is_empty());
    let warning = &reports[0];
    assert_eq!(warning.code, "build.intersphinx.fetch");
    assert!(warning.message.contains("outside the source folder"), "{}", warning.message);
    assert!(warning.message.contains("uses no network"), "{}", warning.message);
}

#[test]
fn nothing_loads_without_the_extension_and_a_tiny_budget_refuses() {
    let dir = TempDir::new("budget-inv");
    dir.put("docs/python.inv", &inventory_bytes());
    let mut config = config_with(&dir, vec!["python.inv".into()], "https://docs.python.org/3/");
    let budget = Budget::new(16);
    let options = IntersphinxOptions { network: true, budget: &budget };
    let mut reports = Vec::new();
    let problem = load_intersphinx(&config, options, &mut reports).unwrap_err();
    assert_eq!(problem.code, "host.budget");
    config.extensions.clear();
    assert!(load_intersphinx(&config, options, &mut reports).unwrap().named.is_empty());
}

#[test]
fn remote_inventories_are_fetched_then_cached() {
    let body = inventory_bytes();
    let server = gd_fetch::testserver::start(move |_| gd_fetch::testserver::Reply {
        status: 200,
        body: body.clone(),
        ..Default::default()
    })
    .unwrap();
    let dir = TempDir::new("remote-inv");
    let uri = server.url("/docs/");
    let config = config_with(&dir, vec![String::new()], &uri);
    let budget = Budget::new(64 << 20);
    let mut reports = Vec::new();
    let network = IntersphinxOptions { network: true, budget: &budget };
    let it = load_intersphinx(&config, network, &mut reports).unwrap();
    assert!(reports.is_empty(), "{reports:?}");
    let encoder = it.main.find("py:class", "json.JSONEncoder").unwrap();
    assert_eq!(encoder.uri, format!("{uri}library/json.html#json.JSONEncoder"));
    assert_eq!(server.served_count(), 1);
    // The cache serves the next build, and an untrusted one at any age.
    let again = load_intersphinx(&config, network, &mut reports).unwrap();
    assert_eq!(again.digest, it.digest);
    let untrusted = IntersphinxOptions { network: false, budget: &budget };
    assert!(
        load_intersphinx(&config, untrusted, &mut reports).unwrap().named.contains_key("python")
    );
    assert_eq!(server.served_count(), 1);
    assert!(reports.is_empty());
}
