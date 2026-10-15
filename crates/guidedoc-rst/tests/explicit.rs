#![allow(clippy::unwrap_used)]

mod common;

use common::*;

#[test]
fn footnotes_take_every_label_form() {
    expect_dump(
        concat!(
            ".. [1] Manual.\n",
            ".. [#] Auto.\n",
            ".. [#named] Auto labelled.\n",
            ".. [*] Symbol.\n",
        ),
        concat!(
            "document\n",
            "  footnote value=1 \"1\"\n",
            "    paragraph\n",
            "      text \"Manual.\"\n",
            "  footnote value=2 +auto \"#\"\n",
            "    paragraph\n",
            "      text \"Auto.\"\n",
            "  footnote value=3 +auto \"#named\"\n",
            "    paragraph\n",
            "      text \"Auto labelled.\"\n",
            "  footnote value=1 +symbol \"*\"\n",
            "    paragraph\n",
            "      text \"Symbol.\"\n",
        ),
    );
}

#[test]
fn footnote_body_may_start_on_next_line() {
    expect_dump(
        concat!(".. [2]\n", "   Body on the next line.\n", "\n", "   Second paragraph.\n",),
        concat!(
            "document\n",
            "  footnote value=2 \"2\"\n",
            "    paragraph\n",
            "      text \"Body on the next line.\"\n",
            "    paragraph\n",
            "      text \"Second paragraph.\"\n",
        ),
    );
}

#[test]
fn citations_use_simple_names() {
    expect_dump(
        concat!(".. [CIT2002] A citation.\n", ".. [Doe-1.x] Another.\n",),
        concat!(
            "document\n",
            "  citation \"CIT2002\"\n",
            "    paragraph\n",
            "      text \"A citation.\"\n",
            "  citation \"Doe-1.x\"\n",
            "    paragraph\n",
            "      text \"Another.\"\n",
        ),
    );
}

#[test]
fn external_target() {
    expect_dump(
        ".. _Python Home: http://www.python.org\n",
        concat!(
            "document\n",
            "  target \"python home\" link=external dest=\"http://www.python.org\"\n",
        ),
    );
}

#[test]
fn target_uri_spans_lines() {
    expect_dump(
        concat!(".. _long: http://example.com/a/very/\n", "   long/path\n",),
        concat!(
            "document\n",
            "  target \"long\" link=external dest=\"http://example.com/a/very/long/path\"\n",
        ),
    );
}

#[test]
fn target_uri_escaped_whitespace() {
    expect_dump(
        ".. _spaced: file\\ name.txt\n",
        concat!("document\n", "  target \"spaced\" link=external dest=\"file name.txt\"\n",),
    );
}

#[test]
fn target_email_becomes_email_link() {
    expect_dump(
        ".. _mail: someone@example.com\n",
        concat!("document\n", "  target \"mail\" link=email dest=\"someone@example.com\"\n",),
    );
}

#[test]
fn target_name_with_escaped_colon() {
    expect_dump(
        ".. _a\\: b: http://example.com\n",
        concat!("document\n", "  target \"a: b\" link=external dest=\"http://example.com\"\n",),
    );
}

#[test]
fn target_name_quoted_phrase() {
    expect_dump(
        ".. _`phrase: with colon`: http://example.com\n",
        concat!(
            "document\n",
            "  target \"phrase: with colon\" link=external dest=\"http://example.com\"\n",
        ),
    );
}

#[test]
fn target_name_quoted_across_lines() {
    expect_dump(
        concat!(".. _`a long\n", "   phrase`: http://example.com\n",),
        concat!(
            "document\n",
            "  target \"a long phrase\" link=external dest=\"http://example.com\"\n",
        ),
    );
}

#[test]
fn indirect_targets() {
    expect_dump(
        concat!(
            ".. _one: two_\n",
            ".. _three: `Two Words`_\n",
            ".. _two: http://example.com\n",
            ".. _two words: http://example.org\n",
        ),
        concat!(
            "document\n",
            "  target \"one\" link=external dest=\"http://example.com\" name=\"two\"\n",
            "  target \"three\" link=external dest=\"http://example.org\" name=\"two words\"\n",
            "  target \"two\" link=external dest=\"http://example.com\"\n",
            "  target \"two words\" link=external dest=\"http://example.org\"\n",
        ),
    );
}

#[test]
fn internal_target_precedes_element() {
    expect_dump(
        concat!(".. _here:\n", "\n", "Paragraph.\n",),
        concat!(
            "document\n",
            "  target \"here\" link=internal ->2\n",
            "  paragraph\n",
            "    text \"Paragraph.\"\n",
        ),
    );
}

#[test]
fn anonymous_targets_both_forms() {
    expect_dump(
        concat!(
            "See `this`__ and `that`__.\n",
            "\n",
            ".. __: http://one.example.com\n",
            "\n",
            "__ http://two.example.com\n",
        ),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"See \"\n",
            "    reference +anonymous link=external dest=\"http://one.example.com\"\n",
            "      text \"this\"\n",
            "    text \" and \"\n",
            "    reference +anonymous link=external dest=\"http://two.example.com\"\n",
            "      text \"that\"\n",
            "    text \".\"\n",
            "  target +anonymous +referenced link=external dest=\"http://one.example.com\"\n",
            "  target +anonymous +referenced link=external dest=\"http://two.example.com\"\n",
        ),
    );
}

