//! Directives: admonitions, images and figures, and the other standard directives with their
//! options.

#![allow(clippy::unwrap_used)]

mod common;

use common::*;

#[test]
fn specific_admonitions() {
    expect_dump(
        concat!(
            ".. attention:: a\n",
            ".. caution:: b\n",
            ".. danger:: c\n",
            ".. error:: d\n",
            ".. hint:: e\n",
            ".. important:: f\n",
            ".. note:: g\n",
            ".. tip:: h\n",
            ".. warning:: i\n",
            ".. seealso:: j\n",
        ),
        concat!(
            "document\n",
            "  admonition value=1\n",
            "    paragraph\n",
            "      text \"a\"\n",
            "  admonition value=2\n",
            "    paragraph\n",
            "      text \"b\"\n",
            "  admonition value=3\n",
            "    paragraph\n",
            "      text \"c\"\n",
            "  admonition value=4\n",
            "    paragraph\n",
            "      text \"d\"\n",
            "  admonition value=5\n",
            "    paragraph\n",
            "      text \"e\"\n",
            "  admonition value=6\n",
            "    paragraph\n",
            "      text \"f\"\n",
            "  admonition value=7\n",
            "    paragraph\n",
            "      text \"g\"\n",
            "  admonition value=8\n",
            "    paragraph\n",
            "      text \"h\"\n",
            "  admonition value=9\n",
            "    paragraph\n",
            "      text \"i\"\n",
            "  admonition value=10\n",
            "    paragraph\n",
            "      text \"j\"\n",
        ),
    );
}

#[test]
fn admonition_content_and_options() {
    expect_dump(
        concat!(
            ".. note:: First line of content\n",
            "   continues here.\n",
            "   :class: wide\n",
            "   :name: my note\n",
            "\n",
            "   Second paragraph.\n",
        ),
        concat!(
            "document\n",
            "  admonition value=7 @class=\"wide\" @name=\"my note\"\n",
            "    paragraph\n",
            "      text \"First line of content\"\n",
            "      soft-break\n",
            "      text \"continues here.\"\n",
            "    paragraph\n",
            "      text \"Second paragraph.\"\n",
        ),
    );
}

#[test]
fn generic_admonition_gets_title_class() {
    expect_dump(
        concat!(
            ".. admonition:: And, by the way...\n",
            "\n",
            "   You can make up your own admonition too.\n",
        ),
        concat!(
            "document\n",
            "  admonition @class=\"admonition-and-by-the-way\"\n",
            "    title\n",
            "      text \"And, by the way...\"\n",
            "    paragraph\n",
            "      text \"You can make up your own admonition too.\"\n",
        ),
    );
}

#[test]
fn admonition_needs_content() {
    expect_codes(".. note::\n", "rst.directive.content.missing");
}

#[test]
fn image_with_all_options() {
    expect_dump(
        concat!(
            ".. image:: images/picture.png\n",
            "   :alt: alternate text\n",
            "   :height: 100px\n",
            "   :width: 50%\n",
            "   :scale: 50 %\n",
            "   :align: right\n",
            "   :loading: lazy\n",
            "   :class: photo\n",
            "   :name: pic\n",
        ),
        concat!(
            "document\n",
            "  image link=external dest=\"images/picture.png\" @alt=\"alternate text\"",
            " @width=\"50%\" @height=\"100px\" @scale=\"50\" @align=\"right\"",
            " @loading=\"lazy\" @class=\"photo\" @name=\"pic\"\n",
        ),
    );
}

#[test]
fn image_uri_joins_lines() {
    expect_dump(
        concat!(".. image:: images/very/\n", "   long.png\n",),
        concat!("document\n", "  image link=external dest=\"images/very/long.png\"\n",),
    );
}

#[test]
fn image_target_reference() {
    expect_dump(
        concat!(
            ".. image:: logo.png\n",
            "   :target: home_\n",
            "\n",
            ".. _home: http://example.com\n",
        ),
        concat!(
            "document\n",
            "  paragraph\n",
            "    reference link=external dest=\"http://example.com\" name=\"home\"\n",
            "      image link=external dest=\"logo.png\"\n",
            "  target +referenced \"home\" link=external dest=\"http://example.com\"\n",
        ),
    );
}

#[test]
fn image_vertical_align_is_error_in_block() {
    expect_codes(
        concat!(".. image:: a.png\n", "   :align: top\n",),
        "rst.directive.option.invalid",
    );
}

