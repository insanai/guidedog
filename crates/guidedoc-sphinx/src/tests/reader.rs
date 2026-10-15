//! Tests of the sphinx reader: settings, documents and reports.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

#[test]
fn prolog_and_epilog_surround_the_document() {
    let mut config = Config::default();
    config.rst_prolog = ".. |project| replace:: Demo\n".into();
    config.rst_epilog = "Epilogue for |project|.\n".into();
    expect_dump(
        "Title\n=====\n\nBody |project|.\n",
        concat!(
            "document value=1\n",
            "  substitution-definition \"project\"\n    text \"Demo\"\n",
            "  section value=1\n    heading value=1\n      text \"Title\"\n",
            "    paragraph\n      text \"Body \"\n",
            "      substitution-reference \"project\" link=internal name=\"project\" ->1\n",
            "      text \".\"\n",
            "    paragraph\n      text \"Epilogue for \"\n",
            "      substitution-reference \"project\" link=internal name=\"project\" ->1\n",
            "      text \".\"\n",
        ),
        Some(&config),
    );
}

#[test]
fn reader_metadata() {
    let r = crate::reader();
    assert_eq!(r.id, "sphinx");
    assert_eq!(r.version, crate::VERSION);
    assert!(r.read_inline.is_some());
    let config = Config::default();
    assert_eq!(config.primary_domain, "py");
    assert_eq!(config.highlight_language, "default");
    assert!(config.add_function_parentheses && config.add_module_names);
    assert!(config.settings.docinfo);
}

// CPython's own directives and roles come from its Python extensions; like Sphinx, a
// policy that keeps documents with errors reports them and drops only the construct.
#[test]
fn unknown_constructs_keep_the_document_under_the_policy() {
    let text = "Intro.\n\n.. availability:: Unix.\n\nSee :gh:`123` here.\n";
    let mut request = guidedoc::default_request("test.rst", text, "sphinx", "html");
    request.read.promote_title = false;
    let refused = read_request(&request, &crate::READER);
    assert!(!refused.ok);
    request.policy.keep_documents_with_errors = true;
    let r = read_request(&request, &crate::READER);
    assert!(r.ok, "{}", r.codes);
    assert!(r.codes.starts_with("rst.directive.unknown"), "{}", r.codes);
    assert_eq!(r.codes.split(' ').count(), 2, "{}", r.codes);
    assert_eq!(
        r.dump,
        concat!(
            "document value=1\n  paragraph\n    text \"Intro.\"\n",
            "  paragraph\n    text \"See \"\n    text \":gh:`123`\"\n    text \" here.\"\n",
        )
    );
}

// rst_prolog goes after a document's leading field lines, as Sphinx's _prepend_prologue
// puts it (tests/test_util/test_util_rst.py): the fields stay bibliographic, and a
// section title that starts with a role is not taken for a field.
#[test]
fn prolog_follows_the_leading_fields() {
    let mut config =
        Config { rst_prolog: "this is rst_prolog\nhello reST!".into(), ..Config::default() };
    expect_contains(
        ":title: test of SphinxFileInput\n:author: Sphinx team\n\nhello Sphinx world\n",
        &[
            "test of SphinxFileInput",
            "Sphinx team",
            "text \"this is rst_prolog\"",
            "text \"hello reST!\"",
            "text \"hello Sphinx world\"",
        ],
        Some(&config),
    );
    expect_contains(
        ":title: test\n\n:mod:`foo`\n----------\n\nhello\n",
        &["text \"test\"", "text \"this is rst_prolog", "section", "text \"hello\""],
        Some(&config),
    );
    for prolog in ["this is rst_prolog\nhello reST!\n", "this is rst_prolog"] {
        config.rst_prolog = prolog.into();
        expect_contains(
            ":mod:`foo`\n----------\n\nhello\n",
            &["paragraph\n    text \"this is rst_prolog", "section", "text \"hello\""],
            Some(&config),
        );
    }
}

// Sphinx 8.1's :cve: and :cwe: roles (tests/test_markup/test_markup.py): an index entry,
// and a strong link; a CWE is read as a number.
#[test]
fn cve_and_cwe_roles_follow_sphinx() {
    expect_contains(
        ":cve:`2020-10735` and :cwe:`0787#top`\n",
        &[
            "@entries=\"single: Common Vulnerabilities and Exposures; CVE 2020-10735\"",
            "dest=\"https://www.cve.org/CVERecord?id=CVE-2020-10735\" @class=\"cve\"",
            "text \"CVE 2020-10735\"",
            "@entries=\"single: Common Weakness Enumeration; CWE 0787#top\"",
            "https://cwe.mitre.org/data/definitions/787.html#top",
            "text \"CWE 0787#top\"",
        ],
        None,
    );
    expect_contains(":cve:`Title <2020-1>`\n", &["text \"Title\""], None);
    let r = read_text(":cwe:`x`\n", None);
    assert!(r.codes.contains("sphinx.role.cwe"), "{}", r.codes);
    assert!(!r.dump.contains("index"), "{}", r.dump);
}

// :py:deco: shows "@" before the title and refers to the decorator's function.
#[test]
fn deco_role_follows_sphinx() {
    expect_contains(
        ".. py:module:: m\n\nUse :deco:`wraps` and :py:deco:`~m.cached`.\n",
        &["@reftype=\"py:deco\"", "code \"@wraps\"", "code \"@cached\""],
        None,
    );
}
