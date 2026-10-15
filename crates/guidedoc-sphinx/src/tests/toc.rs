//! Tests of toctrees and the table of contents.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

const TOC_OBJECTS: &str = concat!(
    ".. py:module:: mod\n\n",
    ".. py:class:: Queue\n\n",
    "   .. py:method:: put(item)\n\n",
    "   .. py:attribute:: size\n\n",
    ".. js:function:: fetch(url)\n\n",
    ".. cpp:class:: template<typename T> Box\n\n",
    "   .. cpp:function:: void open()\n\n",
    ".. c:macro:: MAX(a, b)\n\n",
    ".. c:struct:: point\n\n",
    "   .. c:member:: int x\n\n",
    ".. rst:directive:: toctree\n\n",
    "   .. rst:directive:option:: maxdepth\n\n",
    ".. rst:role:: ref\n\n",
    ".. confval:: timeout\n\n",
    ".. envvar:: HOME\n",
);

// Each domain names its objects' table of contents entries as Sphinx's _toc_entry_name
// does, following toc_object_entries_show_parents and add_function_parentheses.
#[test]
fn toc_entry_names_follow_each_domain() {
    expect_contains(
        TOC_OBJECTS,
        &[
            "@toc=\"Queue\"",
            "@toc=\"Queue.put()\"",
            "@toc=\"Queue.size\"",
            "@toc=\"fetch()\"",
            "@toc=\"Box\"",
            "@toc=\"Box::open()\"",
            "@toc=\"MAX()\"",
            "@toc=\"point\"",
            "@toc=\"x\"",
            "@toc=\".. toctree::\"",
            "@toc=\":maxdepth:\"",
            "@toc=\":ref:\"",
            "@toc=\"timeout\"",
        ],
        None,
    );
    let got = read_text(TOC_OBJECTS, None).dump;
    assert_eq!(got.matches("@toc=").count(), 13, "envvar gets no entry");
    let hide = Config {
        toc_object_entries_show_parents: TocParents::Hide,
        add_function_parentheses: false,
        ..Config::default()
    };
    expect_contains(
        TOC_OBJECTS,
        &[
            "@toc=\"Queue\"",
            "@toc=\"put\"",
            "@toc=\"size\"",
            "@toc=\"fetch\"",
            "@toc=\"Box\"",
            "@toc=\"open\"",
            "@toc=\"MAX\"",
            "@toc=\"point\"",
            "@toc=\"x\"",
            "@toc=\".. toctree::\"",
            "@toc=\":maxdepth:\"",
            "@toc=\":ref:\"",
        ],
        Some(&hide),
    );
    let all = Config { toc_object_entries_show_parents: TocParents::All, ..Config::default() };
    expect_contains(
        TOC_OBJECTS,
        &[
            "@toc=\"mod.Queue\"",
            "@toc=\"mod.Queue.put()\"",
            "@toc=\"Box\"",
            "@toc=\"Box::open()\"",
            "@toc=\"point::x\"",
            "@toc=\".. toctree::\"",
            "@toc=\":maxdepth:\"",
        ],
        Some(&all),
    );
    let off = Config { toc_object_entries: false, ..Config::default() };
    let got = read_text(TOC_OBJECTS, Some(&off)).dump;
    assert!(!got.contains("@toc="), "toc_object_entries = False");
}

// :no-contents-entry: (or :nocontentsentry:) is noted on the object; :no-index: leaves
// the signature without an id, so it has no entry either.
#[test]
fn toc_entries_can_be_left_out() {
    expect_contains(
        concat!(
            ".. py:function:: f()\n   :nocontentsentry:\n\n",
            ".. py:function:: g()\n   :no-index:\n",
        ),
        &[
            "desc @domain=\"py\" @objtype=\"function\" @no-contents-entry=\"1\"",
            "desc-signature @id=\"f\" @fullname=\"f\" @toc=\"f()\"",
            "desc-signature @fullname=\"g\"\n",
        ],
        None,
    );
}

// Titles in an object's content begin sections, as in Sphinx's
// test-object-description-sections, with adornments of their own.
#[test]
fn sections_in_object_descriptions() {
    expect_contains(
        concat!(
            "Top\n===\n\n.. py:function:: func()\n\n   Overview\n   ========\n\n",
            "   Text.\n\nAfter\n=====\n",
        ),
        &[
            "section value=1\n    heading value=1\n      text \"Top\"",
            concat!(
                "      desc-content\n        section value=2\n          heading value=2\n",
                "            text \"Overview\"\n          paragraph\n            text \"Text.\"",
            ),
            "  section value=1\n    heading value=1\n      text \"After\"",
        ],
        None,
    );
}
