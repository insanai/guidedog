//! The reader as a whole: its description, empty input, byte order marks, title promotion and wide
//! characters.

#![allow(clippy::unwrap_used)]

mod common;

use common::*;
use guidedoc::{Arg, Conformance, Workspace, read_document};
use guidedoc_rst::{CONFORMANCE, VERSION, reader};

#[test]
fn reader_describes_itself() {
    let r = reader();
    assert_eq!(r.id, "rst");
    assert_eq!(r.name, "reStructuredText");
    assert_eq!(r.version, VERSION);
    assert_eq!(r.conformance, Conformance::Partial);
    assert_eq!(CONFORMANCE, Conformance::Partial);
    assert!(r.read_inline.is_some());
}

#[test]
fn empty_input_reads_to_an_empty_document() {
    expect_dump("", "document\n");
    expect_dump("\n\n   \n", "document\n");
}

#[test]
fn byte_order_mark_is_skipped() {
    expect_dump("\u{feff}Text.\n", "document\n  paragraph\n    text \"Text.\"\n");
}

#[test]
fn default_read_options_promote_the_title() {
    let mut r = request_for("Title\n=====\n\nBody.\n");
    r.read = guidedoc::default_read_options();
    let got = read_request(&r);
    assert!(got.ok, "{}", got.codes);
}

// East Asian wide characters take two columns, as in Docutils, so tables drawn in an
// editor with a CJK font line up.

#[test]
fn wide_characters_fill_two_columns_in_grid_tables() {
    let r =
        read_text("+------+----+\n| 漢字 | ab |\n+------+----+\n| 한글 | cd |\n+------+----+\n");
    assert!(r.ok);
    assert_eq!(r.codes, "");
    for t in ["text \"漢字\"", "text \"한글\"", "text \"cd\""] {
        assert!(r.dump.contains(t), "{}", r.dump);
    }
}

#[test]
fn wide_characters_fill_two_columns_in_simple_tables() {
    let r = read_text("====  ====\n日本  en\n中文  zh\n====  ====\n");
    assert!(r.ok);
    assert_eq!(r.codes, "");
    assert!(r.dump.contains("text \"日本\""));
    assert!(r.dump.contains("text \"zh\""));
}

#[test]
fn wide_titles_need_underlines_of_their_display_width() {
    assert_eq!(read_text("漢字の題\n========\n\nBody.\n").codes, "");
    assert!(read_text("漢字の題\n====\n\nBody.\n").codes.contains("rst.section"));
}

const SPECIALIZED: &str = r#"document
  paragraph
    reference link=external dest="http://x.org" name="x"
      text "x"
    text " "
    reference +anonymous link=external dest="http://anon.org"
      text "a b"
    text " "
    footnote-reference value=1 "1" link=internal ->24
    soft-break
    citation-reference "CIT" link=internal ->27
    text " "
    substitution-reference "s" link=internal name="s" ->30
    text " "
    inline-target +referenced "t t"
      text "t t"
    soft-break
    link link=external dest="http://a.b/c"
      text "http://a.b/c"
    text " "
    reference link=internal name="t t" ->14
      text "t t"
  target +referenced "x" link=external dest="http://x.org"
  target +anonymous +referenced link=external dest="http://anon.org"
  footnote value=1 +referenced "1"
    paragraph
      text "one"
  citation +referenced "CIT"
    paragraph
      text "cit"
  substitution-definition "s"
    text "sub"
"#;

#[test]
fn specialized_roles_name_their_constructs() {
    expect_dump(
        concat!(
            ":named-reference:`x` :anonymous-reference:`a b` :footnote-reference:`1`\n",
            ":citation-reference:`CIT` :substitution-reference:`s` :target:`t t`\n",
            ":uri-reference:`http://a.b/c` `t t`_\n\n",
            ".. _x: http://x.org\n.. __: http://anon.org\n.. [1] one\n.. [CIT] cit\n",
            ".. |s| replace:: sub\n",
        ),
        SPECIALIZED,
    );
}

