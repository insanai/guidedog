//! Tests of the Sphinx directives outside the domains.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

#[test]
fn toctree_entries_and_options() {
    expect_dump(
        concat!(
            ".. toctree::\n",
            "   :maxdepth: 2\n",
            "   :caption: Contents\n",
            "   :numbered:\n",
            "   :glob:\n",
            "   :hidden:\n",
            "\n",
            "   intro\n",
            "   Usage guide <usage/index>\n",
            "   self\n",
            "   https://example.org\n",
            "   api/*\n",
        ),
        concat!(
            "document value=1\n",
            "  toctree value=2 @caption=\"Contents\" @hidden=\"1\" @glob=\"1\" ",
            "@numbered=\"999\"\n",
            "    toctree-entry \"intro\"\n",
            "    toctree-entry \"usage/index\" @title=\"Usage guide\"\n",
            "    toctree-entry \"self\"\n",
            "    toctree-entry \"https://example.org\"\n",
            "    toctree-entry \"api/*\"\n",
        ),
        None,
    );
}

#[test]
fn toctree_without_options() {
    expect_dump(
        ".. toctree::\n   a\n   b\n",
        "document value=1\n  toctree\n    toctree-entry \"a\"\n    toctree-entry \"b\"\n",
        None,
    );
}

#[test]
fn only_and_ifconfig() {
    expect_dump(
        concat!(
            ".. only:: html and not latex\n\n   Web only.\n\n",
            ".. ifconfig:: release.endswith('dev')\n\n   Dev.\n",
        ),
        concat!(
            "document value=1\n",
            "  only @expression=\"html and not latex\"\n",
            "    paragraph\n      text \"Web only.\"\n",
            "  only @ifconfig=\"release.endswith('dev')\"\n",
            "    paragraph\n      text \"Dev.\"\n",
        ),
        None,
    );
}

// Titles in only begin sections, as in Sphinx's test-directive-only: a new adornment
// nests in the section the directive stands in; a known one places the content at its
// level; adornments inside take levels afresh and are forgotten after it.
#[test]
fn sections_in_only() {
    expect_dump(
        concat!(
            "A\n=\n\n.. only:: html\n\n   In A\n   ----\n\n   Deeper\n   ~~~~~~\n\n",
            "B\n-\n\nB1\n~~\n\n.. only:: html\n\n   B2\n   ~~\n\n",
            ".. only:: html\n\n   C\n   -\n\n   Text.\n\n",
            ".. only:: html\n\n   D\n   =\n",
        ),
        concat!(
            "document value=1\n",
            "  section value=1\n    heading value=1\n      text \"A\"\n",
            "    only @expression=\"html\"\n",
            "      section value=2\n        heading value=2\n          text \"In A\"\n",
            "        section value=3\n          heading value=3\n            text \"Deeper\"\n",
            "    section value=2\n      heading value=2\n        text \"B\"\n",
            "      section value=3\n        heading value=3\n          text \"B1\"\n",
            "      only @expression=\"html\"\n",
            "        section value=3\n          heading value=3\n            text \"B2\"\n",
            "    only @expression=\"html\"\n",
            "      section value=2\n        heading value=2\n          text \"C\"\n",
            "        paragraph\n          text \"Text.\"\n",
            "  only @expression=\"html\"\n",
            "    section value=1\n      heading value=1\n        text \"D\"\n",
        ),
        None,
    );
    // Inside a body element, titles stay errors.
    expect_codes("* .. only:: html\n\n     T\n     -\n", "rst.section.unexpected", None);
}

#[test]
fn index_directive_entries() {
    expect_dump(
        concat!(
            ".. index::\n",
            "   single: execution; context\n",
            "   pair: loop; statement\n",
            "   !triple: a; b; c\n",
            "   see: x; y\n",
            "   seealso: p; q\n",
            "   Python, interpreter\n\n",
            ".. index:: second\n",
        ),
        concat!(
            "document value=1\n",
            "  index @id=\"index-0\" @entries=\"single: execution; context\\npair: loop; ",
            "statement\\n!triple: a; b; c\\nsee: x; y\\nseealso: p; q\\nPython, interpreter\"\n",
            "  index @id=\"index-1\" @entries=\"second\"\n",
        ),
        None,
    );
}