#[test]
fn image_takes_no_content() {
    expect_codes(
        concat!(".. image:: a.png\n", "\n", "   content\n",),
        "rst.directive.content.forbidden",
    );
}

#[test]
fn image_needs_uri() {
    expect_codes(".. image::\n", "rst.directive.argument.missing");
}

#[test]
fn figure_with_caption_and_legend() {
    expect_dump(
        concat!(
            ".. figure:: picture.png\n",
            "   :scale: 50\n",
            "   :alt: map\n",
            "   :figwidth: 60%\n",
            "   :figclass: wide\n",
            "   :align: center\n",
            "   :name: fig-map\n",
            "   :target: http://example.com\n",
            "\n",
            "   The caption\n",
            "   on two lines.\n",
            "\n",
            "   +-------+-----+\n",
            "   | Key   | Val |\n",
            "   +-------+-----+\n",
        ),
        concat!(
            "document\n",
            "  figure @width=\"60%\" @class=\"wide\" @name=\"fig-map\" @align=\"center\"\n",
            "    link link=external dest=\"http://example.com\"\n",
            "      image link=external dest=\"picture.png\" @alt=\"map\" @scale=\"50\"\n",
            "    caption\n",
            "      text \"The caption\"\n",
            "      soft-break\n",
            "      text \"on two lines.\"\n",
            "    legend\n",
            "      table value=2 @widths=\"7 5\"\n",
            "        table-body\n",
            "          row\n",
            "            cell span=1x1\n",
            "              paragraph\n",
            "                text \"Key\"\n",
            "            cell span=1x1\n",
            "              paragraph\n",
            "                text \"Val\"\n",
        ),
    );
}

#[test]
fn figure_legend_without_caption() {
    expect_dump(
        concat!(".. figure:: picture.png\n", "\n", "   ..\n", "\n", "   Legend only.\n",),
        concat!(
            "document\n",
            "  figure\n",
            "    image link=external dest=\"picture.png\"\n",
            "    legend\n",
            "      paragraph\n",
            "        text \"Legend only.\"\n",
        ),
    );
}

#[test]
fn figure_caption_must_be_paragraph() {
    expect_codes(
        concat!(".. figure:: picture.png\n", "\n", "   - a list\n",),
        "rst.figure.caption",
    );
}

#[test]
fn topic_with_title() {
    expect_dump(
        concat!(".. topic:: Topic Title\n", "   :class: special\n", "\n", "   Topic body.\n",),
        concat!(
            "document\n",
            "  topic @class=\"special\"\n",
            "    title\n",
            "      text \"Topic Title\"\n",
            "    paragraph\n",
            "      text \"Topic body.\"\n",
        ),
    );
}

#[test]
fn topic_not_allowed_in_body_elements() {
    expect_codes(
        concat!("- item\n", "\n", "  .. topic:: T\n", "\n", "     body\n",),
        "rst.directive.context",
    );
}

#[test]
fn sidebar_with_subtitle() {
    expect_dump(
        concat!(
            ".. sidebar:: Sidebar Title\n",
            "   :subtitle: Optional Subtitle\n",
            "\n",
            "   Sidebar body.\n",
            "\n",
            "   .. topic:: Inner topic\n",
            "\n",
            "      Allowed in a sidebar.\n",
        ),
        concat!(
            "document\n",
            "  sidebar\n",
            "    title\n",
            "      text \"Sidebar Title\"\n",
            "    subtitle\n",
            "      text \"Optional Subtitle\"\n",
            "    paragraph\n",
            "      text \"Sidebar body.\"\n",
            "    topic\n",
            "      title\n",
            "        text \"Inner topic\"\n",
            "      paragraph\n",
            "        text \"Allowed in a sidebar.\"\n",
        ),
    );
}

#[test]
fn sidebar_without_title() {
    expect_dump(
        concat!(".. sidebar::\n", "\n", "   Body only.\n",),
        concat!("document\n", "  sidebar\n", "    paragraph\n", "      text \"Body only.\"\n",),
    );
}

#[test]
fn sidebar_cannot_nest() {
    expect_codes(
        concat!(".. sidebar:: Outer\n", "\n", "   .. sidebar:: Inner\n", "\n", "      body\n",),
        "rst.directive.context",
    );
}