#[test]
fn escaped_trailing_underscore_is_a_uri() {
    expect_dump(
        ".. _file: path\\_\n",
        concat!("document\n", "  target \"file\" link=external dest=\"path_\"\n",),
    );
}

#[test]
fn comments_keep_their_text() {
    expect_dump(
        concat!(
            ".. A comment\n",
            "   on two lines.\n",
            "\n",
            ".. [not a footnote label because of the space\n",
        ),
        concat!(
            "document\n",
            "  comment \"A comment\\non two lines.\"\n",
            "  comment \"[not a footnote label because of the space\"\n",
        ),
    );
}

#[test]
fn empty_comment_ends_a_list() {
    expect_dump(
        concat!("- item\n", "\n", "..\n", "\n", "    block quote\n",),
        concat!(
            "document\n",
            "  list +tight list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"item\"\n",
            "  comment\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"block quote\"\n",
        ),
    );
}

#[test]
fn substitution_replace() {
    expect_dump(
        ".. |name| replace:: replacement text\n",
        concat!(
            "document\n",
            "  substitution-definition \"name\"\n",
            "    text \"replacement text\"\n",
        ),
    );
}

#[test]
fn substitution_directive_on_next_line() {
    expect_dump(
        concat!(".. |name|\n", "   replace:: text on the next line\n",),
        concat!(
            "document\n",
            "  substitution-definition \"name\"\n",
            "    text \"text on the next line\"\n",
        ),
    );
}

#[test]
fn substitution_unicode_codes() {
    expect_dump(
        ".. |codes| unicode:: U+00A9 0x2014 x41 \\x42 \\u0043 &#x44; 69 text .. comment\n",
        concat!(
            "document\n",
            "  substitution-definition \"codes\"\n",
            "    text \"©—ABCDEtext\"\n",
        ),
    );
}

#[test]
fn substitution_unicode_trim() {
    expect_dump(
        concat!(".. |t| unicode:: U+2014\n", "   :ltrim:\n",),
        concat!(
            "document\n",
            "  substitution-definition \"t\" @ltrim=\"1\"\n",
            "    text \"—\"\n",
        ),
    );
}

#[test]
fn substitution_date_formats() {
    expect_dump(
        concat!(".. |d| date:: %A %d %B %Y (%j) %a %b %y %m %%\n", ".. |iso| date::\n",),
        concat!(
            "document\n",
            "  substitution-definition \"d\"\n",
            "    text \"Tuesday 29 September 2026 (272) Tue Sep 26 09 %\"\n",
            "  substitution-definition \"iso\"\n",
            "    text \"2026-09-29\"\n",
        ),
    );
}

#[test]
fn substitution_image_with_target() {
    expect_dump(
        concat!(
            ".. |logo| image:: logo.png\n",
            "   :alt: Logo\n",
            "   :align: top\n",
            "   :target: http://example.com\n",
        ),
        concat!(
            "document\n",
            "  substitution-definition \"logo\"\n",
            "    link link=external dest=\"http://example.com\"\n",
            "      image link=external dest=\"logo.png\" @alt=\"Logo\" @align=\"top\"\n",
        ),
    );
}

#[test]
fn substitution_raw() {
    expect_dump(
        concat!(".. |br| raw:: html\n", "\n", "   <br>\n",),
        concat!(
            "document\n",
            "  substitution-definition \"br\"\n",
            "    raw-inline \"<br>\\n\" @format=\"html\"\n",
        ),
    );
}

#[test]
fn substitution_names_are_normalized() {
    expect_dump(
        ".. |Two  Words| replace:: x\n",
        concat!("document\n", "  substitution-definition \"two words\"\n", "    text \"x\"\n",),
    );
}

#[test]
fn substitution_needs_a_directive() {
    expect_codes(".. |bad| just text\n", "rst.substitution.invalid");
}

#[test]
fn substitution_rejects_block_directives() {
    expect_codes(".. |bad| note:: x\n", "rst.substitution.directive");
}

#[test]
fn substitution_replace_single_paragraph() {
    expect_codes(
        concat!(".. |bad| replace:: one\n", "\n", "   two\n",),
        "rst.substitution.replace",
    );
}

#[test]
fn malformed_target_is_error() {
    expect_codes(".. __name: http://example.com\n", "rst.target.malformed");
}

#[test]
fn unicode_invalid_code_is_error() {
    expect_codes(".. |bad| unicode:: 0x110000\n", "rst.unicode.invalid");
}

#[test]
fn date_without_today_warns() {
    let mut r = request_for(".. |d| date:: %Y\n");
    r.read.today = "";
    let got = read_request(&r);
    assert!(got.ok);
    assert_eq!(got.dump, "document\n  substitution-definition \"d\"\n    text \"%Y\"\n");
    expect_codes_in(&got.codes, "rst.date.today");
}
