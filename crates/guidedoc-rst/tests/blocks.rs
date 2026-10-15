//! Body elements: line endings, paragraphs, section titles, lists, literal and quoted blocks.

#![allow(clippy::unwrap_used)]

mod common;

use common::*;

#[test]
fn line_endings_are_normalized() {
    expect_dump(
        concat!("One\r\n", "two\r\rthree\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"One\"\n",
            "    soft-break\n",
            "    text \"two\"\n",
            "  paragraph\n",
            "    text \"three\"\n",
        ),
    );
}

#[test]
fn paragraphs_are_separated_by_blank_lines() {
    expect_dump(
        concat!("First paragraph\n", "continues here.\n", "\n", "Second paragraph.\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"First paragraph\"\n",
            "    soft-break\n",
            "    text \"continues here.\"\n",
            "  paragraph\n",
            "    text \"Second paragraph.\"\n",
        ),
    );
}

#[test]
fn underlined_titles_make_sections() {
    expect_dump(
        concat!(
            "Chapter\n",
            "=======\n",
            "\n",
            "Text.\n",
            "\n",
            "Section\n",
            "-------\n",
            "\n",
            "More.\n",
            "\n",
            "Next chapter\n",
            "============\n",
            "\n",
            "End.\n",
        ),
        concat!(
            "document\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"Chapter\"\n",
            "    paragraph\n",
            "      text \"Text.\"\n",
            "    section value=2\n",
            "      heading value=2\n",
            "        text \"Section\"\n",
            "      paragraph\n",
            "        text \"More.\"\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"Next chapter\"\n",
            "    paragraph\n",
            "      text \"End.\"\n",
        ),
    );
}

#[test]
fn overlined_titles_may_be_inset() {
    expect_dump(
        concat!("=========\n", " Inset\n", "=========\n", "\n", "Body.\n",),
        concat!(
            "document\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"Inset\"\n",
            "    paragraph\n",
            "      text \"Body.\"\n",
        ),
    );
}

#[test]
fn over_and_under_style_differs_from_underline_only() {
    expect_dump(
        concat!("=====\n", "Top\n", "=====\n", "\n", "Under\n", "=====\n", "\n", "Body.\n",),
        concat!(
            "document\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"Top\"\n",
            "    section value=2\n",
            "      heading value=2\n",
            "        text \"Under\"\n",
            "      paragraph\n",
            "        text \"Body.\"\n",
        ),
    );
}

#[test]
fn short_underline_is_text() {
    expect_dump(
        concat!("Long title\n", "---\n", "\n", "Body.\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Long title\"\n",
            "    soft-break\n",
            "    text \"---\"\n",
            "  paragraph\n",
            "    text \"Body.\"\n",
        ),
    );
}

#[test]
fn short_title_underline_warns() {
    expect_dump_codes(
        concat!("Title text\n", "====\n", "\n", "Body.\n",),
        concat!(
            "document\n",
            "  section value=1\n",
            "    heading value=1\n",
            "      text \"Title text\"\n",
            "    paragraph\n",
            "      text \"Body.\"\n",
        ),
        "rst.section.underline-short",
    );
}

#[test]
fn transitions_separate_body_elements() {
    expect_dump(
        concat!("Para one.\n", "\n", "----------\n", "\n", "Para two.\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Para one.\"\n",
            "  thematic-break\n",
            "  paragraph\n",
            "    text \"Para two.\"\n",
        ),
    );
}

#[test]
fn bullet_lists_use_every_bullet() {
    expect_dump(
        concat!(
            "* star\n",
            "* star two\n",
            "\n",
            "- dash\n",
            "\n",
            "+ plus\n",
            "\n",
            "• bullet\n",
            "\n",
            "‣ triangle\n",
            "\n",
            "⁃ hyphen\n",
        ),
        concat!(
            "document\n",
            "  list +tight list=bullet,*,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"star\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"star two\"\n",
            "  list +tight list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"dash\"\n",
            "  list +tight list=bullet,+,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"plus\"\n",
            "  list +tight list=bullet,,0 @bullet=\"•\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"bullet\"\n",
            "  list +tight list=bullet,,0 @bullet=\"‣\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"triangle\"\n",
            "  list +tight list=bullet,,0 @bullet=\"⁃\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"hyphen\"\n",
        ),
    );
}