#[test]
fn line_block_directive_nests_lines() {
    expect_dump(
        concat!(
            ".. line-block::\n",
            "   :class: poem\n",
            "\n",
            "   First line\n",
            "      indented\n",
            "   Last line\n",
        ),
        concat!(
            "document\n",
            "  line-block @class=\"poem\"\n",
            "    line\n",
            "      text \"First line\"\n",
            "    line-block\n",
            "      line\n",
            "        text \"indented\"\n",
            "    line\n",
            "      text \"Last line\"\n",
        ),
    );
}

#[test]
fn parsed_literal_keeps_lines() {
    expect_dump(
        concat!(".. parsed-literal::\n", "\n", "   first line\n", "     second line\n",),
        concat!(
            "document\n",
            "  preformatted\n",
            "    text \"first line\"\n",
            "    soft-break\n",
            "    text \"  second line\"\n",
        ),
    );
}

#[test]
fn code_with_language_and_numbers() {
    expect_dump(
        concat!(
            ".. code:: python\n",
            "   :number-lines:\n",
            "   :class: example\n",
            "\n",
            "   print(\"hi\")\n",
        ),
        concat!(
            "document\n",
            "  code-block value=1 \"print(\\\"hi\\\")\\n\" @language=\"python\"",
            " @class=\"example\"\n",
        ),
    );
}

#[test]
fn code_aliases() {
    expect_dump(
        concat!(
            ".. code-block:: c\n",
            "\n",
            "   int x;\n",
            "\n",
            ".. sourcecode::\n",
            "\n",
            "   plain\n",
        ),
        concat!(
            "document\n",
            "  code-block \"int x;\\n\" @language=\"c\"\n",
            "  code-block \"plain\\n\"\n",
        ),
    );
}

#[test]
fn code_accepts_one_argument() {
    expect_codes(
        concat!(".. code:: python extra\n", "\n", "   x\n",),
        "rst.directive.argument.extra",
    );
}

#[test]
fn math_argument_and_groups() {
    expect_dump(
        concat!(
            ".. math:: E = mc^2\n",
            "\n",
            ".. math::\n",
            "   :name: eq\n",
            "\n",
            "   a^2 + b^2 = c^2\n",
            "\n",
            "   \\alpha\n",
        ),
        concat!(
            "document\n",
            "  math-block \"E = mc^2\"\n",
            "  math-block \"a^2 + b^2 = c^2\" @name=\"eq\"\n",
            "  math-block \"\\\\alpha\"\n",
        ),
    );
}

#[test]
fn rubric_with_class() {
    expect_dump(
        concat!(".. rubric:: Informal heading\n", "   :class: small\n",),
        concat!("document\n", "  rubric @class=\"small\"\n", "    text \"Informal heading\"\n",),
    );
}

#[test]
fn quote_directives_add_classes() {
    expect_dump(
        concat!(
            ".. epigraph::\n",
            "\n",
            "   No matter where you go, there you are.\n",
            "\n",
            "   -- Buckaroo Banzai\n",
            "\n",
            ".. highlights::\n",
            "\n",
            "   Highlight.\n",
            "\n",
            ".. pull-quote::\n",
            "\n",
            "   Pulled.\n",
        ),
        concat!(
            "document\n",
            "  block-quote @class=\"epigraph\"\n",
            "    paragraph\n",
            "      text \"No matter where you go, there you are.\"\n",
            "    attribution\n",
            "      text \"Buckaroo Banzai\"\n",
            "  block-quote @class=\"highlights\"\n",
            "    paragraph\n",
            "      text \"Highlight.\"\n",
            "  block-quote @class=\"pull-quote\"\n",
            "    paragraph\n",
            "      text \"Pulled.\"\n",
        ),
    );
}

#[test]
fn compound_and_container() {
    expect_dump(
        concat!(
            ".. compound::\n",
            "   :class: c\n",
            "\n",
            "   Paragraph one.\n",
            "\n",
            "   ::\n",
            "\n",
            "       literal\n",
            "\n",
            ".. container:: custom-class other\n",
            "   :name: box\n",
            "\n",
            "   Inside the container.\n",
        ),
        concat!(
            "document\n",
            "  compound @class=\"c\"\n",
            "    paragraph\n",
            "      text \"Paragraph one.\"\n",
            "    code-block \"literal\\n\"\n",
            "  container @class=\"custom-class other\" @name=\"box\"\n",
            "    paragraph\n",
            "      text \"Inside the container.\"\n",
        ),
    );
}

