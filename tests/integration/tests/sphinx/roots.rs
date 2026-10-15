//! Tests ported from Sphinx 9.1's test roots. Where a root relies on a Python extension,
//! the behaviour it tests is checked with Guidedog's own equivalent.

use guidedog_tests::{has, lacks};

use crate::{build, build_json, code_count, in_order, project, with_message};

const P: &str = "project = \"P\"\n";
const NUMFIG: &str = "project = \"P\"\nnumfig = true\n";

/// test-prolog: rst_prolog opens every reStructuredText document, before its title's
/// section, and rst_epilog closes its last section; neither is added to Markdown.
#[test]
fn rst_prolog_and_epilog_follow_sphinx() {
    let conf = "project = \"Python\"\nextensions = [\"myst_parser\"]\n\
        rst_prolog = \"*Hello world*.\\n\\n\"\nrst_epilog = \"\\n\\n*Good-bye world*.\"\n";
    let dir = project(
        "prolog",
        conf,
        &[
            (
                "index.rst",
                "prolog and epilog\n=================\n\n.. toctree::\n\n   \
            restructuredtext\n   markdown\n",
            ),
            (
                "restructuredtext.rst",
                "sample document\n===============\n\n\
            This is a sample document in reST\n",
            ),
            ("markdown.md", "# sample document\n\nThis is a sample document in markdown\n"),
        ],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let rst = dir.read("_build/html/restructuredtext.html");
    in_order(
        &rst,
        &[
            "<p><em>Hello world</em>.</p>",
            "<section",
            "sample document",
            "This is a sample document in reST",
            "<p><em>Good-bye world</em>.</p>",
            "</section>",
        ],
    );
    let md = dir.read("_build/html/markdown.html");
    has(&md, "This is a sample document in markdown");
    lacks(&md, "Hello world");
    lacks(&md, "Good-bye world");
}

/// test_prepend_prologue: bibliographic fields stay first, so they still become the
/// document's metadata, and the prolog follows them.
#[test]
fn rst_prolog_follows_bibliographic_fields() {
    let conf = "project = \"P\"\nrst_prolog = \"this is rst_prolog\\nhello reST!\"\n";
    let dir = project(
        "prolog-fields",
        conf,
        &[
            ("index.rst", "Index\n=====\n"),
            // In no toctree: only a bibliographic :orphan: keeps -W from failing.
            ("other.rst", ":orphan:\n\nTitle\n=====\n\nhello Sphinx world\n"),
        ],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let page = dir.read("_build/html/other.html");
    in_order(&page, &["this is rst_prolog", "hello Sphinx world"]);
    lacks(&page, ":orphan:");
}

/// test-numfig: a label before a figure, a table, or a captioned code block names it for
/// :numref:, in the default or an explicit format; a section's label shows its number.
#[test]
fn numref_finds_items_by_the_label_before_them() {
    let index = "Index\n=====\n\n.. toctree::\n   :numbered:\n\n   foo\n\n.. _fig1:\n\n\
        .. figure:: a.png\n\n   should be Fig.1\n\n.. _table-1:\n\n\
        .. csv-table:: should be Table 1\n\n   hello,world\n\n.. _CODE_1:\n\n\
        .. code-block:: python\n   :caption: should be List 1\n\n   print('hello world')\n\n\
        * :numref:`fig1`\n* :numref:`table-1`\n* :numref:`CODE_1`\n* :numref:`foo`\n\
        * :numref:`Fig.{number} {name} <fig1>`\n* :numref:`Sect.{number} {name} <foo>`\n\
        * :numref:`Table:%s <table-1>`\n";
    let dir = project(
        "numref",
        NUMFIG,
        &[("index.rst", index), ("foo.rst", ".. _foo:\n\nFoo\n===\n"), ("a.png", "")],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    for link in [
        "<a href=\"#fig1\">Fig. 1</a>",
        "<a href=\"#table-1\">Table 1</a>",
        "<a href=\"#code-1\">Listing 1</a>",
        "<a href=\"foo.html#foo\">Section 1</a>",
        "<a href=\"#fig1\">Fig.1 should be Fig.1</a>",
        "<a href=\"foo.html#foo\">Sect.1 Foo</a>",
        "<a href=\"#table-1\">Table:1</a>",
    ] {
        has(&index, link);
    }
}

/// test-numfig's warnings: a :numref: to what has no number, or with a format without a
/// single place for the number, shows its text and warns at its line; with numfig off,
/// a :numref: to a figure warns that it is off.
#[test]
fn numref_warns_at_the_line_of_what_it_cannot_number() {
    let index = ".. _index:\n\nIndex\n=====\n\n.. _fig1:\n\n.. figure:: a.png\n\n   cap\n\n\
        * :numref:`index`\n* :numref:`invalid <fig1>`\n* :numref:`Fig %s %s <fig1>`\n\
        * :numref:`100%% of %s <fig1>`\n";
    let dir = project("numref-warn", NUMFIG, &[("index.rst", index), ("a.png", "")]);
    let diags = build_json(&dir.path);
    let d = with_message(&diags, "index has no number, so the reference shows its text.");
    assert_eq!((d.path.as_str(), d.line), ("index.rst", 12));
    let format = ": write %s or {number} once for the number, and {name} for the caption.";
    for (label, line) in [("invalid", 13), ("Fig %s %s", 14)] {
        let d = with_message(&diags, &format!("{label} is not a numfig format{format}"));
        assert_eq!(d.line, line);
    }
    let index = dir.read("_build/html/index.html");
    has(&index, "<li><code>index</code></li>");
    has(&index, "<a href=\"#fig1\">100% of 1</a>");
    dir.write("conf.toml", P);
    let diags = build_json(&dir.path);
    with_message(&diags, "numfig is off, so the fig1 reference shows its text.");
}

/// test-numfig's bar.rst: the documents a toctree lists are numbered where the toctree
/// stands, so a figure after it follows theirs.
#[test]
fn figures_of_a_toctree_entry_come_where_the_toctree_stands() {
    let dir = project(
        "numfig-toctree",
        NUMFIG,
        &[
            ("index.rst", "Index\n=====\n\n.. toctree::\n   :numbered:\n\n   bar\n"),
            (
                "bar.rst",
                "Bar\n===\n\n.. figure:: a.png\n\n   first\n\n.. toctree::\n\n   baz\n\n\
            .. figure:: a.png\n\n   third\n",
            ),
            ("baz.rst", "Baz\n---\n\n.. figure:: a.png\n\n   second\n"),
            ("a.png", ""),
        ],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let bar = dir.read("_build/html/bar.html");
    in_order(
        &bar,
        &[
            "Fig. 1.1 </span><span class=\"caption-text\">first",
            "Fig. 1.3 </span><span class=\"caption-text\">third",
        ],
    );
    let baz = dir.read("_build/html/baz.html");
    has(&baz, "<span class=\"section-number\">1.1. </span>Baz");
    has(&baz, "Fig. 1.2 </span><span class=\"caption-text\">second");
}

/// test-toctree's foo.rst: the documents of a toctree standing before a section's
/// subsections are numbered as its first children, and the subsections follow them.
#[test]
fn toctree_documents_and_subsections_share_one_numbering() {
    let dir = project(
        "toctree-numbers",
        P,
        &[
            ("index.rst", "Index\n=====\n\n.. toctree::\n   :numbered:\n\n   foo\n"),
            (
                "foo.rst",
                "foo\n===\n\n.. toctree::\n\n   quux\n\nfoo.1\n-----\n\nfoo.1-1\n^^^^^^^\n\n\
            foo.2\n-----\n",
            ),
            ("quux.rst", "quux\n====\n"),
        ],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let foo = dir.read("_build/html/foo.html");
    in_order(
        &foo,
        &[
            "<span class=\"section-number\">1. </span>foo",
            "<span class=\"section-number\">1.2. </span>foo.1",
            "<span class=\"section-number\">1.2.1. </span>foo.1-1",
            "<span class=\"section-number\">1.3. </span>foo.2",
        ],
    );
    has(&dir.read("_build/html/quux.html"), "<span class=\"section-number\">1.1. </span>quux");
}

/// test-toctree-duplicated, test-toctree-maxdepth and test-numbered-circular: a
/// document listed twice, a second numbered toctree, and a toctree listing the root
/// document each warn at the toctree's line.
#[test]
fn toctree_mistakes_warn_at_the_toctree() {
    let dir = project(
        "toctree-mistakes",
        P,
        &[
            (
                "index.rst",
                "Index\n=====\n\n.. toctree::\n   :numbered:\n\n   foo\n   foo\n   sub\n   \
            untitled\n   missing\n",
            ),
            ("foo.rst", "Foo\n===\n"),
            ("untitled.rst", "text\n"),
            ("sub.rst", "Sub\n===\n\n.. toctree::\n\n   index\n"),
            ("qux.rst", ":orphan:\n\nQux\n===\n\n.. toctree::\n   :numbered:\n\n   foo\n"),
        ],
    );
    let diags = build_json(&dir.path);
    let d = with_message(&diags, "The toctree lists foo more than once.");
    assert_eq!(
        (d.hint.as_str(), d.path.as_str(), d.line),
        ("Remove the repeated entry.", "index.rst", 4)
    );
    let d = with_message(
        &diags,
        "The toctree names untitled, which has no title, so it gets no link.",
    );
    assert_eq!(
        (d.hint.as_str(), d.path.as_str(), d.line),
        ("Give the document a title.", "index.rst", 4)
    );
    assert_eq!(code_count(&diags, "build.toctree.missing"), 1);
    let d = with_message(&diags, "sub includes index, which includes it.");
    assert_eq!(
        (d.hint.as_str(), d.path.as_str(), d.line),
        ("Remove one of the two toctree entries.", "sub.rst", 4)
    );
    let renumbered =
        "foo already has section numbers from another numbered toctree, which it keeps.";
    // The index's own repeated foo warns too; qux's numbered toctree is the second one.
    let at_qux = diags.iter().find(|d| d.message == renumbered && d.path == "qux.rst");
    assert_eq!(at_qux.map(|d| d.line), Some(6), "{diags:#?}");
    has(&dir.read("_build/html/foo.html"), "<span class=\"section-number\">1. </span>Foo");
}

/// test-domain-js's roles.rst and test-domain-py's canonical.rst: in a JavaScript
/// function's content, references look up from the function's parent; :any: finds a
/// Python object by the end of its name.
#[test]
fn any_and_javascript_references_resolve_from_where_they_stand() {
    let index = "Index\n=====\n\n.. js:class:: Parent\n\n   .. js:function:: child_1()\n\n      \
        * :js:func:`child_2`\n      * :any:`child_2`\n\n   .. js:function:: child_2()\n\n\
        .. py:module:: canonical\n\n.. py:class:: Foo\n   :canonical: original.module.Foo\n\n\
        * :any:`Foo`\n* :any:`module.Foo`\n";
    let dir = project("any-js", P, &[("index.rst", index)]);
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<li><a href=\"#Parent.child_2\"><code>child_2()</code></a></li>");
    has(&index, "<li><a href=\"#Parent.child_2\"><code>child_2</code></a></li>");
    has(&index, "<li><a href=\"#canonical.Foo\"><code>Foo</code></a></li>");
    has(&index, "<li><a href=\"#canonical.Foo\"><code>module.Foo</code></a></li>");
}

/// test-toctree-glob: a glob pattern leaves out the documents the toctree already lists.
#[test]
fn toctree_glob_leaves_out_listed_documents() {
    let dir = project(
        "glob",
        P,
        &[
            ("index.rst", "Index\n=====\n\n.. toctree::\n   :glob:\n\n   bar/index\n   bar/*\n"),
            ("bar/index.rst", "Bar\n===\n"),
            ("bar/one.rst", "One\n===\n"),
        ],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    let article = &index[index.find("<article").unwrap()..];
    assert_eq!(article.matches("<a href=\"bar/index.html\">").count(), 1);
    assert_eq!(article.matches("<a href=\"bar/one.html\">").count(), 1);
}

/// test-productionlist's Dup1.rst and Dup2.rst: a grammar token or a Python object
/// described twice warns at the later description; a canonical name does not.
#[test]
fn objects_described_twice_warn() {
    let dir = project(
        "dup",
        P,
        &[
            ("index.rst", "Index\n=====\n\n.. toctree::\n\n   Dup1\n   Dup2\n"),
            (
                "Dup1.rst",
                "Dup1\n====\n\n.. productionlist::\n   Dup: \"a\"\n\n.. py:function:: f()\n\n\
            .. py:class:: C\n   :canonical: m.C\n",
            ),
            (
                "Dup2.rst",
                "Dup2\n====\n\n.. productionlist::\n   Dup: \"b\"\n\n.. py:function:: f()\n\n\
            .. py:class:: m.C\n",
            ),
        ],
    );
    let diags = build_json(&dir.path);
    assert_eq!(code_count(&diags, "build.object.duplicate"), 2, "{diags:#?}");
    for (name, line) in [("Dup", 5), ("f", 7)] {
        let message =
            format!("{name} is also described in Dup1; references go to this description.");
        let d = with_message(&diags, &message);
        assert_eq!(
            (d.hint.as_str(), d.path.as_str(), d.line),
            ("Add :no-index: to one of the descriptions.", "Dup2.rst", line)
        );
    }
}

/// test-images: an image that does not exist warns at the line that shows it.
#[test]
fn missing_images_warn_at_their_line() {
    let index = "Index\n=====\n\ntext\n\n.. image:: testimäge.png\n";
    let dir = project("images", P, &[("index.rst", index)]);
    let diags = build_json(&dir.path);
    let d = diags.iter().find(|d| d.code == "build.image.missing").expect("a warning");
    assert_eq!((d.path.as_str(), d.line), ("index.rst", 6));
}

/// test-index_on_title: a project with contents and no index has contents as its root
/// document, with a warning, as in Sphinx.
#[test]
fn contents_is_the_root_without_an_index() {
    let dir = project(
        "contents-root",
        P,
        &[
            ("contents.rst", "Contents\n========\n\n.. toctree::\n\n   other\n"),
            ("other.rst", "Other\n=====\n"),
        ],
    );
    let diags = build_json(&dir.path);
    assert_eq!(code_count(&diags, "build.root.contents"), 1, "{diags:#?}");
    assert_eq!(code_count(&diags, "build.toctree.orphan"), 0, "other is reached");
    assert_eq!(code_count(&diags, "build.root.missing"), 0, "the root is found");
    build(&dir.path, 0, &["pdf"]);
}

/// test-ext-extlinks-hardcoded-urls: a URL an extlink could write warns at its line
/// with the role to write; the extlink's own link and a URL the pattern does not fit
/// do not.
#[test]
fn hardcoded_links_an_extlink_could_write_warn() {
    let conf = "project = \"P\"\nextensions = [\"sphinx.ext.extlinks\"]\n\
        extlinks_detect_hardcoded_links = true\n[extlinks]\n\
        issue = [\"https://github.com/sphinx-doc/sphinx/issues/%s\", \"issue %s\"]\n";
    let index = "Index\n=====\n\n:issue:`1`\n\nhttps://github.com/sphinx-doc/sphinx/issues/1\n\n\
        `inline replaceable link <https://github.com/sphinx-doc/sphinx/issues/1>`_\n\n\
        `replaceable link`_\n\nhttps://github.com/sphinx-doc/sphinx/pulls/1\n\n\
        .. _replaceable link: https://github.com/sphinx-doc/sphinx/issues/1\n";
    let dir = project("extlinks", conf, &[("index.rst", index)]);
    let diags = build_json(&dir.path);
    assert_eq!(code_count(&diags, "build.extlink.hardcoded"), 3, "{diags:#?}");
    let found = |role: &str| {
        diags
            .iter()
            .find(|d| d.message.ends_with(&format!("could be written {role}.")))
            .unwrap_or_else(|| panic!("no warning for {role}: {diags:#?}"))
    };
    let d = found(":issue:`1`");
    assert_eq!(
        (d.hint.as_str(), d.path.as_str(), d.line),
        ("Write :issue:`1` instead.", "index.rst", 6)
    );
    assert_eq!(found(":issue:`inline replaceable link <1>`").line, 8);
    assert_eq!(found(":issue:`replaceable link <1>`").line, 10);
}

/// test-domain-c's ns_lookup.rst: in a C function's content, a reference finds the
/// function's parameter, which links to the function, and names outside it.
#[test]
fn c_references_in_a_function_find_its_parameters() {
    let index = ".. c:namespace:: ns_lookup\n\nIndex\n=====\n\n.. c:var:: int i\n\n\
        .. c:function:: void f(int j)\n\n   - :c:var:`i`\n   - :c:var:`j`\n";
    let dir = project("c-lookup", P, &[("index.rst", index)]);
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(&index, "<li><a href=\"#c.ns_lookup.i\"><code>i</code></a></li>");
    has(&index, "<li><a href=\"#c.ns_lookup.f\"><code>j</code></a></li>");
}

/// test-domain-cpp's xref_consistency.rst and roles-targets-warn.rst: :cpp:expr: and
/// :cpp:texpr: link the names they use; a role naming a declaration of another kind
/// links to it and warns at its line.
#[test]
fn cpp_expressions_link_and_roles_check_their_kind() {
    let index = "Index\n=====\n\n.. cpp:namespace:: ns\n\n.. cpp:class:: item\n\n\
        .. cpp:union:: U\n\n:cpp:expr:`item`\n:cpp:texpr:`const item &`\n\
        :cpp:expr:`sizeof(item) + 1`\n\n:cpp:class:`U`\n";
    let dir = project("cpp-expr", P, &[("index.rst", index)]);
    let diags = build_json(&dir.path);
    let index = dir.read("_build/html/index.html");
    has(
        &index,
        "<code class=\"cpp-expr sig sig-inline cpp\"><a href=\"#_CPPv4N2ns4itemE\">\
        item</a></code>",
    );
    has(
        &index,
        "<span class=\"cpp-texpr sig sig-inline cpp\">const <a href=\"\
        #_CPPv4N2ns4itemE\">item</a>&amp;</span>",
    );
    has(&index, "sizeof(<a href=\"#_CPPv4N2ns4itemE\">item</a>) + 1</code>");
    has(&index, "<a href=\"#_CPPv4N2ns1UE\">");
    assert_eq!(code_count(&diags, "build.cpp.role"), 1, "{diags:#?}");
    let d = with_message(&diags, ":cpp:class: names ns::U, a union.");
    assert_eq!((d.path.as_str(), d.line), ("index.rst", 14));
}

/// test-domain-c's index.rst: the names a C declaration's types, qualification and
/// parameters use link to their declarations, and so do those of :c:expr: and
/// :c:texpr: (not a member after "->").
#[test]
fn c_declarations_and_expressions_link_their_names() {
    let index = ".. c:namespace:: index\n\nIndex\n=====\n\n.. c:struct:: A\n\n\
        .. c:type:: Sphinx\n\n.. c:member:: float Sphinx.version\n\n.. c:var:: A a\n\n\
        .. c:function:: A *make(A *from, size_t n)\n\n- :c:expr:`a->b`\n- :c:texpr:`const A *`\n";
    let dir = project("c-links", P, &[("index.rst", index)]);
    build(&dir.path, 0, &["html", "-W"]);
    let index = dir.read("_build/html/index.html");
    has(
        &index,
        "<span class=\"sig-prename descclassname\"><a href=\"#c.index.Sphinx\">\
        Sphinx</a>.</span>",
    );
    has(&index, "<a href=\"#c.index.A\">A</a> <span class=\"sig-name descname\">a</span>");
    has(&index, "<em class=\"sig-param\"><a href=\"#c.index.A\">A</a> *from</em>");
    has(&index, "<em class=\"sig-param\">size_t n</em>");
    has(
        &index,
        "<code class=\"c-expr sig sig-inline c\"><a href=\"#c.index.a\">a</a>\
        -&gt;b</code>",
    );
    has(
        &index,
        "<span class=\"c-texpr sig sig-inline c\">const <a href=\"#c.index.A\">A\
        </a> *</span>",
    );
}

/// test-domain-cpp's roles2.rst: a C++ declaration made again warns at its line.
#[test]
fn cpp_declarations_made_twice_warn_at_their_line() {
    let dir = project(
        "cpp-dup",
        P,
        &[
            ("index.rst", "Index\n=====\n\n.. cpp:class:: A\n\n.. toctree::\n\n   other\n"),
            ("other.rst", "Other\n=====\n\ntext\n\n.. cpp:class:: A\n"),
        ],
    );
    let diags = build_json(&dir.path);
    let d = diags.iter().find(|d| d.code == "build.cpp.duplicate").expect("a warning");
    assert_eq!((d.path.as_str(), d.line), ("other.rst", 6));
}