#[test]
fn bullet_items_hold_blocks() {
    expect_dump(
        concat!(
            "- First item paragraph.\n",
            "\n",
            "  Second paragraph.\n",
            "\n",
            "  - nested item\n",
            "\n",
            "- Second item.\n",
        ),
        concat!(
            "document\n",
            "  list list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"First item paragraph.\"\n",
            "      paragraph\n",
            "        text \"Second paragraph.\"\n",
            "      list +tight list=bullet,-,0\n",
            "        list-item\n",
            "          paragraph\n",
            "            text \"nested item\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"Second item.\"\n",
        ),
    );
}

#[test]
fn bullet_item_may_start_on_next_line() {
    expect_dump(
        concat!("-\n", "  text below\n",),
        concat!(
            "document\n",
            "  list +tight list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"text below\"\n",
        ),
    );
}

#[test]
fn enumerated_formats_and_sequences() {
    expect_dump(
        concat!(
            "1. arabic\n",
            "2. two\n",
            "\n",
            "a. alpha\n",
            "b. two\n",
            "\n",
            "A) upper\n",
            "B) two\n",
            "\n",
            "(i) roman\n",
            "(ii) two\n",
            "\n",
            "(IV) upper roman\n",
            "(V) five\n",
            "\n",
            "#. auto\n",
            "#. auto two\n",
        ),
        concat!(
            "document\n",
            "  list +tight list=arabic,.,1\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"arabic\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"two\"\n",
            "  list +tight list=loweralpha,.,1\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"alpha\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"two\"\n",
            "  list +tight list=upperalpha,),1\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"upper\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"two\"\n",
            "  list +tight list=lowerroman,(,1\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"roman\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"two\"\n",
            "  list +tight list=upperroman,(,4\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"upper roman\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"five\"\n",
            "  list +tight list=arabic,.,1\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"auto\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"auto two\"\n",
        ),
    );
}

#[test]
fn enumerated_list_start_value() {
    expect_dump(
        concat!("3. three\n", "4. four\n",),
        concat!(
            "document\n",
            "  list +tight list=arabic,.,3\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"three\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"four\"\n",
        ),
    );
}

#[test]
fn enumerated_ambiguity_prefers_alpha_in_sequence() {
    expect_dump(
        concat!("h. eight\n", "i. nine\n", "j. ten\n",),
        concat!(
            "document\n",
            "  list +tight list=loweralpha,.,8\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"eight\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"nine\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"ten\"\n",
        ),
    );
}

#[test]
fn enumerated_first_i_is_roman() {
    expect_dump(
        concat!("i. one\n", "ii. two\n",),
        concat!(
            "document\n",
            "  list +tight list=lowerroman,.,1\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"one\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"two\"\n",
        ),
    );
}

#[test]
fn enumerated_second_line_must_continue_sequence() {
    expect_dump(
        concat!("1. one\n", "3. three\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"1. one\"\n",
            "    soft-break\n",
            "    text \"3. three\"\n",
        ),
    );
}

#[test]
fn enumerated_sequence_break_starts_new_list() {
    expect_dump(
        concat!("1. one\n", "\n", "3. three\n",),
        concat!(
            "document\n",
            "  list +tight list=arabic,.,1\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"one\"\n",
            "  list +tight list=arabic,.,3\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"three\"\n",
        ),
    );
}

#[test]
fn enumerated_needs_valid_second_line() {
    expect_dump(
        concat!("A. Smith wrote this\n", "sentence.\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"A. Smith wrote this\"\n",
            "    soft-break\n",
            "    text \"sentence.\"\n",
        ),
    );
}

#[test]
fn enumerated_invalid_roman_is_text() {
    expect_dump(
        "iiii. text\n",
        concat!("document\n", "  paragraph\n", "    text \"iiii. text\"\n",),
    );
}

