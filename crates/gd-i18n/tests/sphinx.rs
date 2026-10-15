//! Translations that need the Sphinx layer's roles, ignored until guidedoc-sphinx is
//! ported. They read with the plain readers now; once the Sphinx reader exists, read
//! with it ("sphinx", and MyST with Sphinx settings) and drop the `#[ignore]`.
//! tests/shapes.rs covers the same matching with Builder-made cross-references.

mod common;

use common::{Case, has, translate};

#[test]
#[ignore = "needs guidedoc-sphinx (not ported): the :ref: and :py:func: roles"]
fn sphinx_cross_references_keep_their_targets() {
    let text = ".. _intro:\n\nIntro\n=====\n\nSee :ref:`intro` and :py:func:`run`.\n";
    let po = "msgid \"See :ref:`intro` and :py:func:`run`.\"\n\
              msgstr \"Siehe :ref:`Einleitung <intro>` und :py:func:`run`.\"\n";
    let t = translate(Case { reader: "rst", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "diagnostics: {:?}", t.codes);
    has(&t.html, "Siehe ");
    has(&t.html, "Einleitung");
}

#[test]
#[ignore = "needs guidedoc-sphinx (not ported): MyST with Sphinx's {ref} role"]
fn markdown_with_sphinx_roles() {
    let text = "(intro)=\n# Intro\n\nSee {ref}`intro` and *more*.\n";
    let po = "msgid \"See {ref}`intro` and *more*.\"\n\
              msgstr \"Siehe {ref}`Einleitung <intro>` und *mehr*.\"\n";
    let t = translate(Case { reader: "myst", text, po, ..Case::default() });
    has(&t.tree, "\"Siehe \"");
    has(&t.tree, "xref");
    has(&t.tree, "\"Einleitung\"");
    has(&t.tree, "@reftype=\"ref\"");
}

#[test]
#[ignore = "needs guidedoc-sphinx (not ported): py:function and info fields"]
fn info_fields_translate_their_content() {
    let text = ".. py:function:: f(a)\n\n   :param int a: The *a*.\n   :returns: The result.\n\
                \x20  :rtype: int\n";
    let got = common::messages("rst", text, gd_i18n::Targets::default());
    assert!(got.iter().any(|m| m == "The *a*."));
    assert!(got.iter().any(|m| m == "The result."));
    assert!(!got.iter().any(|m| m == "Parameters" || m == "int"));
    let po = "msgid \"The *a*.\"\nmsgstr \"Das *a*.\"\n";
    let t = translate(Case { reader: "rst", text, po, ..Case::default() });
    assert!(t.codes.is_empty(), "diagnostics: {:?}", t.codes);
    has(&t.html, "<strong>a</strong> (");
    has(&t.html, "<em>int</em>");
    has(&t.html, ") – Das <em>a</em>.");
}
