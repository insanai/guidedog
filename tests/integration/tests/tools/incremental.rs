//! A build's result must not depend on what an earlier build left behind: these repeat
//! a build after changes, or after none, through the program, and expect what a fresh
//! build gives. The in-process halves (re-reading changed documents, re-rendering pages
//! whose lookups changed, keeping unchanged books) are covered by gd-read's
//! `project.rs`, gd-site's `incremental.rs` and gd-book's `build.rs`.

use guidedog_tests::{has, lacks};

use crate::{build, project};

const DEMO: &str = "project = \"Demo\"\n";

#[test]
fn distinct_documents_keep_distinct_objects() {
    let dir = project("inc-distinct", DEMO);
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   a/b\n   a__b\n");
    dir.write("a/b.rst", "Nested\n======\n\nThe nested page.\n");
    dir.write("a__b.rst", "Flat\n====\n\nThe flat page.\n");
    build(&dir.path, 0, &["html", "-j", "2"]);
    build(&dir.path, 0, &["html", "-a"]);
    has(&dir.read("_build/html/a/b.html"), "The nested page.");
    has(&dir.read("_build/html/a__b.html"), "The flat page.");
}

#[test]
fn diagnostics_do_not_depend_on_earlier_builds() {
    let dir = project("inc-diagnostics", DEMO);
    dir.write("index.rst", "Demo\n====\n\nSee :ref:`missing` and :py:func:`absent`.\n");
    has(&build(&dir.path, 1, &["html", "-W"]).err, "missing");
    // Nothing changed: the same warning, and the same failure.
    has(&build(&dir.path, 1, &["html", "-W"]).err, "missing");
    // -n asks for more checks than the build that wrote the page made.
    build(&dir.path, 0, &["html"]);
    has(&build(&dir.path, 1, &["html", "-n", "-W"]).err, "absent");
    // The dummy builder writes nothing, but checks every reference.
    has(&build(&dir.path, 1, &["dummy", "-W"]).err, "missing");
}

#[test]
fn failed_builds_publish_nothing_and_deleted_documents_lose_their_pages() {
    let dir = project("inc-failed", DEMO);
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   old\n\nFirst.\n");
    dir.write("old.rst", "Old\n===\n\nOld page.\n");
    build(&dir.path, 0, &["html"]);
    // A missing logo is a warning, as in Sphinx.
    dir.write("conf.toml", "project = \"Demo\"\nhtml_logo = \"missing.png\"\n");
    has(&build(&dir.path, 0, &["html"]).err, "logo file missing.png does not exist");
    // The search page fails after the document's page was written: the page stays.
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   old\n\nSecond.\n");
    dir.write("conf.toml", "project = \"Demo\"\ntemplates_path = [\"_templates\"]\n");
    dir.write(
        "_templates/layout.html",
        "{% extends \"!layout.html\" %}{% block footer %}{% if docname == \"\" %}\
         {% include \"no.html\" %}{% endif %}{% endblock %}",
    );
    build(&dir.path, 1, &["html"]);
    let page = dir.read("_build/html/index.html");
    has(&page, "First.");
    lacks(&page, "Second.");
    std::fs::remove_file(dir.join("_templates/layout.html")).unwrap();
    build(&dir.path, 0, &["html"]);
    has(&dir.read("_build/html/index.html"), "Second.");
    // A deleted document's page goes with it.
    assert!(dir.exists("_build/html/old.html"));
    std::fs::remove_file(dir.join("old.rst")).unwrap();
    dir.write("index.rst", "Demo\n====\n\nThird.\n");
    build(&dir.path, 0, &["html"]);
    has(&dir.read("_build/html/index.html"), "Third.");
    assert!(!dir.exists("_build/html/old.html"));
}

