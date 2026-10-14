use super::*;

fn item(name: &str, kind: &str, priority: i64, uri: &str, display: &str) -> Item {
    Item {
        name: name.into(),
        kind: kind.into(),
        priority,
        uri: uri.into(),
        display: display.into(),
    }
}

fn version_2(body: &str) -> Vec<u8> {
    let mut data = b"# Sphinx inventory version 2\n# Project: P\n# Version: 1\n# z\n".to_vec();
    data.extend(gd_flate::zlib_compress(body.as_bytes(), gd_flate::Level::Store));
    data
}

#[test]
fn round_trip_keeps_every_field() {
    let items = [
        item("len", "py:function", 1, "library/functions.html#len", "-"),
        item("tut-packages", "std:label", -1, "tutorial/modules.html#packages", "Packages"),
        item("with statement", "std:term", -1, "glossary.html#term-with-statement", "-"),
        item("os", "py:module", 0, "library/os.html#module-os", "-"),
    ];
    let data = write_inventory("Python", "3.15", &items);
    let inv = load_inventory(&data, "https://docs.python.org/3/").unwrap_or_default();
    assert_eq!((inv.project.as_str(), inv.version.as_str()), ("Python", "3.15"));
    let uri = |kind: &str, name: &str| inv.find(kind, name).map(|i| i.uri.clone());
    // The name at the end of the location was written as $, and comes back.
    assert_eq!(
        uri("py:function", "len").as_deref(),
        Some("https://docs.python.org/3/library/functions.html#len")
    );
    assert_eq!(
        uri("std:term", "with statement").as_deref(),
        Some("https://docs.python.org/3/glossary.html#term-with-statement")
    );
    let label = inv.find("std:label", "tut-packages").map(|i| i.display.as_str());
    assert_eq!(label, Some("Packages"));
    assert_eq!(inv.item_count(), 4);
    let text = String::from_utf8_lossy(&data);
    assert!(text.starts_with("# Sphinx inventory version 2\n# Project: Python\n"));
}

#[test]
fn lines_follow_sphinxs_pattern() {
    let body = "a name with spaces std:label -1 page.html#x The Title\n\
                os py:module 0 os.html#module-os -\nos py:module 0 wrong.html -\n\
                empty std:doc -1  Empty Page\nbad-line\nnotype label 1 x.html -\n";
    let inv = load_inventory(&version_2(body), "").unwrap_or_default();
    let spaced = inv.find("std:label", "a name with spaces");
    assert_eq!(spaced.map(|i| i.display.as_str()), Some("The Title"));
    assert_eq!(inv.find("py:module", "os").map(|i| i.uri.as_str()), Some("os.html#module-os"));
    let empty = inv.find("std:doc", "empty");
    assert_eq!(empty.map(|i| (i.uri.as_str(), i.display.as_str())), Some(("", "Empty Page")));
    assert!(!inv.has_type("label"));
}

#[test]
fn version_one_and_errors() {
    let v1 = b"# Sphinx inventory version 1\n# Project: Old\n# Version: 0.1\n\
               mod mod lib/mod.html\nmod.f function lib/mod.html\n";
    let inv = load_inventory(v1, "http://x/").unwrap_or_default();
    let uri = |kind: &str, name: &str| inv.find(kind, name).map(|i| i.uri.clone());
    assert_eq!(uri("py:module", "mod").as_deref(), Some("http://x/lib/mod.html#module-mod"));
    assert_eq!(uri("py:function", "mod.f").as_deref(), Some("http://x/lib/mod.html#mod.f"));
    let kind = |data: &[u8]| load_inventory(data, "").map(|_| ()).map_err(|e| e.kind);
    assert_eq!(kind(b"<html>not an inventory"), Err(ErrorKind::UnknownVersion));
    assert_eq!(kind(b"# Sphinx inventory version 2\n# Project: P\n"), Err(ErrorKind::Truncated));
    let mut damaged = version_2("x std:doc -1 x.html -\n");
    let last = damaged.len() - 1;
    damaged[last] ^= 0xff;
    assert_eq!(kind(&damaged), Err(ErrorKind::Compression(gd_flate::ErrorKind::Checksum)));
    let report = load_inventory(b"nope", "").map_err(|e| e.report("objects.inv"));
    let text = report.map_err(|r| gd_report::render_text(&r, &Default::default()));
    assert!(text.is_err_and(|t| t.starts_with("-- BAD INVENTORY [inventory.version]")));
}

#[test]
fn labels_and_terms_match_ignoring_case() {
    let items = [
        item("Tut-Packages", "std:label", -1, "a.html#x", "A"),
        item("tut-packages", "std:label", -1, "b.html#y", "B"),
        item("Len", "py:function", 1, "c.html#Len", ""),
    ];
    let inv = load_inventory(&write_inventory("P", "1", &items), "").unwrap_or_default();
    // The first spelling listed wins; other types match exactly only.
    let found = inv.find_folded("std:label", "TUT-PACKAGES");
    assert_eq!(found.map(|i| i.display.as_str()), Some("A"));
    assert!(inv.find_folded("py:function", "len").is_none());
    assert_eq!(inv.find("py:function", "Len").map(|i| i.display.as_str()), Some("-"));
}

#[test]
fn sorts_as_sphinx_writes() {
    let mut items = [
        item("b", "std:label", 0, "", ""),
        item("z", "py:function", 0, "", ""),
        item("a", "std:doc", 0, "", ""),
        item("a", "py:class", 0, "", ""),
    ];
    sort_items(&mut items);
    let order: Vec<&str> = items.iter().map(|i| i.kind.as_str()).collect();
    assert_eq!(order, ["py:class", "py:function", "std:doc", "std:label"]);
}