#[test]
fn contents_options() {
    expect_dump(
        concat!(
            ".. contents:: Table of Contents\n",
            "   :depth: 2\n",
            "   :local:\n",
            "   :backlinks: top\n",
            "   :class: toc\n",
        ),
        concat!(
            "document\n",
            "  contents value=2 +local @title=\"Table of Contents\" @class=\"toc\"",
            " @backlinks=\"top\"\n",
        ),
    );
}

#[test]
fn contents_defaults() {
    expect_dump(".. contents::\n", concat!("document\n", "  contents\n",));
}

#[test]
fn sectnum_options() {
    expect_dump(
        concat!(
            ".. sectnum::\n",
            "   :depth: 3\n",
            "   :prefix: A.\n",
            "   :suffix: )\n",
            "   :start: 2\n",
            "\n",
            ".. section-numbering::\n",
        ),
        concat!(
            "document\n",
            "  directive @name=\"sectnum\" @depth=\"3\" @prefix=\"A.\" @suffix=\")\"",
            " @start=\"2\"\n",
            "  directive @name=\"sectnum\"\n",
        ),
    );
}

#[test]
fn header_and_footer() {
    expect_dump(
        concat!(".. header:: Page header\n\n", ".. footer:: Page footer\n   with more.\n"),
        concat!(
            "document\n",
            "  directive @name=\"header\"\n",
            "    paragraph\n",
            "      text \"Page header\"\n",
            "  directive @name=\"footer\"\n",
            "    paragraph\n",
            "      text \"Page footer\"\n",
            "      soft-break\n",
            "      text \"with more.\"\n",
        ),
    );
}

#[test]
fn target_notes_directive() {
    expect_dump(
        concat!(".. target-notes::\n", "   :class: notes\n", "   :name: notes\n",),
        concat!(
            "document\n",
            "  directive @name=\"target-notes\" @class=\"notes\" @name=\"notes\"\n",
        ),
    );
}

#[test]
fn meta_fields() {
    expect_dump(
        concat!(
            ".. meta::\n",
            "   :keywords: plaintext, markup\n",
            "   :description lang=en: An amusing story\n",
            "      spanning lines.\n",
        ),
        concat!(
            "document\n",
            "  directive @name=\"meta\" @keywords=\"plaintext, markup\"",
            " @description lang=en=\"An amusing story spanning lines.\"\n",
        ),
    );
}

#[test]
fn meta_needs_fields() {
    expect_codes(concat!(".. meta::\n", "\n", "   Not a field.\n",), "rst.meta.invalid");
}

#[test]
fn title_directive_sets_title() {
    expect_dump(
        ".. title:: Document Title\n",
        concat!("document\n", "  directive @name=\"title\" @title=\"Document Title\"\n",),
    );
}

#[test]
fn raw_block_content() {
    expect_dump(
        concat!(".. raw:: HTML   LaTeX\n", "\n", "   <hr/>\n", "   <br/>\n",),
        concat!("document\n", "  raw-block \"<hr/>\\n<br/>\\n\" @format=\"html latex\"\n",),
    );
}

#[test]
fn raw_url_is_refused() {
    expect_codes(
        concat!(".. raw:: html\n", "   :url: http://example.com\n",),
        "rst.directive.url",
    );
}

#[test]
fn raw_needs_content() {
    expect_codes(".. raw:: html\n", "rst.directive.content.missing");
}

#[test]
fn class_applies_to_next_element() {
    expect_dump(
        concat!(".. class:: special\n", "\n", "Paragraph.\n",),
        concat!("document\n", "  paragraph @class=\"special\"\n", "    text \"Paragraph.\"\n",),
    );
}

#[test]
fn class_skips_targets_and_comments() {
    expect_dump(
        concat!(
            ".. class:: special\n",
            "\n",
            ".. _here:\n",
            "\n",
            ".. a comment\n",
            "\n",
            "Paragraph.\n",
        ),
        concat!(
            "document\n",
            "  target \"here\" link=internal ->1\n",
            "  comment \"a comment\"\n",
            "  paragraph @class=\"special\"\n",
            "    text \"Paragraph.\"\n",
        ),
    );
}