#[test]
fn index_entry_with_wrong_parts_warns() {
    expect_codes(".. index:: pair: lonely\n", "sphinx.index.entry", None);
    expect_codes(".. index:: triple: a; b\n", "sphinx.index.entry", None);
}

#[test]
fn centered_rubric_hlist() {
    expect_dump(
        concat!(
            ".. centered:: LICENSE *terms*\n\n",
            ".. rubric:: Footnotes\n   :heading-level: 2\n\n",
            ".. hlist::\n   :columns: 2\n\n   * a\n   * b\n   * c\n",
        ),
        concat!(
            "document value=1\n",
            "  paragraph @class=\"centered\"\n",
            "    text \"LICENSE \"\n    emphasis\n      text \"terms\"\n",
            "  rubric @heading-level=\"2\"\n    text \"Footnotes\"\n",
            "  hlist value=2\n",
            "    list +tight list=bullet,*,0\n",
            "      list-item\n        paragraph\n          text \"a\"\n",
            "      list-item\n        paragraph\n          text \"b\"\n",
            "    list +tight list=bullet,*,0\n",
            "      list-item\n        paragraph\n          text \"c\"\n",
        ),
        None,
    );
}

#[test]
fn hlist_needs_a_bullet_list() {
    expect_codes(".. hlist::\n\n   Just text.\n", "sphinx.hlist.content", None);
}

#[test]
fn version_directives() {
    expect_dump(
        concat!(
            ".. versionadded:: 2.5\n\n",
            ".. versionchanged:: 3.0 The *default* changed.\n\n",
            ".. deprecated:: 3.1\n\n   Use :func:`other` instead.\n\n",
            ".. versionremoved:: 4.0 Gone.\n",
        ),
        concat!(
            "document value=1\n",
            "  version-modified @type=\"versionadded\" @version=\"2.5\"\n",
            "  version-modified @type=\"versionchanged\" @version=\"3.0\"\n",
            "    paragraph\n      text \"The \"\n      emphasis\n        text \"default\"\n",
            "      text \" changed.\"\n",
            "  version-modified @type=\"deprecated\" @version=\"3.1\"\n",
            "    paragraph\n      text \"Use \"\n",
            "      xref link=unresolved name=\"other\" @reftype=\"py:func\"\n",
            "        code \"other()\"\n",
            "      text \" instead.\"\n",
            "  version-modified @type=\"versionremoved\" @version=\"4.0\"\n",
            "    paragraph\n      text \"Gone.\"\n",
        ),
        None,
    );
}

#[test]
fn authors_follow_show_authors() {
    let text = concat!(
        ".. sectionauthor:: Ada <ada@example.org>\n\n",
        ".. moduleauthor:: Bob\n\n.. codeauthor:: Cy\n\nText.\n",
    );
    expect_dump(text, "document value=1\n  paragraph\n    text \"Text.\"\n", None);
    let config = Config { show_authors: true, ..Config::default() };
    expect_dump(
        text,
        concat!(
            "document value=1\n",
            "  directive @name=\"sectionauthor\" @author=\"Ada <ada@example.org>\"\n",
            "  directive @name=\"moduleauthor\" @author=\"Bob\"\n",
            "  directive @name=\"codeauthor\" @author=\"Cy\"\n",
            "  paragraph\n    text \"Text.\"\n",
        ),
        Some(&config),
    );
}

#[test]
fn state_directives_emit_nothing() {
    expect_dump(
        concat!(
            ".. tabularcolumns:: |l|c|p{5cm}|\n\n",
            ".. default-domain:: c\n\n",
            ".. highlight:: c\n   :linenothreshold: 5\n\n",
            ".. py:currentmodule:: os\n\n.. program:: rm\n",
        ),
        "document value=1\n",
        None,
    );
}

