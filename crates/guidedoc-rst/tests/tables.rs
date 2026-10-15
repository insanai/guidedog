//! Grid and simple tables: headers, UTF-8 cells, cell paragraphs and malformed tables reported as
//! errors.

#![allow(clippy::unwrap_used)]

mod common;

use common::*;

#[test]
#[allow(clippy::too_many_lines)] // one table's source and tree
fn grid_table_with_header_and_spans() {
    expect_dump(
        concat!(
            "+------------+------------+-----------+\n",
            "| Header 1   | Header 2   | Header 3  |\n",
            "+============+============+===========+\n",
            "| body row 1 | column 2   | column 3  |\n",
            "+------------+------------+-----------+\n",
            "| body row 2 | Cells may span columns.|\n",
            "+------------+------------+-----------+\n",
            "| body row 3 | Cells may  | - Cells   |\n",
            "+------------+ span rows. | - contain |\n",
            "| body row 4 |            | - blocks. |\n",
            "+------------+------------+-----------+\n",
        ),
        concat!(
            "document\n",
            "  table value=3 @widths=\"12 12 11\"\n",
            "    table-head\n",
            "      row +header\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Header 1\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Header 2\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Header 3\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"body row 1\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"column 2\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"column 3\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"body row 2\"\n",
            "        cell span=1x2\n",
            "          paragraph\n",
            "            text \"Cells may span columns.\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"body row 3\"\n",
            "        cell span=2x1\n",
            "          paragraph\n",
            "            text \"Cells may\"\n",
            "            soft-break\n",
            "            text \"span rows.\"\n",
            "        cell span=2x1\n",
            "          list +tight list=bullet,-,0\n",
            "            list-item\n",
            "              paragraph\n",
            "                text \"Cells\"\n",
            "            list-item\n",
            "              paragraph\n",
            "                text \"contain\"\n",
            "            list-item\n",
            "              paragraph\n",
            "                text \"blocks.\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"body row 4\"\n",
        ),
    );
}

#[test]
fn grid_table_without_header() {
    expect_dump(
        concat!("+---+---+\n", "| a | b |\n", "+---+---+\n",),
        concat!(
            "document\n",
            "  table value=2 @widths=\"3 3\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
        ),
    );
}

#[test]
fn grid_table_cells_hold_utf8() {
    expect_dump(
        concat!("+------+------+\n", "| café | naïve|\n", "+------+------+\n",),
        concat!(
            "document\n",
            "  table value=2 @widths=\"6 6\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"café\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"naïve\"\n",
        ),
    );
}

#[test]
fn grid_table_cell_paragraphs() {
    expect_dump(
        concat!(
            "+-----------------+\n",
            "| First para.     |\n",
            "|                 |\n",
            "| Second para.    |\n",
            "+-----------------+\n",
        ),
        concat!(
            "document\n",
            "  table value=1 @widths=\"17\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"First para.\"\n",
            "          paragraph\n",
            "            text \"Second para.\"\n",
        ),
    );
}

#[test]
fn grid_table_ragged_edge_is_error() {
    expect_codes(concat!("+---+---+\n", "| a | b |\n", "+---+--+\n",), "rst.table.malformed");
}

#[test]
fn grid_table_incomplete_is_error() {
    expect_codes(concat!("+---+---+\n", "| a | b\n", "+---+---+\n",), "rst.table.malformed");
}

#[test]
fn grid_table_two_header_separators_is_error() {
    expect_codes(
        concat!("+---+\n", "| a |\n", "+===+\n", "| b |\n", "+===+\n", "| c |\n", "+---+\n",),
        "rst.table.malformed",
    );
}

#[test]
fn simple_table_with_header() {
    expect_dump(
        concat!(
            "=====  =====\n",
            "col 1  col 2\n",
            "=====  =====\n",
            "1      Second column of row 1.\n",
            "2      Second column of row 2.\n",
            "       Second line of paragraph.\n",
            "3      - Second column of row 3.\n",
            "\n",
            "       - Second item in bullet\n",
            "         list (row 3, column 2).\n",
            "\\      Row 4; column 1 will be empty.\n",
            "=====  =====\n",
        ),
        concat!(
            "document\n",
            "  table value=2 @widths=\"5 30\"\n",
            "    table-head\n",
            "      row +header\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"col 1\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"col 2\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"1\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Second column of row 1.\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"2\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Second column of row 2.\"\n",
            "            soft-break\n",
            "            text \"Second line of paragraph.\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"3\"\n",
            "        cell span=1x1\n",
            "          list +tight list=bullet,-,0\n",
            "            list-item\n",
            "              paragraph\n",
            "                text \"Second column of row 3.\"\n",
            "            list-item\n",
            "              paragraph\n",
            "                text \"Second item in bullet\"\n",
            "                soft-break\n",
            "                text \"list (row 3, column 2).\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Row 4; column 1 will be empty.\"\n",
        ),
    );
}

