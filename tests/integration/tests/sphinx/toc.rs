//! Tables of contents as Sphinx writes them: object entries, generated pages in
//! toctrees, :tocdepth:, sections inside `only`, and untitled documents.

use guidedog_tests::{has, lacks};

use crate::{build, quickstart};

const NUMBERED_API: &str = "Demo\n====\n\n.. toctree::\n   :numbered:\n\n   api\n";

/// Objects appear in the table of contents as Sphinx 7.2+ puts them
/// (toc_object_entries): under their section, nested under the object that describes
/// them, in document order among subsections, as code, without section numbers.
#[test]
fn object_descriptions_have_toc_entries() {
    let api = "API\n===\n\n.. py:module:: mod\n\n.. py:class:: Queue\n\n   \
        .. py:method:: put(item)\n\nHelpers\n-------\n\n.. py:function:: helper()\n\n   \
        Notes\n   =====\n\n   Text.\n\n.. py:function:: quiet()\n   :no-contents-entry:\n";
    let dir = quickstart("toc-objects", &[("index.rst", NUMBERED_API), ("api.rst", api)]);
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(
        &index,
        "<a href=\"api.html\">1. API</a></p>\n<ul>\n<li>\n\
        <p><a href=\"api.html#mod.Queue\"><code>Queue</code></a></p>\n<ul>\n<li>\n\
        <p><a href=\"api.html#mod.Queue.put\"><code>Queue.put()</code></a></p>",
    );
    has(
        &index,
        "<p><a href=\"api.html#helpers\">1.1. Helpers</a></p>\n<ul>\n<li>\n\
        <p><a href=\"api.html#mod.helper\"><code>helper()</code></a></p>",
    );
    lacks(&index, "mod.quiet\"><code>");
    lacks(&index, "api.html#notes");
    let api = dir.read("_build/html/api.html");
    // The sidebar shows the current page's entries; the rail lists them too.
    has(
        &api,
        "<li class=\"toctree-l2\"><a class=\"reference internal\" \
        href=\"#mod.Queue\"><code>Queue</code></a><ul><li class=\"toctree-l3\">",
    );
    has(&api, "<li class=\"level-2\"><a href=\"#mod.Queue\"><code>Queue</code></a></li>");
    dir.write("conf.toml", "project = \"Demo\"\ntoc_object_entries_show_parents = \"hide\"\n");
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<a href=\"api.html#mod.Queue.put\"><code>put()</code></a>");
    dir.write("conf.toml", "project = \"Demo\"\ntoc_object_entries = false\n");
    build(&dir.path, 0, &["html", "-W"]);
    lacks(&dir.read("_build/html/index.html"), "<code>");
    dir.write("conf.toml", "toc_object_entries_show_parents = \"some\"\n");
    let r = build(&dir.path, 2, &["html"]);
    has(&r.err, "toc_object_entries_show_parents must be domain, hide, or all.");
}

/// A toctree may name the generated pages, as Sphinx's _virtual_doc_names: genindex,
/// modindex (py-modindex), and search.
#[test]
fn toctrees_list_generated_pages() {
    let index = "Demo\n====\n\n.. toctree::\n\n   sub/page\n\n.. toctree::\n   \
        :caption: Indices\n   :name: indices\n\n   genindex\n   modindex\n   Find it <search>\n";
    let dir = quickstart(
        "toc-generated",
        &[("index.rst", index), ("sub/page.rst", "Page\n====\n\n.. toctree::\n\n   /genindex\n")],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<p><a href=\"genindex.html\">Index</a></p>");
    has(&index, "<p><a href=\"py-modindex.html\">Module Index</a></p>");
    has(&index, "<p><a href=\"search.html\">Find it</a></p>");
    has(&index, "<div id=\"indices\" class=\"toctree-wrapper compound\">");
    let page = dir.read("_build/html/sub/page.html");
    has(&page, "<p><a href=\"../genindex.html\">Index</a></p>");
    has(
        &page,
        "<li class=\"toctree-l1\"><a class=\"reference internal\" \
        href=\"../search.html\">Find it</a></li>",
    );
}

/// A document's :tocdepth: cuts its own entries, counted from its title, wherever a
/// toctree shows it; toctrees below the cut are left out with them.
#[test]
fn tocdepth_limits_a_documents_entries() {
    let bar = ":tocdepth: 2\n\nBar\n===\n\nBar A\n-----\n\n.. toctree::\n\n   baz\n\n\
        Bar B\n-----\n\nBar B1\n~~~~~~\n\nText.\n";
    let dir = quickstart(
        "tocdepth",
        &[
            ("index.rst", "Demo\n====\n\n.. toctree::\n\n   bar\n"),
            ("bar.rst", bar),
            ("baz.rst", "Baz\n===\n\nText.\n"),
        ],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<a href=\"bar.html#bar-b\">Bar B</a>");
    lacks(&index, "bar-b1");
    lacks(&index, "<a href=\"baz.html\">");
}

/// Sections inside `only` are kept or dropped with it, and tables of contents list them
/// only when the expression holds; a dropped section still takes its id, as in Sphinx.
#[test]
fn sections_in_only_follow_their_expression() {
    let only = "Top\n===\n\n.. only:: html\n\n   Web\n   ---\n\n\
        .. only:: latex\n\n   Web\n   ---\n\n   \
        Print.\n\nWeb\n---\n\n.. only:: html\n\n   Second\n   ======\n";
    let dir = quickstart(
        "toc-only",
        &[("index.rst", "Demo\n====\n\n.. toctree::\n\n   only\n"), ("only.rst", only)],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(
        &index,
        "<p><a href=\"only.html#web\">Web</a></p>\n</li>\n<li>\n\
        <p><a href=\"only.html#id2\">Web</a></p>",
    );
    has(&index, "<p><a href=\"only.html#second\">Second</a></p>");
    lacks(&index, "only.html#id1");
    let page = dir.read("_build/html/only.html");
    has(&page, "<section id=\"id2\">");
    lacks(&page, "Print.");
    has(
        &page,
        "</ul></li><li class=\"toctree-l1\"><a class=\"reference internal\" \
        href=\"#second\">Second</a></li>",
    );
}

/// A document without a title gets no entry of its own: the objects it describes stand
/// in its place, the first linking to the page; with nothing, the toctree is warned about.
#[test]
fn untitled_documents_in_toctrees() {
    let dir = quickstart(
        "toc-untitled",
        &[
            ("index.rst", "Demo\n====\n\n.. toctree::\n\n   api\n   empty\n"),
            ("api.rst", ".. py:function:: first()\n\n.. py:function:: second()\n"),
            ("empty.rst", "Only text.\n"),
        ],
    );
    let r = build(&dir.path, 0, &["html"]);
    has(&r.err, "build.toctree.untitled");
    let index = dir.read("_build/html/index.html");
    has(&index, "<p><a href=\"api.html\"><code>first()</code></a></p>");
    has(&index, "<p><a href=\"api.html#second\"><code>second()</code></a></p>");
    lacks(&index, "<a href=\"empty.html\">");
}
