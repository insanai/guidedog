//! Reader settings: tab expansion, PEP and RFC references, smart quotes, substitutions and docinfo
//! fields.

#![allow(clippy::unwrap_used)]

mod common;

use common::*;

#[test]
fn tabs_in_inline_text_expand_to_tab_stops() {
    expect_dump_with(
        "a\tb *c\td* ``e\tf``\n",
        &every_setting(),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"a       b \"\n",
            "    emphasis\n",
            "      text \"c    d\"\n",
            "    text \" \"\n",
            "    code \"e  f\"\n",
        ),
        false,
    );
}

#[test]
fn implicit_pep_and_rfc_references_and_trimmed_space() {
    expect_dump_with(
        concat!("See PEP 8 and RFC\n", "2822 [#]_.\n", "\n", ".. [#] n\n",),
        &every_setting(),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"See \"\n",
            "    link link=external dest=\"https://peps.python.org/pep-0008/\"\n",
            "      text \"PEP 8\"\n",
            "    text \" and \"\n",
            "    link link=external dest=\"https://datatracker.ietf.org/doc/html/rfc2822\"\n",
            "      text \"RFC\"\n",
            "      soft-break\n",
            "      text \"2822\"\n",
            "    footnote-reference value=1 +auto \"#\" link=internal ->12\n",
            "    text \".\"\n",
            "  footnote value=1 +auto +referenced \"#\"\n",
            "    paragraph\n",
            "      text \"n\"\n",
        ),
        false,
    );
}

#[test]
fn smart_quotes_dashes_and_ellipses() {
    expect_dump_with(
        "\"Hi\" -- 'it's' ... ``\"x\"``\n",
        &every_setting(),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"“Hi” – ‘it’s’ … \"\n",
            "    code \"\\\"x\\\"\"\n",
        ),
        false,
    );
}

#[test]
fn substitution_names_may_span_lines() {
    expect_dump_with(
        concat!("A |long\n", "name|.\n", "\n", ".. |long\n", "   name| replace:: x\n",),
        &every_setting(),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"A \"\n",
            "    substitution-reference \"long name\" link=internal name=\"long name\" ->5\n",
            "    text \".\"\n",
            "  substitution-definition \"long name\"\n",
            "    text \"x\"\n",
        ),
        false,
    );
}

#[test]
fn bibliographic_fields_become_docinfo() {
    expect_dump_with(
        concat!(
            "T\n",
            "=\n",
            "\n",
            ":Author: A\n",
            ":Authors: B; C\n",
            ":Date: $Date: 2002/08/27 13:04:05 $\n",
            ":Dedication: D\n",
            ":Extra: e\n",
        ),
        &every_setting(),
        concat!(
            "document\n",
            "  title\n",
            "    text \"T\"\n",
            "  field-list @class=\"docinfo\"\n",
            "    field @docinfo=\"author\"\n",
            "      field-name\n",
            "        text \"Author\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"A\"\n",
            "    field @docinfo=\"authors\"\n",
            "      field-name\n",
            "        text \"Authors\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"B\"\n",
            "        paragraph\n",
            "          text \"C\"\n",
            "    field @docinfo=\"date\"\n",
            "      field-name\n",
            "        text \"Date\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"2002-08-27\"\n",
            "    field @class=\"extra\"\n",
            "      field-name\n",
            "        text \"Extra\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"e\"\n",
            "  topic @class=\"dedication\"\n",
            "    title\n",
            "      text \"Dedication\"\n",
            "    paragraph\n",
            "      text \"D\"\n",
        ),
        true,
    );
}

#[test]
fn lone_subsection_becomes_section_subtitle() {
    expect_dump_with(
        concat!("A\n", "=\n", "\n", "B\n", "-\n", "\n", "text\n",),
        &every_setting(),
        concat!(
            "document\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"A\"\n",
            "    paragraph @class=\"section-subtitle\"\n",
            "      text \"B\"\n",
            "    paragraph\n",
            "      text \"text\"\n",
        ),
        false,
    );
}

#[test]
fn unicode_trailing_white_space_is_stripped() {
    expect_dump_with(
        "x \n",
        &every_setting(),
        concat!("document\n", "  paragraph\n", "    text \"x\"\n",),
        false,
    );
}

#[test]
fn figure_width_of_image_and_normalized_lengths() {
    expect_dump_with(
        concat!(".. figure:: a.png\n", "   :figwidth: image\n", "   :width: 3 em\n",),
        &every_setting(),
        concat!(
            "document\n",
            "  figure @figwidth=\"image\"\n",
            "    image link=external dest=\"a.png\" @width=\"3em\"\n",
        ),
        false,
    );
}

#[test]
fn unlabelled_footnote_numbers_are_names() {
    expect_dump_with(
        concat!("`2`_ [2]_\n", "\n", ".. [1] a\n", ".. [#] b\n",),
        &every_setting(),
        concat!(
            "document\n",
            "  paragraph\n",
            "    reference link=internal name=\"2\" ->8\n",
            "      text \"2\"\n",
            "    footnote-reference value=2 \"2\" link=internal name=\"2\" ->8\n",
            "  footnote value=1 \"1\"\n",
            "    paragraph\n",
            "      text \"a\"\n",
            "  footnote value=2 +auto +referenced \"#\"\n",
            "    paragraph\n",
            "      text \"b\"\n",
        ),
        false,
    );
}

#[test]
fn chained_target_takes_the_uri() {
    expect_dump_with(
        concat!("x_\n", "\n", ".. _x:\n", ".. _y: http://y.org\n",),
        &every_setting(),
        concat!(
            "document\n",
            "  paragraph\n",
            "    reference link=external dest=\"http://y.org\" name=\"x\"\n",
            "      text \"x\"\n",
            "  target +referenced \"x\" link=external dest=\"http://y.org\"\n",
            "  target \"y\" link=external dest=\"http://y.org\"\n",
        ),
        false,
    );
}