#[test]
fn simple_table_column_spans() {
    expect_dump(
        concat!(
            "=====  =====  ======\n",
            "   Inputs     Output\n",
            "------------  ------\n",
            "  A      B    A or B\n",
            "=====  =====  ======\n",
            "False  False  False\n",
            "True   False  True\n",
            "=====  =====  ======\n",
        ),
        concat!(
            "document\n",
            "  table value=3 @widths=\"5 5 6\"\n",
            "    table-head\n",
            "      row +header\n",
            "        cell span=1x2\n",
            "          paragraph\n",
            "            text \"Inputs\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Output\"\n",
            "      row +header\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"A\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"B\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"A or B\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"False\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"False\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"False\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"True\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"False\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"True\"\n",
        ),
    );
}

#[test]
fn simple_table_without_header() {
    expect_dump(
        concat!("=====  =====\n", "a      b\n", "c      d\n", "=====  =====\n",),
        concat!(
            "document\n",
            "  table value=2 @widths=\"5 5\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"c\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"d\"\n",
        ),
    );
}

#[test]
fn simple_table_last_column_grows() {
    expect_dump(
        concat!("=====  ===\n", "a      b\n", "c      overflowing text\n", "=====  ===\n",),
        concat!(
            "document\n",
            "  table value=2 @widths=\"5 16\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"c\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"overflowing text\"\n",
        ),
    );
}

#[test]
fn simple_table_text_in_margin_is_error() {
    expect_codes(
        concat!("=====  =====\n", "abcdefgh  b\n", "=====  =====\n",),
        "rst.table.malformed",
    );
}

#[test]
fn simple_table_missing_bottom_is_error() {
    expect_codes(concat!("=====  =====\n", "a      b\n",), "rst.table.malformed");
}

#[test]
fn simple_table_border_mismatch_is_error() {
    expect_codes(concat!("=====  =====\n", "a      b\n", "=====  ===\n",), "rst.table.malformed");
}

#[test]
fn table_directive_titles_a_table() {
    expect_dump(
        concat!(
            ".. table:: Truth table\n",
            "   :class: tight\n",
            "   :name: truth\n",
            "   :align: center\n",
            "   :widths: 1 2\n",
            "\n",
            "   =====  =====\n",
            "   A      B\n",
            "   =====  =====\n",
            "   x      y\n",
            "   =====  =====\n",
        ),
        concat!(
            "document\n",
            "  table value=2 @class=\"tight colwidths-given\" @name=\"truth\"",
            " @align=\"center\" @widths=\"1 2\"\n",
            "    title\n",
            "      text \"Truth table\"\n",
            "    table-head\n",
            "      row +header\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"A\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"B\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"x\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"y\"\n",
        ),
    );
}

#[test]
fn table_directive_needs_a_table() {
    expect_codes(concat!(".. table:: Title\n", "\n", "   Not a table.\n",), "rst.table.shape");
}

#[test]
fn table_directive_widths_must_match() {
    expect_codes(
        concat!(
            ".. table::\n",
            "   :widths: 1 2 3\n",
            "\n",
            "   =====  =====\n",
            "   A      B\n",
            "   =====  =====\n",
        ),
        "rst.table.shape",
    );
}

#[test]
fn table_directive_auto_widths() {
    expect_dump(
        concat!(
            ".. table::\n",
            "   :widths: auto\n",
            "\n",
            "   +---+---+\n",
            "   | a | b |\n",
            "   +---+---+\n",
        ),
        concat!(
            "document\n",
            "  table value=2 @class=\"colwidths-auto\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
        ),
    );
}

#[test]
fn csv_table_with_options() {
    expect_dump(
        concat!(
            ".. csv-table:: Prices\n",
            "   :header: \"Item\", \"Price\"\n",
            "   :widths: 3, 1\n",
            "   :header-rows: 1\n",
            "   :stub-columns: 1\n",
            "\n",
            "   \"Name\", \"Cost\"\n",
            "   \"Ice \"\"cream\"\"\", 1.5\n",
            "   \"Cone, big\", 2\n",
            "   short\n",
        ),
        concat!(
            "document\n",
            "  table value=2 @class=\"colwidths-given\" @widths=\"3 1\"\n",
            "    title\n",
            "      text \"Prices\"\n",
            "    table-head\n",
            "      row +header\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"Item\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Price\"\n",
            "      row +header\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"Name\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Cost\"\n",
            "    table-body\n",
            "      row\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"Ice \\\"cream\\\"\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"1.5\"\n",
            "      row\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"Cone, big\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"2\"\n",
            "      row\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"short\"\n",
            "        cell span=1x1\n",
        ),
    );
}

