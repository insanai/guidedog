//! objects.inv as Sphinx writes it: C and C++ symbols, duplicates, explicit labels and
//! parsed code captions, read back with `gd-inventory`.

use std::path::Path;

use guidedog_tests::has;

use crate::{build, quickstart};

/// The items of a built site's objects.inv, one `kind name uri display` line each.
fn inventory_lines(dir: &Path) -> String {
    let data = std::fs::read(dir.join("_build/html/objects.inv")).unwrap();
    let inventory = gd_inventory::load_inventory(&data, "").unwrap();
    let lines: Vec<String> = inventory
        .items()
        .iter()
        .map(|i| format!("{} {} {} {}", i.kind, i.name, i.uri, i.display))
        .collect();
    lines.join("\n")
}

/// A var is a member, a function's named parameters are functionParam objects at the
/// function, and an enum's enumerator is also declared in the enum's scope.
#[test]
fn c_symbols_in_the_inventory() {
    let index = "Demo\n====\n\n.. c:var:: int counter\n\n\
        .. c:function:: int hello(const char *name)\n\n   Uses :c:var:`hello.name`.\n\n\
        .. c:enum:: Color\n\n   .. c:enumerator:: RED\n";
    let dir = quickstart("inv-c", &[("index.rst", index)]);
    build(&dir.path, 0, &["html", "-W", "-n"]);
    let all = inventory_lines(&dir.path);
    has(&all, "c:member counter index.html#c.counter");
    has(&all, "c:functionParam hello.name index.html#c.hello");
    has(&all, "c:enumerator Color.RED index.html#c.Color.RED");
    has(&all, "c:enumerator RED index.html#c.Color.RED");
    has(&dir.read("_build/html/index.html"), "<a href=\"#c.hello\"><code>hello.name</code></a>");
}

/// A member of an anonymous entity is listed with its anonymous part, displayed as
/// "[anonymous]", and found through its parent (Sphinx's test-domain-c).
#[test]
fn c_anonymous_entities_in_the_inventory() {
    let index = ".. c:namespace:: ns\n\nDemo\n====\n\n.. c:struct:: A\n\n   \
        .. c:union:: @data\n\n      .. c:member:: int a\n\n\
        See :c:member:`A.@data.a` and :c:member:`A.a`.\n";
    let dir = quickstart("inv-anonymous", &[("index.rst", index)]);
    build(&dir.path, 0, &["html", "-W", "-n"]);
    let all = inventory_lines(&dir.path);
    has(&all, "c:union ns.A.@data index.html#c.ns.A.@data ns.A.[anonymous]");
    has(&all, "c:member ns.A.@data.a index.html#c.ns.A.@data.a ns.A.[anonymous].a");
    let page = dir.read("_build/html/index.html");
    assert_eq!(page.matches("href=\"#c.ns.A.&#64;data.a\"").count(), 2, "{page}");
}

/// The C++ symbol tree as Sphinx 9.1 builds it: template and function parameters are
/// symbols at their declaration's id, an unscoped enum's enumerator is declared in the
/// enclosing scope too, and a redeclaration gets no ids and no permalink.
#[test]
fn cpp_symbols_in_the_inventory() {
    let index = "Demo\n====\n\n.. cpp:namespace:: ns\n\n.. cpp:enum:: E\n\n   \
        .. cpp:enumerator:: A\n\n.. cpp:class:: template<typename T> C\n\n   \
        .. cpp:function:: void f(int a)\n\n      See :cpp:var:`a` and :cpp:type:`T`.\n\n\
        .. cpp:function:: void g(int)\n\n.. cpp:function:: void g(int)\n";
    let dir = quickstart("inv-cpp", &[("index.rst", index)]);
    let r = build(&dir.path, 0, &["html"]);
    has(&r.err, "DUPLICATE C++ DECLARATION");
    let all = inventory_lines(&dir.path);
    has(&all, "cpp:enumerator ns::A index.html#_CPPv4N2ns1E1AE");
    has(&all, "cpp:enumerator ns::E::A index.html#_CPPv4N2ns1E1AE");
    has(&all, "cpp:templateParam ns::C::T index.html#_CPPv4I0EN2ns1CE");
    has(&all, "cpp:functionParam ns::C::f::a index.html#_CPPv4N2ns1C1fEi");
    let page = dir.read("_build/html/index.html");
    has(&page, "<a href=\"#_CPPv4N2ns1C1fEi\">");
    has(&page, "<a href=\"#_CPPv4I0EN2ns1CE\">");
    let permalink = "href=\"#_CPPv4N2ns1gEi\" title=\"Link to this definition\"";
    assert_eq!(page.matches(permalink).count(), 1);
}

/// An object described twice keeps one entry, the one noted last, as Sphinx's domains
/// keep one object per name (test-productionlist's Dup token).
#[test]
fn duplicate_objects_keep_the_last_entry() {
    let dir = quickstart(
        "inv-dup",
        &[
            ("index.rst", "Demo\n====\n\n.. toctree::\n\n   one\n   two\n"),
            ("one.rst", "One\n===\n\n.. productionlist::\n   Dup: `Dup` | x\n"),
            ("two.rst", "Two\n===\n\n.. productionlist::\n   Dup: `Dup` | y\n"),
        ],
    );
    build(&dir.path, 0, &["html"]);
    let data = std::fs::read(dir.join("_build/html/objects.inv")).unwrap();
    let inventory = gd_inventory::load_inventory(&data, "").unwrap();
    let found: Vec<&str> =
        inventory.items().iter().filter(|i| i.name == "Dup").map(|i| i.uri.as_str()).collect();
    assert_eq!(found, ["two.html#grammar-token-Dup"]);
}

/// Citations and inline targets are explicit names, which Sphinx lists as labels without
/// a title of their own (test-markup-citation).
#[test]
fn citations_and_inline_targets_are_labels() {
    let index = "Demo\n====\n\nSee [CITE1]_ and _`Inline Name`.\n\n.. [CITE1] A citation.\n";
    let dir = quickstart("inv-labels", &[("index.rst", index)]);
    build(&dir.path, 0, &["html", "-W"]);
    let all = inventory_lines(&dir.path);
    has(&all, "std:label cite1 index.html#cite1 -");
    has(&all, "std:label inline name index.html#inline-name -");
}

/// A code block's caption is parsed as inline markup: the page shows its emphasis, and
/// its label's title is its text (test-directive-code's caption.rst).
#[test]
fn code_captions_are_parsed() {
    let index = "Demo\n====\n\n.. code-block:: ruby\n   :name: name *test* rb\n   \
        :caption: caption *test* rb\n\n   x\n";
    let dir = quickstart("inv-caption", &[("index.rst", index)]);
    build(&dir.path, 0, &["html", "-W"]);
    let data = std::fs::read(dir.join("_build/html/objects.inv")).unwrap();
    let inventory = gd_inventory::load_inventory(&data, "").unwrap();
    let found = inventory.items().iter().find(|i| i.name == "name *test* rb");
    assert_eq!(found.map(|i| i.display.as_str()), Some("caption test rb"));
    has(
        &dir.read("_build/html/index.html"),
        "<span class=\"caption-text\">caption <em>test</em> rb</span>",
    );
}