/// The source text the first report with `code` quotes in its two arguments.
fn quoted_args(text: &str, code: &str) -> Option<(String, String)> {
    let mut ws = Workspace::new();
    let request = request_for(text);
    let _ = read_document(&mut ws, &reader(), &request);
    let d = ws.reports().items().iter().find(|d| d.code == code)?.clone();
    let quote = |a: &Arg| match a {
        Arg::Span(s) => text[s.start as usize..s.end as usize].to_owned(),
        _ => String::new(),
    };
    Some((quote(&d.args[0]), quote(&d.args[1])))
}

#[test]
fn shadowed_section_name_quotes_the_title() {
    let text = ".. _installing: https://example.org\n\nInstalling\n==========\n\nText.\n";
    let (first, second) = quoted_args(text, "rst.target.implicit").unwrap();
    assert_eq!(first, "Installing");
    assert_eq!(second, ".. _installing: https://example.org");
}

#[test]
fn target_before_its_section_is_not_reported() {
    assert_eq!(read_text(".. _installing:\n\nInstalling\n==========\n\nText.\n").codes, "");
}

#[test]
fn duplicate_targets_quote_their_first_lines() {
    let text = ".. _a: https://one.example\n   /more\n\n.. _a: https://two.example\n";
    let (first, second) = quoted_args(text, "rst.target.duplicate").unwrap();
    assert_eq!(first, ".. _a: https://two.example");
    assert_eq!(second, ".. _a: https://one.example");
}

// Duplicate names follow Docutils' set_duplicate_name_id; each case was checked
// against Docutils' pseudo-XML.
#[test]
fn repeated_targets_with_one_destination_stay_usable() {
    // The same URI twice is only a note; the name stays usable.
    let r = read_text(concat!(
        "`IEEE <ieee 754 standard_>`_ and `ieee 754 standard`_.\n\n",
        ".. _ieee 754 standard: https://a.org/754\n\n",
        ".. _ieee 754 standard: https://a.org/754\n",
    ));
    assert_eq!(r.codes, "rst.target.repeated");
    assert_eq!(r.dump.matches("link=external dest=\"https://a.org/754\"").count(), 4);
    // A different URI after them makes the name unusable, and a later repetition of the
    // first URI cannot make it usable again.
    let r = read_text(concat!(
        "`c`_\n\n.. _c: https://a.org/1\n\n.. _c: https://a.org/1\n\n",
        ".. _c: https://a.org/2\n\n.. _c: https://a.org/1\n",
    ));
    assert_eq!(
        r.codes,
        "rst.target.repeated rst.target.duplicate rst.target.ambiguous rst.reference.ambiguous"
    );
    // Two indirect targets naming one target agree too.
    let r = read_text("`a`_\n\n.. _a: x_\n\n.. _a: x_\n\n.. _x: https://a.org/x\n");
    assert_eq!(r.codes, "rst.target.repeated");
}

#[test]
fn embedded_aliases_are_implicit_targets() {
    // CPython's library/array.rst: the alias's text names the target it refers to.
    let r = read_text(concat!(
        "Revision of the `IEEE 754 standard <ieee 754 standard_>`_.\n\n",
        ".. _ieee 754 standard: https://a.org/754\n",
    ));
    assert_eq!(r.codes, "rst.target.implicit");
    assert!(r.dump.contains("link=external dest=\"https://a.org/754\""), "{}", r.dump);
    // An explicit target of another URI takes the alias's name.
    let r = read_text(concat!(
        "`ieee <x_>`_ and `ieee`_.\n\n.. _x: https://a.org/x\n\n",
        ".. _ieee: https://b.org/other\n",
    ));
    assert_eq!(r.codes, "rst.target.implicit");
    assert_eq!(r.dump.matches("dest=\"https://b.org/other\"").count(), 2);
    // A section title and an alias are two implicit names: neither is usable.
    let r = read_text("`a`_\n\nA\n=\n\n`a <x_>`_\n\n.. _x: https://a.org/x\n");
    assert_eq!(r.codes, "rst.target.implicit rst.reference.ambiguous");
}