#[test]
fn csv_table_multiline_quoted_cell() {
    expect_dump(
        concat!(".. csv-table::\n", "\n", "   \"first\n", "   second\", b\n",),
        concat!(
            "document\n",
            "  table value=2 @widths=\"1 1\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"first\"\n",
            "            soft-break\n",
            "            text \"second\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
        ),
    );
}

#[test]
fn csv_table_delimiter_and_quote() {
    expect_dump(
        concat!(
            ".. csv-table::\n",
            "   :delim: ;\n",
            "   :quote: '\n",
            "   :keepspace:\n",
            "\n",
            "   'a;b'; c\n",
        ),
        concat!(
            "document\n",
            "  table value=2 @widths=\"1 1\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a;b\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"c\"\n",
        ),
    );
}

#[test]
fn csv_table_escape_character() {
    expect_dump(
        concat!(".. csv-table::\n   :escape: \\\n\n", "   \"a \\\"quoted\\\" word\", b\n"),
        concat!(
            "document\n",
            "  table value=2 @widths=\"1 1\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a \\\"quoted\\\" word\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
        ),
    );
}

#[test]
fn csv_table_tab_delimiter() {
    expect_dump(
        concat!(".. csv-table::\n", "   :delim: tab\n", "\n", "   a\tb\n",),
        concat!(
            "document\n",
            "  table value=2 @widths=\"1 1\"\n",
            "    table-body\n",
            "      row\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"a\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"b\"\n",
        ),
    );
}

#[test]
fn csv_table_unclosed_quote_is_error() {
    expect_codes(concat!(".. csv-table::\n", "\n", "   \"open, b\n",), "rst.csv.malformed");
}

#[test]
fn csv_table_url_is_refused() {
    expect_codes(
        concat!(".. csv-table::\n", "   :url: http://example.com/data.csv\n",),
        "rst.directive.url",
    );
}

#[test]
fn csv_table_header_rows_exceed_data() {
    expect_codes(
        concat!(".. csv-table::\n", "   :header-rows: 3\n", "\n", "   a, b\n",),
        "rst.table.shape",
    );
}

#[test]
fn list_table_with_header_and_stub() {
    expect_dump(
        concat!(
            ".. list-table:: Frozen\n",
            "   :header-rows: 1\n",
            "   :stub-columns: 1\n",
            "   :widths: 2 1\n",
            "\n",
            "   * - Treat\n",
            "     - Quantity\n",
            "   * - Albatross\n",
            "     - 2.99\n",
            "   * - Crunchy Frog\n",
            "     -\n",
        ),
        concat!(
            "document\n",
            "  table value=2 @class=\"colwidths-given\" @widths=\"2 1\"\n",
            "    title\n",
            "      text \"Frozen\"\n",
            "    table-head\n",
            "      row +header\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"Treat\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"Quantity\"\n",
            "    table-body\n",
            "      row\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"Albatross\"\n",
            "        cell span=1x1\n",
            "          paragraph\n",
            "            text \"2.99\"\n",
            "      row\n",
            "        cell +header span=1x1\n",
            "          paragraph\n",
            "            text \"Crunchy Frog\"\n",
            "        cell span=1x1\n",
        ),
    );
}

#[test]
fn list_table_uneven_rows_is_error() {
    expect_codes(
        concat!(".. list-table::\n", "\n", "   * - a\n", "     - b\n", "   * - c\n",),
        "rst.list-table.shape",
    );
}

#[test]
fn list_table_needs_nested_lists() {
    expect_codes(
        concat!(".. list-table::\n", "\n", "   * a\n", "   * b\n",),
        "rst.list-table.shape",
    );
}

#[test]
fn list_table_cell_directive_content_after_blank_line() {
    // CPython's whatsnew/3.15.rst: the cell's text column fixes the item's indentation,
    // so the code directive's content, deeper and after a blank line, belongs to it.
    expect_dump(
        concat!(
            "Intro.\n\n",
            "  .. list-table:: T\n",
            "      :header-rows: 1\n\n",
            "      * - A\n",
            "        - B\n",
            "      * - x\n",
            "        - .. code:: python\n\n",
            "              def read() -> bytes:\n",
            "                  return b\"\"\n",
        ),
        concat!(
            "document\n  paragraph\n    text \"Intro.\"\n  block-quote\n",
            "    table value=2 @widths=\"1 1\"\n      title\n        text \"T\"\n",
            "      table-head\n        row +header\n",
            "          cell span=1x1\n            paragraph\n              text \"A\"\n",
            "          cell span=1x1\n            paragraph\n              text \"B\"\n",
            "      table-body\n        row\n",
            "          cell span=1x1\n            paragraph\n              text \"x\"\n",
            "          cell span=1x1\n",
            "            code-block \"def read() -> bytes:\\n    return b\\\"\\\"\\n\" ",
            "@language=\"python\"\n",
        ),
    );
}