#[test]
fn class_with_content_applies_to_each() {
    expect_dump(
        concat!(
            ".. class:: multiple\n",
            "\n",
            "   First paragraph.\n",
            "\n",
            "   Second paragraph.\n",
        ),
        concat!(
            "document\n",
            "  paragraph @class=\"multiple\"\n",
            "    text \"First paragraph.\"\n",
            "  paragraph @class=\"multiple\"\n",
            "    text \"Second paragraph.\"\n",
        ),
    );
}

#[test]
fn class_names_are_identifiers() {
    expect_dump(
        concat!(".. class:: Hello_World 2nd-Class\n", "\n", "Para.\n",),
        concat!(
            "document\n",
            "  paragraph @class=\"hello-world nd-class\"\n",
            "    text \"Para.\"\n",
        ),
    );
}

#[test]
fn class_without_following_element_is_error() {
    expect_codes(concat!("Para.\n", "\n", ".. class:: dangling\n",), "rst.class.dangling");
}

#[test]
fn custom_roles_are_defined() {
    expect_dump(
        concat!(
            ".. role:: custom\n",
            "\n",
            ".. role:: raw-html(raw)\n",
            "   :format: html\n",
            "\n",
            ".. role:: python(code)\n",
            "   :language: python\n",
            "   :class: py\n",
            "\n",
            ":custom:`one` :raw-html:`<b>` :python:`x = 1`\n",
        ),
        concat!(
            "document\n",
            "  paragraph\n",
            "    role \"custom\" @class=\"custom\"\n",
            "      text \"one\"\n",
            "    text \" \"\n",
            "    raw-inline \"<b>\" @format=\"html\" @class=\"raw-html\"\n",
            "    text \" \"\n",
            "    code \"x = 1\" @language=\"python\" @class=\"py\"\n",
        ),
    );
}

#[test]
fn role_with_unknown_base_is_error() {
    expect_codes(".. role:: mine(nonexistent)\n", "rst.role.unknown");
}

#[test]
fn role_format_needs_raw_base() {
    expect_codes(
        concat!(".. role:: mine(emphasis)\n", "   :format: html\n",),
        "rst.directive.option.unknown",
    );
}

#[test]
fn default_role_sets_role() {
    expect_dump(
        concat!(".. default-role:: strong\n", "\n", "`bold`\n",),
        concat!("document\n", "  paragraph\n", "    strong\n", "      text \"bold\"\n",),
    );
}

#[test]
fn default_role_unknown_is_error() {
    expect_codes(".. default-role:: nonexistent\n", "rst.role.unknown");
}

#[test]
fn replace_outside_substitution_is_error() {
    expect_codes(".. replace:: text\n", "rst.directive.context");
}

#[test]
fn unknown_directive_is_error() {
    expect_codes(concat!(".. toctree::\n", "   :maxdepth: 2\n",), "rst.directive.unknown");
}

#[test]
fn test_directive_is_unknown() {
    expect_codes(".. restructuredtext-test-directive:: args\n", "rst.directive.unknown");
}

#[test]
fn unknown_option_is_error() {
    expect_codes(
        concat!(".. note:: text\n", "   :bogus: value\n",),
        "rst.directive.option.unknown",
    );
}

#[test]
fn duplicate_option_is_error() {
    expect_codes(
        concat!(".. note:: text\n", "   :class: a\n", "   :class: b\n",),
        "rst.directive.option.duplicate",
    );
}

#[test]
fn invalid_option_values_are_errors() {
    expect_codes(
        concat!(
            ".. image:: a.png\n",
            "   :width: wide\n",
            "\n",
            ".. image:: b.png\n",
            "   :align: sideways\n",
            "\n",
            ".. contents::\n",
            "   :local: yes\n",
            "\n",
            ".. contents::\n",
            "   :depth: -1\n",
        ),
        "rst.directive.option.invalid",
    );
}

#[test]
fn option_block_must_be_fields() {
    expect_codes(
        concat!(".. image:: a.png\n", "   :alt: x\n", "   not an option\n",),
        "rst.directive.option.block",
    );
}

#[test]
fn directive_name_is_case_insensitive() {
    expect_dump(
        ".. NOTE:: Shouting.\n",
        concat!(
            "document\n",
            "  admonition value=7\n",
            "    paragraph\n",
            "      text \"Shouting.\"\n",
        ),
    );
}
