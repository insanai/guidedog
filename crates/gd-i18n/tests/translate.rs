//! The translation pass over documents read by the reStructuredText and CommonMark
//! readers.

mod common;

use common::{Case, has, lacks, translate};
use gd_i18n::{Progress, Target, Targets};

const DOC: &str = "Getting started
===============

Read the guide_ and the *notes* [#]_.

.. _guide: https://example.org/guide

.. [#] A footnote.

Details
-------

See Details_ above. Use |name| here.

.. |name| replace:: the name

- An item.

::

   print(\"hello\")

.. figure:: logo.png
   :alt: The logo

   The caption.
";

const DE: &str = r#"msgid "Getting started"
msgstr "Erste Schritte"

msgid "Read the guide_ and the *notes* [#]_."
msgstr "Lies die *Anleitung* guide_ und die **Notizen** [#]_."

msgid "Details"
msgstr "Einzelheiten"

msgid "See Details_ above. Use |name| here."
msgstr "Siehe Details_ oben. Nimm |name| hier."

msgid "An item."
msgstr "Ein Punkt."

msgid "A footnote."
msgstr "Eine Fußnote."

msgid "print(\"hello\")"
msgstr "print(\"hallo\")"

msgid "The logo"
msgstr "Das Logo"

msgid "The caption."
msgstr "Die Bildunterschrift."
"#;

fn rst(po: &str) -> Case<'_> {
    Case { reader: "rst", text: DOC, po, ..Case::default() }
}

#[test]
fn translates_paragraphs_with_markup() {
    let t = translate(rst(DE));
    assert!(t.codes.is_empty(), "diagnostics: {:?}", t.codes);
    has(&t.html, "Erste Schritte");
    has(
        &t.html,
        "Lies die <em>Anleitung</em> <a href=\"https://example.org/guide\">guide</a> und die \
         <strong>Notizen</strong>",
    );
    has(&t.html, "Eine Fußnote.");
    has(&t.html, "Ein Punkt.");
    has(&t.html, "Die Bildunterschrift.");
    has(&t.html, "the name hier.");
    lacks(&t.html, "Read the");
    lacks(&t.html, "hallo"); // literal blocks need the literal-block target,
    lacks(&t.html, "Das Logo"); // and alt text the image target.
    assert!(t.counts.translated >= 7, "{:?}", t.counts);
    assert_eq!(t.counts.kept, 0);
}

#[test]
fn titles_keep_their_anchors() {
    let t = translate(rst(DE));
    has(&t.html, "id=\"details\"");
    // The reference to the section by its title still reaches it.
    has(&t.html, "href=\"#details\"");
}

#[test]
fn additional_targets_translate_literals_and_alt() {
    let targets = Targets::of(&[Target::LiteralBlock, Target::Image]);
    let t = translate(Case { targets, ..rst(DE) });
    has(&t.html, "print(&quot;hallo&quot;)");
    has(&t.html, "alt=\"Das Logo\"");
}

#[test]
fn dropped_references_warn() {
    let po = "msgid \"See Details_ above. Use |name| here.\"\nmsgstr \"Siehe oben.\"\n";
    let t = translate(rst(po));
    assert!(t.codes.contains(&"i18n.references.inconsistent"), "{:?}", t.codes);
    has(&t.html, "Siehe oben.");
    assert_eq!(t.counts.translated, 1);
}

#[test]
fn unknown_references_keep_the_original() {
    let po = "msgid \"See Details_ above. Use |name| here.\"\n\
              msgstr \"Siehe Elsewhere_ oben. Nimm |name| hier.\"\n";
    let t = translate(rst(po));
    assert!(t.codes.contains(&"i18n.references.unknown"), "{:?}", t.codes);
    has(&t.html, "See <a");
    lacks(&t.html, "Siehe");
    assert_eq!(t.counts.kept, 1);
}

#[test]
fn broken_markup_keeps_the_original() {
    let po = "msgid \"An item.\"\nmsgstr \"Ein :unbekannt:`Punkt`.\"\n";
    let t = translate(rst(po));
    assert!(t.codes.contains(&"i18n.translation.markup"), "{:?}", t.codes);
    has(&t.html, "An item.");
    assert_eq!(t.counts.kept, 1);
}

#[test]
fn fuzzy_translations_are_skipped() {
    let t = translate(rst("#, fuzzy\nmsgid \"An item.\"\nmsgstr \"Ein Punkt.\"\n"));
    has(&t.html, "An item.");
}

#[test]
fn untouched_documents_are_unchanged() {
    let t = translate(rst("msgid \"Other\"\nmsgstr \"Andere\"\n"));
    assert!(t.codes.is_empty(), "{:?}", t.codes);
    assert_eq!(t.counts.translated, 0);
    has(&t.html, "Getting started");
}

#[test]
fn markdown_is_reparsed_as_markdown() {
    let text = "# Title\n\nSome *text* with [a link](https://example.org).\n";
    let po = "msgid \"Title\"\nmsgstr \"Titel\"\n\n\
              msgid \"Some *text* with [a link](https://example.org).\"\n\
              msgstr \"Etwas **Text** mit [einem Link](https://example.org).\"\n";
    let t = translate(Case { reader: "commonmark", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "diagnostics: {:?}", t.codes);
    has(&t.html, "Titel");
    has(&t.html, "Etwas <strong>Text</strong> mit <a");
    has(&t.html, "einem Link</a>");
    has(&t.html, "id=\"title\"");
}

#[test]
fn progress_classes_mark_nodes() {
    let po = "msgid \"One.\"\nmsgstr \"Eins.\"\n";
    let both = translate(Case {
        reader: "rst",
        text: "One.\n\nTwo.\n",
        po,
        progress: Progress::ALL,
        ..Case::default()
    });
    has(&both.tree, "\"Eins.\"");
    has(&both.tree, "@class=\"translated\"");
    has(&both.tree, "@class=\"untranslated\"");
    let progress = Progress { untranslated: true, ..Progress::default() };
    let none =
        translate(Case { reader: "rst", text: "Two.\n", po: "", progress, ..Case::default() });
    has(&none.tree, "@class=\"untranslated\"");
}

#[test]
fn images_follow_figure_language_filename() {
    let mut ws = guidedoc::Workspace::new();
    let text = ".. image:: img/logo.png\n\n.. image:: img/other.png\n";
    common::read(&mut ws, "rst", text, guidedoc::ReadOptions::default());
    let exists = |_: &str, path: &str| path == "img/logo.de.png";
    let images = gd_i18n::ImageLocalization {
        language: "de",
        docname: "index",
        exists: Some(&exists),
        ..Default::default()
    };
    let translation = gd_i18n::Translation { images, ..Default::default() };
    let (doc, _) = gd_i18n::translate_document(&mut ws, translation)
        .unwrap_or_else(|f| panic!("{}", f.diagnostic.formatted_message()));
    let tree = guidedoc::dump_to_string(doc);
    has(&tree, "img/logo.de.png");
    has(&tree, "img/other.png");
}