#[test]
fn todo_follows_todo_include_todos() {
    let text = ".. todo:: Write more.\n\n.. todolist::\n";
    expect_dump(text, "document value=1\n  directive @name=\"todolist\"\n", None);
    let config = Config { todo_include_todos: true, ..Config::default() };
    expect_dump(
        text,
        concat!(
            "document value=1\n  admonition value=11\n    paragraph\n      text \"Write more.\"\n",
            "  directive @name=\"todolist\"\n",
        ),
        Some(&config),
    );
}

#[test]
fn seealso_and_rst_class() {
    expect_dump(
        concat!(".. seealso:: Module :mod:`zipfile`\n\n", ".. rst-class:: special\n\nPara.\n",),
        concat!(
            "document value=1\n",
            "  admonition value=10\n    paragraph\n      text \"Module \"\n",
            "      xref link=unresolved name=\"zipfile\" @reftype=\"py:mod\"\n",
            "        code \"zipfile\"\n",
            "  paragraph @class=\"special\"\n    text \"Para.\"\n",
        ),
        None,
    );
}

#[test]
fn default_role_accepts_sphinx_roles() {
    expect_dump(
        ".. default-role:: py:obj\n\n`os.path`\n\n.. default-role::\n\n`title`\n",
        concat!(
            "document value=1\n",
            "  paragraph\n    xref link=unresolved name=\"os.path\" @reftype=\"py:obj\"\n",
            "      code \"os.path\"\n",
            "  paragraph\n    title-reference\n      text \"title\"\n",
        ),
        None,
    );
    expect_codes(".. default-role:: nosuchrole\n", "sphinx.role.unknown", None);
}

#[test]
fn default_role_from_config() {
    let mut config = Config::default();
    config.default_role = "any".into();
    expect_dump(
        "`thing`\n",
        concat!(
            "document value=1\n  paragraph\n",
            "    xref link=unresolved name=\"thing\" @reftype=\"any\" @cpp:name=\"thing\x1f\"\n",
            "      code \"thing\"\n",
        ),
        Some(&config),
    );
}

#[test]
fn graphviz_directives() {
    expect_dump(
        concat!(
            ".. graphviz::\n   :caption: Flow\n   :layout: neato\n   :name: flow\n\n",
            "   digraph { a -> b }\n\n",
            ".. digraph:: foo\n   :alt: Foo\n   :align: center\n   :class: wide\n\n",
            "   a -> b\n\n",
            ".. graph:: bar\n   :graphviz_dot: circo\n\n   a -- b\n\n",
            ".. graphviz:: old.txt\n",
        ),
        concat!(
            "document value=1\n",
            "  code-block \"digraph { a -> b }\\n\" @language=\"dot\" @graphviz=\"1\" ",
            "@layout=\"neato\" @caption=\"Flow\" @name=\"flow\"\n",
            "    caption\n      text \"Flow\"\n",
            "  code-block \"digraph foo {\\na -> b\\n}\\n\" @language=\"dot\" @class=\"wide\" ",
            "@graphviz=\"1\" @layout=\"\" @alt=\"Foo\" @align=\"center\"\n",
            "  code-block \"graph bar {\\na -- b\\n}\\n\" @language=\"dot\" @graphviz=\"1\" ",
            "@layout=\"circo\"\n",
            "  code-block \"a\\nb\\nc\\nd\\n\" @language=\"dot\" @graphviz=\"1\" @layout=\"\"\n",
        ),
        None,
    );
    expect_codes(".. graphviz:: missing.dot\n", "core.include.missing", None);
    expect_codes(".. graphviz:: old.txt\n\n   a -> b\n", "sphinx.graphviz.source", None);
    expect_codes(".. graphviz::\n", "sphinx.graphviz.source", None);
    assert!(crate::extension_supported("sphinx.ext.graphviz"));
    assert!(!crate::extension_supported("sphinx.ext.autodoc"));
}

