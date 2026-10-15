//! Projects and Sphinx inventories (objects.inv): intersphinx resolves through other
//! projects' inventories, and canonical names are aliases in the project's own.

use gd_inventory::{Item, load_inventory, write_inventory};
use guidedog_tests::{has, lacks};

use crate::support::{build, project};

/// An inventory item.
fn item(name: &str, kind: &str, priority: i64, uri: &str, display: &str) -> Item {
    Item {
        name: name.into(),
        kind: kind.into(),
        priority,
        uri: uri.into(),
        display: display.into(),
    }
}

/// intersphinx resolves references as Sphinx does: by role, with a name: prefix,
/// :external: roles, std:doc disabled unless named, labels and terms ignoring case, and
/// links to a local project relative to each page.
#[test]
fn intersphinx_resolves_through_inventories() {
    let dir = project("intersphinx");
    let items = [
        item("pkg.run", "py:function", 1, "api.html#pkg.run", "-"),
        item("setup", "std:label", -1, "setup.html#setup", "Set up"),
        item("guide", "std:doc", -1, "guide.html", "The Guide"),
        item("Widget", "std:term", -1, "g.html#term-widget", "-"),
    ];
    std::fs::write(dir.join("other.inv"), write_inventory("Other", "2.0", &items)).unwrap();
    std::fs::write(dir.join("local.inv"), write_inventory("Local", "", &items)).unwrap();
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   sub/page\n\n:func:`pkg.run` :ref:`setup` \
         :doc:`other:guide` :doc:`guide` :term:`widget` :external+other:ref:`setup`\n\n\
         .. py:currentmodule:: pkg\n\n:func:`run`\n",
    );
    dir.write("sub/page.rst", "Page\n====\n\n:ref:`local:setup`\n");
    dir.write(
        "conf.toml",
        "project = \"Demo\"\nextensions = [\"sphinx.ext.intersphinx\"]\n\n\
         [intersphinx_mapping]\nother = [\"https://example.org/other/\", \"other.inv\"]\n\
         local = [\"../localdocs\", \"local.inv\"]\n",
    );
    let r = build(&dir.path, 0, &["html"]);
    let page = dir.read("_build/html/index.html");
    has(
        &page,
        "<a href=\"https://example.org/other/api.html#pkg.run\" title=\"(in Other v2.0)\">\
         <code>pkg.run()</code></a>",
    );
    has(
        &page,
        "href=\"https://example.org/other/setup.html#setup\" title=\"(in Other v2.0)\">Set up</a>",
    );
    has(
        &page,
        "href=\"https://example.org/other/guide.html\" title=\"(in Other v2.0)\">The Guide</a>",
    );
    has(&page, "href=\"https://example.org/other/g.html#term-widget\"");
    has(&page, "<code>run()</code></a>");
    // std:doc is disabled unless the inventory is named.
    has(&r.err, "The :doc: reference to `guide` has no target in this project.");
    // A local base makes links relative to the page.
    has(
        &dir.read("_build/html/sub/page.html"),
        "href=\"../../localdocs/setup.html#setup\" title=\"(in Local)\">Set up</a>",
    );
    assert!(!dir.exists("_build/.doctrees/__intersphinx_cache__"));
}

/// A Python object's :canonical: name is an alias of it, as in Sphinx's
/// test_domain_py_canonical.py: references to either name reach the object, a definition
/// takes the name from an alias, and objects.inv lists the alias with priority -1.
#[test]
fn canonical_names_are_aliases() {
    let dir = project("canonical");
    dir.write(
        "index.rst",
        "Demo\n====\n\n:py:class:`.Foo` :any:`original.module.Foo` \
         :py:class:`original.module.Foo`\n\n.. py:module:: canonical\n\n.. py:class:: Foo\n   \
         :canonical: original.module.Foo\n\n.. py:currentmodule:: None\n\n\
         .. py:class:: io.StringIO\n   :canonical: _io.StringIO\n\n.. py:class:: _io.StringIO\n",
    );
    build(&dir.path, 0, &["html", "-W", "-n"]);
    let page = dir.read("_build/html/index.html");
    // The page's outline links to the object too: count the article's references.
    let body = &page[page.find("<article").unwrap()..page.find("</article>").unwrap()];
    assert_eq!(body.matches("<a href=\"#canonical.Foo\"><code>").count(), 3, "{body}");
    let data = std::fs::read(dir.join("_build/html/objects.inv")).unwrap();
    let objects = load_inventory(&data, "").unwrap();
    let all: Vec<String> = objects
        .items()
        .iter()
        .map(|i| format!("{} {} {} {}", i.name, i.kind, i.priority, i.uri))
        .collect();
    let all = all.join("\n");
    has(&all, "original.module.Foo py:class -1 index.html#canonical.Foo");
    has(&all, "canonical.Foo py:class 1 index.html#canonical.Foo");
    has(&all, "_io.StringIO py:class 1 index.html#");
    lacks(&all, "_io.StringIO py:class -1");
    // Aliases are not searched.
    let search = dir.read("_build/html/_static/searchindex.js");
    has(&search, "[0,\"canonical.Foo\"");
    lacks(&search, "[0,\"original.module.Foo\"");
}
