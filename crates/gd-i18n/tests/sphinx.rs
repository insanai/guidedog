//! Translations that need the Sphinx layer's roles: read with the "sphinx" reader, and
//! MyST with the Sphinx layer's settings. tests/shapes.rs covers the same matching with
//! Builder-made cross-references.

mod common;

use common::{Case, has, translate};

#[test]
fn sphinx_cross_references_keep_their_targets() {
    let text = ".. _intro:\n\nIntro\n=====\n\nSee :ref:`intro` and :py:func:`run`.\n";
    let po = "msgid \"See :ref:`intro` and :py:func:`run`.\"\n\
              msgstr \"Siehe :ref:`Einleitung <intro>` und :py:func:`run`.\"\n";
    let t = translate(Case { reader: "sphinx", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "diagnostics: {:?}", t.codes);
    has(&t.html, "Siehe ");
    has(&t.html, "Einleitung");
}

#[test]
fn markdown_with_sphinx_roles() {
    let text = "(intro)=\n# Intro\n\nSee {ref}`intro` and *more*.\n";
    let po = "msgid \"See {ref}`intro` and *more*.\"\n\
              msgstr \"Siehe {ref}`Einleitung <intro>` und *mehr*.\"\n";
    let t = translate(Case { reader: "myst-sphinx", text, po, ..Case::default() });
    has(&t.tree, "\"Siehe \"");
    has(&t.tree, "xref");
    has(&t.tree, "\"Einleitung\"");
    has(&t.tree, "@reftype=\"ref\"");
}

#[test]
fn info_fields_translate_their_content() {
    let text = ".. py:function:: f(a)\n\n   :param int a: The *a*.\n   :returns: The result.\n\
                \x20  :rtype: int\n";
    let got = common::messages("sphinx", text, gd_i18n::Targets::default());
    assert!(got.iter().any(|m| m == "The *a*."));
    assert!(got.iter().any(|m| m == "The result."));
    assert!(!got.iter().any(|m| m == "Parameters" || m == "int"));
    let po = "msgid \"The *a*.\"\nmsgstr \"Das *a*.\"\n";
    let t = translate(Case { reader: "sphinx", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "diagnostics: {:?}", t.codes);
    has(&t.html, "<strong>a</strong> (");
    has(&t.html, "<em>int</em>");
    has(&t.html, ") – Das <em>a</em>.");
}

#[test]
fn translated_glossary_terms_keep_their_index_metadata() {
    let text = ".. glossary::\n\n   *Admission* : category\n      A definition.\n\n\
                \x20  Gate\n      Another definition.\n";
    for word in ["許可", "许可", "허가", "अनुमति", "السماح", "Zulassung"] {
        let po = format!("msgid \"*Admission*\"\nmsgstr \"**{word}**\"\n");
        let t = translate(Case { reader: "sphinx", text, po: &po, ..Case::default() });
        assert!(t.codes.is_empty(), "{:?}", t.codes);
        has(&t.html, "id=\"term-Admission\"");
        has(&t.html, &format!("<strong>{word}</strong>"));
        has(&t.tree, &format!("@entries=\"!single: {word}\""));
        has(&t.tree, "@key=\"category\" @targets=\"term-Admission\"");
        has(&t.tree, "@entries=\"!single: Gate\" @targets=\"term-Gate\"");
        assert_eq!(t.tree.matches("@targets=\"term-Admission\"").count(), 1);
    }
}

#[test]
fn translated_inline_indexes_keep_distinct_original_targets() {
    let text = "One :envvar:`FIRST`.\n\nTwo :envvar:`SECOND`.\n";
    let po = "msgid \"One :envvar:`FIRST`.\"\nmsgstr \"Un :envvar:`FIRST`.\"\n\
              msgid \"Two :envvar:`SECOND`.\"\nmsgstr \"Deux :envvar:`SECOND`.\"\n";
    let t = translate(Case { reader: "sphinx", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "{:?}", t.codes);
    has(&t.html, "id=\"index-0\"");
    has(&t.html, "id=\"index-1\"");
    assert_eq!(t.html.matches("id=\"index-0\"").count(), 1);
}

#[test]
fn translated_index_roles_keep_ids_when_reordered_or_renamed() {
    let text = "One :index:`Alpha` and :pep:`8`.\n\nTwo :index:`Beta` and :rfc:`822`.\n";
    let po = "msgid \"One :index:`Alpha` and :pep:`8`.\"\n\
              msgstr \"Un :pep:`8` et :index:`Alfa`.\"\n\
              msgid \"Two :index:`Beta` and :rfc:`822`.\"\n\
              msgstr \"Deux :rfc:`822` et :index:`Bêta`.\"\n";
    let t = translate(Case { reader: "sphinx", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "{:?}", t.codes);
    for (id, entry) in [(0, "single: Alfa"), (2, "single: Bêta")] {
        has(&t.tree, &format!("index @id=\"index-{id}\" @entries=\"{entry}\""));
    }
    for id in 0..4 {
        assert_eq!(t.html.matches(&format!("id=\"index-{id}\"")).count(), 1, "{}", t.html);
    }
}

#[test]
fn refused_glossary_translations_keep_original_entries_once() {
    let text = ".. glossary::\n\n   Admission\n      A definition.\n";
    let po = "msgid \"Admission\"\nmsgstr \":ref:`unknown`\"\n";
    let t = translate(Case { reader: "sphinx", text, po, ..Case::default() });
    assert_eq!(t.codes, ["i18n.references.unknown"]);
    has(&t.tree, "@entries=\"!single: Admission\"");
    assert_eq!(t.tree.matches("@targets=\"term-Admission\"").count(), 1);
}

#[test]
fn myst_glossary_translations_keep_index_metadata() {
    let text = "```{glossary}\n*Admission*\n   A definition.\n```\n";
    let po = "msgid \"*Admission*\"\nmsgstr \"**許可**\"\n";
    let t = translate(Case { reader: "myst-sphinx", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "{:?}", t.codes);
    has(&t.tree, "@entries=\"!single: 許可\" @targets=\"term-Admission\"");
    has(&t.html, "<strong>許可</strong>");
}

#[test]
fn translations_cannot_add_unmatched_index_identities() {
    let text = "One :index:`Alpha`.\n";
    let po = "msgid \"One :index:`Alpha`.\"\n\
              msgstr \"Un :index:`Alfa` et :index:`Beta`.\"\n";
    let t = translate(Case { reader: "sphinx", text, po, ..Case::default() });
    assert_eq!(t.codes, ["i18n.references.unknown"]);
    has(&t.tree, "@entries=\"single: Alpha\"");
    assert_eq!(t.html.matches("id=\"index-0\"").count(), 1);
    assert!(!t.tree.contains("single: Beta"));
}