#[test]
fn pages_follow_every_input() {
    let dir = project("inc-inputs", "project = \"Demo\"\ntemplates_path = [\"_templates\"]\n");
    dir.write(
        "_templates/layout.html",
        "{% extends \"!layout.html\" %}{% block footer %}{% include \"parts/foot.html\" %}\
         {% endblock %}",
    );
    dir.write("_templates/parts/foot.html", "<p>foot one</p>");
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   api\n   more\n\n.. only:: extra\n\n   \
         Tagged text.\n\nCall :py:func:`f`.\n",
    );
    dir.write("api.rst", "API\n===\n\n.. py:function:: f()\n");
    dir.write("more.rst", "More\n====\n\nText.\n");
    build(&dir.path, 0, &["html"]);
    let page = dir.read("_build/html/index.html");
    has(&page, "foot one");
    has(&page, "href=\"api.html#f\"");
    lacks(&page, "Tagged text.");
    // -t tags change what `only` keeps.
    build(&dir.path, 0, &["html", "-t", "extra"]);
    has(&dir.read("_build/html/index.html"), "Tagged text.");
    // A template included from a subfolder.
    dir.write("_templates/parts/foot.html", "<p>foot two</p>");
    build(&dir.path, 0, &["html", "-t", "extra"]);
    has(&dir.read("_build/html/index.html"), "foot two");
    // An object that moves to another page takes the reference with it.
    dir.write("api.rst", "API\n===\n\nNothing here now.\n");
    dir.write("more.rst", "More\n====\n\n.. py:function:: f()\n");
    build(&dir.path, 0, &["html", "-t", "extra"]);
    has(&dir.read("_build/html/index.html"), "href=\"more.html#f\"");
}

#[test]
fn singlehtml_links_stay_on_the_page() {
    let dir = project("inc-single", DEMO);
    dir.write(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   a\n   b\n\nSee :doc:`b` and :ref:`b-usage`.\n",
    );
    dir.write("a.rst", "A\n=\n\nUsage\n-----\n\nFirst usage.\n");
    dir.write("b.rst", "B\n=\n\n.. _b-usage:\n\nUsage\n-----\n\nSecond usage.\n");
    lacks(&build(&dir.path, 0, &["singlehtml"]).err, "DUPLICATE ID");
    let page = dir.read("_build/singlehtml/index.html");
    lacks(&page, "href=\"a.html");
    lacks(&page, "href=\"b.html");
    for text in ["id=\"a-usage\"", "id=\"b-usage\"", "href=\"#document-b\"", "href=\"#b-b-usage\""]
    {
        has(&page, text);
    }
}

/// The pages besides the single page, the indexes and search, link to a document by the
/// single page's name: `#document-a` alone would stay on genindex.html.
#[test]
fn singlehtml_special_pages_link_to_the_single_page() {
    let dir = project("inc-single-special", DEMO);
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   a\n\n.. py:module:: m\n");
    dir.write("a.rst", "A\n=\n\n.. index:: alpha\n\nText.\n");
    build(&dir.path, 0, &["singlehtml"]);
    for name in ["genindex.html", "search.html", "py-modindex.html"] {
        let page = dir.read(&format!("_build/singlehtml/{name}"));
        has(&page, "href=\"index.html#document-a\"");
        lacks(&page, "href=\"#document-");
    }
    has(&dir.read("_build/singlehtml/genindex.html"), "href=\"index.html#a-index-0\"");
    has(&dir.read("_build/singlehtml/index.html"), "href=\"#document-a\"");
}

/// The search index holds every word of every section, however long the page.
#[test]
fn search_indexes_every_section() {
    let dir = project("inc-search", DEMO);
    let long = "Filler words for a long page. ".repeat(2000);
    dir.write(
        "index.rst",
        &format!(
            "Demo\n====\n\n{long}\n\nLate part\n---------\n\nZanzibar appears only here.\n\n\
             .. py:function:: frobnicate()\n"
        ),
    );
    build(&dir.path, 0, &["html"]);
    let index = dir.read("_build/html/_static/searchindex.js");
    has(&index, "\"zanzibar\":[");
    has(&index, "\"late-part\"");
    has(&index, "\"frobnicate\"");
    lacks(&index, "\"the\":[");
    // The index is kept while no document changed.
    has(&build(&dir.path, 0, &["html"]).out, "0 output(s) written");
}

/// A page whose toctree lists many documents needs far more room once linked than its
/// own source does; it is written like any other.
#[test]
fn pages_that_grow_when_linked_are_written() {
    let dir = project("inc-grow", DEMO);
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n   :glob:\n\n   part*\n");
    let title = "Long title words ".repeat(12);
    for i in 0..300 {
        let heading = format!("{title}{i}");
        let rule = "=".repeat(heading.len());
        dir.write(
            &format!("part{i:03}.rst"),
            &format!("{heading}\n{rule}\n\nA\n-\n\nB\n~\n\nText.\n"),
        );
    }
    build(&dir.path, 0, &["html"]);
    has(&dir.read("_build/html/index.html"), "part299.html");
}