#[test]
fn definition_lists_with_classifiers() {
    expect_dump(
        concat!(
            "term 1\n",
            "   Definition 1.\n",
            "\n",
            "term 2 : classifier one : classifier two\n",
            "   Definition 2.\n",
            "\n",
            "   Second paragraph.\n",
        ),
        concat!(
            "document\n",
            "  definition-list\n",
            "    definition-item\n",
            "      term\n",
            "        text \"term 1\"\n",
            "      definition\n",
            "        paragraph\n",
            "          text \"Definition 1.\"\n",
            "    definition-item\n",
            "      term\n",
            "        text \"term 2\"\n",
            "      classifier\n",
            "        text \"classifier one\"\n",
            "      classifier\n",
            "        text \"classifier two\"\n",
            "      definition\n",
            "        paragraph\n",
            "          text \"Definition 2.\"\n",
            "        paragraph\n",
            "          text \"Second paragraph.\"\n",
        ),
    );
}

#[test]
fn field_lists() {
    expect_dump_plain(
        concat!(
            ":Author: Someone\n",
            ":Version: 1.0\n",
            ":Long field: First line\n",
            "   continued line.\n",
            "\n",
            "   Second paragraph.\n",
            ":empty:\n",
        ),
        concat!(
            "document\n",
            "  field-list\n",
            "    field\n",
            "      field-name\n",
            "        text \"Author\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"Someone\"\n",
            "    field\n",
            "      field-name\n",
            "        text \"Version\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"1.0\"\n",
            "    field\n",
            "      field-name\n",
            "        text \"Long field\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"First line\"\n",
            "          soft-break\n",
            "          text \"continued line.\"\n",
            "        paragraph\n",
            "          text \"Second paragraph.\"\n",
            "    field\n",
            "      field-name\n",
            "        text \"empty\"\n",
            "      field-body\n",
        ),
    );
}

#[test]
fn field_names_may_hold_escaped_colons() {
    expect_dump_plain(
        concat!(":a\\:b: value\n", ":c:d: other\n",),
        concat!(
            "document\n",
            "  field-list\n",
            "    field\n",
            "      field-name\n",
            "        text \"a:b\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"value\"\n",
            "    field\n",
            "      field-name\n",
            "        text \"c:d\"\n",
            "      field-body\n",
            "        paragraph\n",
            "          text \"other\"\n",
        ),
    );
}

#[test]
fn option_lists_cover_every_form() {
    expect_dump(
        concat!(
            "-a            Short option.\n",
            "-b file       With argument.\n",
            "-cfile        Attached argument.\n",
            "--long        Long option.\n",
            "--input=file  Long with equals.\n",
            "--output file  Long with space.\n",
        ),
        concat!(
            "document\n",
            "  option-list\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"-a\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"Short option.\"\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"-b\"\n",
            "          option-argument value=32 \"file\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"With argument.\"\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"-c\"\n",
            "          option-argument \"file\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"Attached argument.\"\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"--long\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"Long option.\"\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"--input\"\n",
            "          option-argument value=61 \"file\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"Long with equals.\"\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"--output\"\n",
            "          option-argument value=32 \"file\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"Long with space.\"\n",
        ),
    );
}

#[test]
fn option_groups_and_next_line_descriptions() {
    expect_dump(
        concat!(
            "/V            DOS option.\n",
            "-x, --exclude=<a b>  Group with bracketed argument.\n",
            "-f\n",
            "    Description on the next line.\n",
        ),
        concat!(
            "document\n",
            "  option-list\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"/V\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"DOS option.\"\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"-x\"\n",
            "        option \"--exclude\"\n",
            "          option-argument value=61 \"<a b>\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"Group with bracketed argument.\"\n",
            "    option-item\n",
            "      option-group\n",
            "        option \"-f\"\n",
            "      option-description\n",
            "        paragraph\n",
            "          text \"Description on the next line.\"\n",
        ),
    );
}

#[test]
fn option_like_line_without_description_is_text() {
    expect_dump(
        "-a is not an option list without a description.\n",
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"-a is not an option list without a description.\"\n",
        ),
    );
}

