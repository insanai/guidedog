//! Extraction from documents read by the reStructuredText and CommonMark readers.

mod common;

use common::{has, lacks, messages, read};
use gd_i18n::{ExtractOptions, PotHeader, Target, Targets, Template};
use guidedoc::{ReadOptions, Workspace};

const RST: &str = "Guide
=====

A paragraph with *emphasis*, ``code``, and a link_,
continued on a second line.

.. _link: https://example.org

- First item.
- Second item.

.. note:: Watch out.

Term
   Definition text.

:Field: Body text.

| Line one.
| Line two.

.. figure:: logo.png
   :alt: The logo

   Caption *text*.

   Legend paragraph.

.. rubric:: A rubric

+-------+-------+
| Cell  | Other |
+-------+-------+

Part
----

::

   literal code

.. |sub| replace:: substituted text

.. math::

   E = mc^2

.. This is a comment.
";

#[test]
fn extracts_what_sphinx_does() {
    let got = messages("rst", RST, Targets::default());
    let want = [
        "Guide",
        "A paragraph with *emphasis*, ``code``, and a link_, continued on a second line.",
        "First item.",
        "Second item.",
        "Watch out.",
        "Term",
        "Definition text.",
        "Field",
        "Body text.",
        "Line one.",
        "Line two.",
        "Caption *text*.",
        "Legend paragraph.",
        "A rubric",
        "Cell",
        "Other",
        "Part",
        "E = mc^2",
    ];
    assert_eq!(got, want);
}

#[test]
fn extracts_additional_targets() {
    let got = messages("rst", RST, Targets::of(&[Target::LiteralBlock, Target::Image]));
    let joined = got.join("|");
    has(&joined, "|literal code|");
    has(&joined, "|The logo|");
}

#[test]
fn extracts_markdown_source_text() {
    let text = "# Title with `code`\n\nSome *Markdown* [link](https://example.org)\n\
                over two lines.\n\n- item\n";
    let got = messages("commonmark", text, Targets::default());
    assert_eq!(
        got,
        [
            "Title with `code`",
            "Some *Markdown* [link](https://example.org) over two lines.",
            "item"
        ]
    );
}

#[test]
fn template_layout_matches_sphinx() {
    let text = "Hello\n=====\n\nSame.\n\nSame.\n\nOther \"quoted\".\n";
    let mut ws = Workspace::new();
    let doc = read(&mut ws, "rst", text, ReadOptions::default());
    let mut pot = Template::new("index");
    let options = ExtractOptions {
        location: true,
        uuid: true,
        path: "../index.rst",
        docname: "index",
        ..Default::default()
    };
    assert_eq!(pot.extract(doc, &options), 4);
    pot.set_header(&PotHeader {
        project: "Demo",
        version: "1.0",
        copyright: "2026, Ada",
        creation_date: "2026-09-29 12:00+0000",
        ..Default::default()
    });
    let out = pot.write();
    has(&out, "# Copyright (C) 2026, Ada\n");
    has(&out, "\"Project-Id-Version: Demo 1.0\\n\"\n");
    has(&out, "#, fuzzy\nmsgid \"\"\nmsgstr \"\"\n");
    has(&out, "#: ../index.rst:4\n#: ../index.rst:6\n#: ");
    has(&out, "msgid \"Other \\\"quoted\\\".\"\nmsgstr \"\"\n");
    assert_eq!(pot.write(), out); // uuids are stable.
    let mut plain = Template::new("index");
    plain.extract(doc, &ExtractOptions::default());
    lacks(&plain.write(), "#:");
}