/// A book is typeset again only when something it is made from changed. An inventory
/// that moved (a redirect) gives the same links from the cache as when it was fetched,
/// so the second build keeps the first one's book.
#[test]
fn unchanged_books_are_kept_across_a_moved_inventory() {
    use gd_fetch::testserver::{self, Reply};
    let item = gd_inventory::Item {
        name: "pkg.run".into(),
        kind: "py:function".into(),
        priority: 1,
        uri: "api.html#pkg.run".into(),
        display: "-".into(),
    };
    let inventory = gd_inventory::write_inventory("Other", "1.0", &[item]);
    let server = testserver::start(move |request| match request.path.as_str() {
        "/other/objects.inv" => Reply {
            status: 301,
            headers: vec!["Location: /other/en/stable/objects.inv".into()],
            ..Reply::default()
        },
        "/other/en/stable/objects.inv" => {
            Reply { status: 200, body: inventory.clone(), ..Reply::default() }
        }
        _ => Reply { status: 404, ..Reply::default() },
    })
    .unwrap();
    let conf = format!(
        "project = \"Demo\"\nextensions = [\"sphinx.ext.intersphinx\"]\n\n\
         [intersphinx_mapping]\nother = [\"{}\", \"\"]\n",
        server.url("/other/")
    );
    let dir = project("inc-books", &conf);
    dir.write("index.rst", "Demo\n====\n\n.. image:: dot.svg\n\n:func:`pkg.run`\n");
    let svg = |size: u32| {
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\"/>")
    };
    dir.write("dot.svg", &svg(4));
    build(&dir.path, 0, &["pdf"]);
    let moved = server.url("/other/en/stable/api.html");
    has(&dir.read("_build/pdf/book-en.typ"), &moved);
    has(&build(&dir.path, 0, &["pdf"]).out, "0 output(s) written");
    has(&dir.read("_build/pdf/book-en.typ"), &moved);
    dir.write("dot.svg", &svg(8));
    has(&build(&dir.path, 0, &["pdf"]).out, "1 output(s) written");
}

/// Each book owns its generated source as well as its PDF: reusing a generation keeps
/// both books' text, and two books may not share an output file.
#[test]
fn separate_books_keep_separate_sources_when_reused() {
    let dir = project(
        "inc-two-books",
        "project = \"Demo\"\npdf_documents = [{root = \"first\", file = \"first.pdf\", \
         title = \"First\"}, {root = \"second\", file = \"second.pdf\", title = \"Second\"}]\n",
    );
    dir.write("index.rst", "Demo\n====\n\n.. toctree::\n\n   first\n   second\n");
    dir.write("first.rst", "First\n=====\n\nThe first book alone.\n");
    dir.write("second.rst", "Second\n======\n\nThe second book alone.\n");
    let check_sources = || {
        let first = dir.read("_build/pdf/sources/first-en.pdf.typ");
        let second = dir.read("_build/pdf/sources/second-en.pdf.typ");
        has(&first, "The first book alone");
        has(&second, "The second book alone");
        lacks(&first, "The second book alone");
        lacks(&second, "The first book alone");
    };
    build(&dir.path, 0, &["pdf"]);
    check_sources();
    has(&build(&dir.path, 0, &["pdf"]).out, "0 output(s) written, 2 unchanged");
    check_sources();
    let before = std::fs::read(dir.join("_build/pdf/first-en.pdf")).unwrap();
    dir.write(
        "conf.toml",
        "project = \"Demo\"\npdf_documents = [{root = \"first\", file = \"first.pdf\"}, \
         {root = \"second\", file = \"first-en.pdf\"}]\n",
    );
    has(&build(&dir.path, 2, &["pdf"]).err, "DUPLICATE PDF OUTPUT");
    assert_eq!(std::fs::read(dir.join("_build/pdf/first-en.pdf")).unwrap(), before);
}

/// A standalone record keeps its section levels and PDF navigation: dropping its cover
/// title must not hide every outline entry.
#[test]
fn standalone_records_keep_sections_in_the_book_outline() {
    let dir = project("inc-record", "project = \"Note\"\n");
    dir.write(
        "index.rst",
        ":status: discussion\n\nNote\n====\n\nAbstract\n--------\n\nA short record.\n\n\
         References\n----------\n\nThe contract.\n",
    );
    build(&dir.path, 0, &["pdf"]);
    let source = dir.read("_build/pdf/book-en.typ");
    has(&source, "#heading(level: 2)[Abstract]");
    has(&source, "#heading(level: 2)[References]");
    lacks(&source, "#set heading(outlined: false)");
}