#[test]
fn literal_block_after_double_colon() {
    expect_dump(
        concat!(
            "Paragraph::\n",
            "\n",
            "    literal text\n",
            "      indented more\n",
            "\n",
            "After.\n",
        ),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Paragraph:\"\n",
            "  code-block \"literal text\\n  indented more\\n\"\n",
            "  paragraph\n",
            "    text \"After.\"\n",
        ),
    );
}

#[test]
fn literal_block_marker_with_space() {
    expect_dump(
        concat!("Paragraph ::\n", "\n", "    literal\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Paragraph\"\n",
            "  code-block \"literal\\n\"\n",
        ),
    );
}

#[test]
fn literal_block_marker_alone() {
    expect_dump(
        concat!("Paragraph.\n", "\n", "::\n", "\n", "    literal\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Paragraph.\"\n",
            "  code-block \"literal\\n\"\n",
        ),
    );
}

#[test]
fn literal_block_escaped_marker() {
    expect_dump(
        concat!("Not literal\\::\n", "\n", "    quote\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Not literal::\"\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"quote\"\n",
        ),
    );
}

#[test]
fn quoted_literal_block() {
    expect_dump(
        concat!("Paragraph::\n", "\n", "> quoted line\n", "> another\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Paragraph:\"\n",
            "  code-block \"> quoted line\\n> another\\n\"\n",
        ),
    );
}

#[test]
fn line_blocks_nest_by_indentation() {
    expect_dump(
        concat!(
            "| First line\n",
            "|    Indented line\n",
            "|        Deeper\n",
            "| Back\n",
            "|\n",
            "| After empty\n",
            "  continuation line\n",
        ),
        concat!(
            "document\n",
            "  line-block\n",
            "    line\n",
            "      text \"First line\"\n",
            "    line-block\n",
            "      line\n",
            "        text \"Indented line\"\n",
            "      line-block\n",
            "        line\n",
            "          text \"Deeper\"\n",
            "    line\n",
            "      text \"Back\"\n",
            "    line\n",
            "    line\n",
            "      text \"After empty\"\n",
            "      soft-break\n",
            "      text \"continuation line\"\n",
        ),
    );
}

#[test]
fn block_quote_with_attribution() {
    expect_dump(
        concat!(
            "Paragraph.\n",
            "\n",
            "    Quoted text.\n",
            "\n",
            "    -- Attribution\n",
            "       continued\n",
            "\n",
            "    Second quote.\n",
        ),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Paragraph.\"\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"Quoted text.\"\n",
            "    attribution\n",
            "      text \"Attribution\"\n",
            "      soft-break\n",
            "      text \"continued\"\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"Second quote.\"\n",
        ),
    );
}

#[test]
fn block_quotes_nest() {
    expect_dump(
        concat!("Para.\n", "\n", "    Level one.\n", "\n", "        Level two.\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Para.\"\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"Level one.\"\n",
            "    block-quote\n",
            "      paragraph\n",
            "        text \"Level two.\"\n",
        ),
    );
}

#[test]
fn doctest_blocks_keep_text() {
    expect_dump(
        concat!(">>> print(\"hi\")\n", "hi\n", ">>> 1 + 1\n", "2\n",),
        concat!("document\n", "  doctest-block \">>> print(\\\"hi\\\")\\nhi\\n>>> 1 + 1\\n2\"\n",),
    );
}

#[test]
fn tabs_expand_to_tab_stops() {
    expect_dump(
        concat!("Para::\n", "\n", "\ttab\tindented\n", "\n", "-\titem\n", "\tcontinuation\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Para:\"\n",
            "  code-block \"tab     indented\\n\"\n",
            "  list +tight list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"item\"\n",
            "        soft-break\n",
            "        text \"continuation\"\n",
        ),
    );
}

#[test]
fn transition_at_start_is_error() {
    expect_codes(concat!("----------\n", "\n", "Para.\n",), "rst.transition.start");
}