#[test]
fn doctest_directives() {
    expect_dump(
        concat!(
            ".. testsetup:: *\n\n   import os\n\n",
            ".. doctest:: g1, g2\n   :options: +ELLIPSIS\n   :pyversion: > 3.8\n\n",
            "   >>> print(1)  # doctest: +SKIP\n   1\n   <BLANKLINE>\n\n",
            ".. testcode::\n   :skipif: True\n\n   print(2)  # doctest: +SKIP\n\n",
            ".. testoutput::\n   :options: +NORMALIZE_WHITESPACE\n\n   2\n\n",
            ".. doctest::\n   :no-trim-doctest-flags:\n\n   >>> x  # doctest: +SKIP\n\n",
            ".. testcode::\n   :hide:\n\n   secret()\n\n",
            ".. testcleanup::\n\n   os.remove('x')\n",
        ),
        concat!(
            "document value=1\n",
            "  comment \"import os\\n\" @testnodetype=\"testsetup\" @groups=\"*\"\n",
            "  code-block \">>> print(1)\\n1\\n\\n\" @language=\"pycon\" ",
            "@testnodetype=\"doctest\" @groups=\"g1, g2\"\n",
            "  code-block \"print(2)  # doctest: +SKIP\\n\" @language=\"python\" ",
            "@testnodetype=\"testcode\" @groups=\"default\"\n",
            "  code-block \"2\\n\" @language=\"none\" @testnodetype=\"testoutput\" ",
            "@groups=\"default\"\n",
            "  code-block \">>> x  # doctest: +SKIP\\n\" @language=\"pycon\" ",
            "@testnodetype=\"doctest\" @groups=\"default\"\n",
            "  comment \"secret()\\n\" @testnodetype=\"testcode\" @groups=\"default\"\n",
            "  comment \"os.remove('x')\\n\" @testnodetype=\"testcleanup\" ",
            "@groups=\"default\"\n",
        ),
        None,
    );
    let mut config = Config::default();
    config.trim_doctest_flags = false;
    expect_dump(
        ".. doctest::\n\n   >>> x  # doctest: +SKIP\n   <BLANKLINE>\n",
        concat!(
            "document value=1\n  code-block \">>> x  # doctest: +SKIP\\n<BLANKLINE>\\n\" ",
            "@language=\"pycon\" @testnodetype=\"doctest\" @groups=\"default\"\n",
        ),
        Some(&config),
    );
    expect_dump(
        ".. testcode::\n   :trim-doctest-flags:\n\n   >>> y  # doctest: +X\n",
        concat!(
            "document value=1\n  code-block \">>> y\\n\" @language=\"python\" ",
            "@testnodetype=\"testcode\" @groups=\"default\"\n",
        ),
        None,
    );
    assert_eq!(crate::doctest::trim_doctest_line("a # not a flag"), "a # not a flag");
    assert_eq!(crate::doctest::trim_doctest_line("a #doctest:"), "a #doctest:");
    assert!(crate::extension_supported("sphinx.ext.doctest"));
}

#[test]
fn default_substitutions_come_from_config() {
    let mut config = Config::default();
    config.version = "3.15".into();
    config.release = "3.15.0a1".into();
    config.today = "Sep 29, 2026".into();
    expect_dump(
        "|version| |release| |today|\n",
        concat!(
            "document value=1\n  paragraph\n    text \"3.15\"\n    text \" \"\n",
            "    text \"3.15.0a1\"\n    text \" \"\n    text \"Sep 29, 2026\"\n",
        ),
        Some(&config),
    );
    // A document's own definition wins, the prolog's included.
    config.rst_prolog = ".. |release| replace:: mine\n".into();
    expect_contains(
        "|release| |version|\n",
        &["substitution-reference \"release\" link=internal", "text \"3.15\""],
        Some(&config),
    );
    config.rst_prolog = "".into();
    // Names match exactly, before Docutils' case-insensitive fallback: |Version| is not
    // a default substitution.
    expect_codes("|Version|\n", "rst.substitution.undefined", Some(&config));
    // Without Sphinx's configuration they are empty, as Sphinx's defaults are.
    expect_dump("|version|.\n", "document value=1\n  paragraph\n    text\n    text \".\"\n", None);
}
