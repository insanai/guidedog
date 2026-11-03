//! The dependency manifest: each page and book lists the files it was made from.

#![allow(clippy::unwrap_used)]

mod common;

use common::{assert_built, build, demo, load};
use gd_json::Value;
use gd_project::{BuilderKind, DEPENDENCIES_FILE};

/// The manifest of `out` (a build output folder) and its output called `unit`.
fn output(dir: &common::TempDir, out: &str, unit: &str) -> (Value, Value) {
    let text = dir.read(&format!("{out}/{DEPENDENCIES_FILE}"));
    let manifest = gd_json::parse(&text).unwrap();
    let outputs = manifest.get("outputs").and_then(Value::as_array).unwrap();
    let found = outputs.iter().find(|o| o.get("unit").and_then(Value::as_str) == Some(unit));
    let found = found.unwrap_or_else(|| panic!("no unit {unit} in {text}")).clone();
    (manifest, found)
}

fn texts(value: &Value, key: &str) -> Vec<String> {
    let list = value.get(key).and_then(Value::as_array).unwrap();
    list.iter().map(|v| v.as_str().unwrap().to_string()).collect()
}

#[test]
fn html_pages_list_their_documents_includes_and_images() {
    let dir = demo("deps-html");
    dir.put(
        "guide.rst",
        "Guide\n=====\n\n.. include:: parts/usage.rst\n\n.. literalinclude:: code.py\n\n\
         .. image:: images/logo.svg\n",
    );
    dir.put("parts/usage.rst", "Included text.\n");
    dir.put("code.py", "x = 1\n");
    dir.put("images/logo.svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
    let r = build(&load(&dir.0), BuilderKind::Html);
    assert_built(&r);
    let (manifest, guide) = output(&dir, "_build/html", "guide");
    assert_eq!(manifest.get("schema").and_then(Value::as_i64), Some(1));
    assert_eq!(texts(&manifest, "shared"), ["conf.toml"]);
    assert_eq!(texts(&guide, "files"), ["guide.html"]);
    let inputs = texts(&guide, "inputs");
    for want in ["guide.rst", "parts/usage.rst", "code.py", "images/logo.svg"] {
        assert!(inputs.iter().any(|i| i == want), "{want} in {inputs:?}");
    }
    assert!(!inputs.iter().any(|i| i == "notes.md"), "{inputs:?}");
    let (_, site) = output(&dir, "_build/html", "");
    assert!(texts(&site, "inputs").iter().any(|i| i == "notes.md"));
}

#[test]
fn a_book_lists_every_chapter() {
    let dir = demo("deps-pdf");
    dir.put(
        "conf.toml",
        "project = \"Demo\"\nauthor = \"Ada\"\nsource_suffix = [\".rst\", \".md\"]\n\
         extensions = [\"myst_parser\"]\npdf_fonts = \"embedded\"\npdf_packages = \"offline\"\n",
    );
    let project = load(&dir.0);
    assert_built(&build(&project, BuilderKind::Pdf));
    let (_, book) = output(&dir, "_build/pdf", "book:demo.pdf");
    assert_eq!(texts(&book, "documents"), ["guide.rst", "index.rst", "notes.md"]);
    assert!(texts(&book, "files").contains(&"demo-en.pdf".to_string()));
    // A book kept by the next build keeps its record.
    let first = dir.read("_build/pdf/gd-dependencies.json");
    assert_built(&build(&project, BuilderKind::Pdf));
    assert_eq!(dir.read("_build/pdf/gd-dependencies.json"), first);
}