#[test]
fn adjacent_transitions_are_errors() {
    expect_codes(
        concat!("Para.\n", "\n", "----------\n", "\n", "----------\n", "\n", "Para.\n",),
        "rst.transition.adjacent",
    );
}

#[test]
fn transition_at_end_is_error() {
    expect_codes(concat!("Para.\n", "\n", "----------\n",), "rst.transition.end");
}

#[test]
fn inconsistent_title_levels_are_errors() {
    expect_codes(
        concat!(
            "Title\n", "=====\n", "\n", "Sub\n", "---\n", "\n", "Subsub\n", "~~~~~~\n", "\n",
            "Wrong\n", "=====\n", "\n", "Bad\n", "~~~\n",
        ),
        "rst.section.level",
    );
}

#[test]
fn title_in_body_element_is_error() {
    expect_codes(concat!("- item\n", "\n", "  Title\n", "  =====\n",), "rst.section.unexpected");
}

#[test]
fn unexpected_indentation_is_error() {
    expect_codes(
        concat!("Line one\n", "line two\n", "    indented line\n",),
        "rst.indent.unexpected",
    );
}

#[test]
fn incomplete_overline_title() {
    expect_codes(concat!("========\n", "Title\n",), "rst.section.incomplete");
}

#[test]
fn mismatched_overline() {
    expect_codes(concat!("======\n", "Title\n", "------\n",), "rst.section.mismatch");
}

#[test]
fn list_without_blank_line_warns() {
    expect_dump_codes(
        concat!("- item\n", "text after\n",),
        concat!(
            "document\n",
            "  list +tight list=bullet,-,0\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"item\"\n",
            "  paragraph\n",
            "    text \"text after\"\n",
        ),
        "rst.block.unindent",
    );
}

#[test]
fn attribution_dash_forms() {
    expect_dump(
        concat!(
            "Para.\n",
            "\n",
            "    Quote one.\n",
            "\n",
            "    — Em Dash Author\n",
            "\n",
            "    Quote two.\n",
            "\n",
            "    --- Triple Author\n",
        ),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Para.\"\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"Quote one.\"\n",
            "    attribution\n",
            "      text \"Em Dash Author\"\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"Quote two.\"\n",
            "    attribution\n",
            "      text \"Triple Author\"\n",
        ),
    );
}

#[test]
fn literal_block_missing_warns() {
    expect_dump_codes(
        concat!("Para::\n", "\n", "Not indented text.\n",),
        concat!(
            "document\n",
            "  paragraph\n",
            "    text \"Para:\"\n",
            "  paragraph\n",
            "    text \"Not indented text.\"\n",
        ),
        "rst.literal.missing",
    );
}

#[test]
fn inconsistent_literal_quoting_is_error() {
    expect_codes(concat!("Para::\n", "\n", "> one\n", "| two\n",), "rst.literal.quoting");
}

#[test]
fn definition_term_keeps_inline_literal_colons() {
    expect_dump(
        concat!("``a : b`` : classifier\n", "   Definition.\n",),
        concat!(
            "document\n",
            "  definition-list\n",
            "    definition-item\n",
            "      term\n",
            "        code \"a : b\"\n",
            "      classifier\n",
            "        text \"classifier\"\n",
            "      definition\n",
            "        paragraph\n",
            "          text \"Definition.\"\n",
        ),
    );
}

#[test]
fn list_item_text_fixes_the_indentation() {
    expect_dump_codes(
        concat!(
            "- item\n",
            "    deeper lines make a definition list\n",
            "- item\n",
            " under-indented text ends the list\n",
        ),
        concat!(
            "document\n",
            "  list list=bullet,-,0\n",
            "    list-item\n",
            "      definition-list\n",
            "        definition-item\n",
            "          term\n",
            "            text \"item\"\n",
            "          definition\n",
            "            paragraph\n",
            "              text \"deeper lines make a definition list\"\n",
            "    list-item\n",
            "      paragraph\n",
            "        text \"item\"\n",
            "  block-quote\n",
            "    paragraph\n",
            "      text \"under-indented text ends the list\"\n",
        ),
        "rst.block.unindent",
    );
}
