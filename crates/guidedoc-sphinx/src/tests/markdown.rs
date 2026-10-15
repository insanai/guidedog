//! Ported from the Odin package's markdown_test.odin.

use crate::tests::*;

#[test]
fn markdown_directives_parse_markdown_content() {
    md_expect(
        concat!(
            "```{note}\nA *note* [x](https://x.org).\n```\n\n",
            "````{admonition} My **title**\n:class: tip\n\n```{warning}\nnested\n```\n````\n",
        ),
        &[
            "admonition value=7\n    paragraph\n      text \"A \"\n      emphasis\n",
            "link link=external dest=\"https://x.org\"",
            "admonition @class=\"tip\"\n    title\n      text \"My \"\n      strong\n",
            "admonition value=9\n      paragraph\n        text \"nested\"",
        ],
    );
    md_expect(
        "```{note} Content on the first line.\n```\n",
        &["admonition value=7\n    paragraph\n      text \"Content on the first line.\""],
    );
}

#[test]
fn markdown_directive_options() {
    md_expect(
        concat!(
            "```{code-block} python\n:linenos:\n:emphasize-lines: 2\n\na\nb\n```\n\n",
            "```{code-block}\n---\nlineno-start: 10\ncaption: |\n  My\n  caption\n---\nx\n```\n",
        ),
        &[
            concat!(
                "code-block value=1 \"a\\nb\\n\" @language=\"python\" @linenos=\"1\" ",
                "@emphasize-lines=\"2\"",
            ),
            concat!(
                "code-block value=10 \"x\\n\" @language=\"default\" @linenos=\"1\" ",
                "@caption=\"My caption\"",
            ),
        ],
    );
    let r = md_read(
        concat!(
            "```{note}\n:unknown: x\n\ntext\n```\n\n```{note}\n:class: ",
            "[1]\n\ntext\n```\n\n```{nosuch}\n```\n",
        ),
        guidedoc_commonmark::Settings::default(),
    );
    for code in
        ["rst.directive.option.unknown", "myst.directive.option-value", "myst.directive.unknown"]
    {
        assert!(r.codes.contains(code), "missing {code} in {}", r.codes);
    }
}

#[test]
fn markdown_roles_are_sphinx_roles() {
    md_expect(
        concat!(
            "{sub}`2` {math}`\\alpha` {ref}`label` {py:func}`os.path.join` ",
            "{doc}`intro` {abbr}`LIFO (last-in, first-out)`\n",
        ),
        &[
            "subscript\n      text \"2\"",
            "math \"\\\\alpha\"",
            "xref link=unresolved name=\"label\" @reftype=\"ref\"",
            "xref link=unresolved name=\"os.path.join\" @reftype=\"py:func\"",
            "xref link=unresolved name=\"intro\" @reftype=\"doc\"",
            "abbreviation @title=\"last-in, first-out\"",
        ],
    );
    md_expect(
        "```{role} raw-latex(raw)\n:format: latex\n```\n\n{raw-latex}`\\x`\n",
        &["raw-inline \"\\\\x\" @format=\"latex\" @class=\"raw-latex\""],
    );
    let r = md_read("{nosuch}`x`\n", guidedoc_commonmark::Settings::default());
    assert!(r.codes.contains("rst.role.unknown"), "{}", r.codes);
}

#[test]
fn markdown_eval_rst_and_math() {
    md_expect(
        concat!(
            "```{eval-rst}\n.. note:: RST *inside*\n\n   `link <https://y.org>`_\n```\n\n",
            "$$\nx = 1\n$$ (eq-a)\n\nSee {eq}`eq-a`.\n",
        ),
        &[
            "admonition value=7\n    paragraph\n      text \"RST \"\n      emphasis\n",
            "link link=external dest=\"https://y.org\"",
            "math-block \"x = 1\" @id=\"equation-eq-a\" @label=\"eq-a\"",
            "xref link=unresolved name=\"eq-a\" @reftype=\"eq\"",
        ],
    );
    let mut ams = guidedoc_commonmark::Settings::default();
    ams.enable_extensions.insert(guidedoc_commonmark::SyntaxExtension::Amsmath);
    md_expect_with(
        "\\begin{align}\na\n\\end{align}\n",
        &[concat!(
            "math-block \"\\\\begin{align}\\na\\n\\\\end{align}\" @class=\"amsmath\" ",
            "@id=\"equation-amsmath-1\" @label=\"amsmath-1\" @nowrap=\"1\"",
        )],
        ams,
    );
}

#[test]
fn markdown_table_and_figure_directives() {
    md_expect(
        concat!(
            "```{table} The *title*\n:name: tbl\n:align: center\n\n| a | b |\n",
            "|---|---|\n| 1 | 2 |\n```\n",
        ),
        &[concat!(
            "table value=2 @class=\"colwidths-auto\" @name=\"tbl\" @align=\"center\"\n",
            "    title\n      text \"The \"\n      emphasis\n        text \"title\"\n",
            "    table-head",
        )],
    );
    md_expect(
        "```{figure} pic.png\n:width: 50%\n\nThe *caption*\n\nA legend.\n```\n",
        &[
            concat!(
                "figure\n    image link=external dest=\"pic.png\" @width=\"50%\"\n",
                "    caption\n      text \"The \"\n      emphasis\n",
            ),
            "legend\n      paragraph\n        text \"A legend.\"",
        ],
    );
    md_expect(
        "```{list-table}\n:header-rows: 1\n\n* - a\n  - *b*\n* - 1\n  - 2\n```\n",
        &["table value=2", "table-head\n      row +header", "emphasis\n"],
    );
}

#[test]
fn markdown_class_directives_and_sections() {
    md_expect(
        "```{rst-class} special\n```\n\nParagraph.\n\n```{class} c\n```\n",
        &[
            "paragraph @class=\"special\"\n    text \"Paragraph.\"",
            "desc @domain=\"py\" @objtype=\"class\"",
        ],
    );
    md_expect(
        "# Top\n\n```{include} part.md\n```\n\nAfter.\n",
        &[
            concat!(
                "section value=1\n    heading value=1\n      text \"Top\"\n",
                "    section value=2\n      heading value=2\n        text \"Part\"\n",
            ),
            "text \"After.\"",
        ],
    );
    md_expect(
        "```{include} sub/inner.md\n:relative-images:\n:relative-docs: ..\n```\n",
        &["image link=external dest=\"sub/pic.png\"", "xref link=unresolved name=\"top\""],
    );
}

#[test]
fn markdown_resolves_as_restructuredtext() {
    md_expect(
        "---\nauthor: A. Writer\n---\n# T\n\n\"Quoted\" text[^a].\n\n[^a]: Note.\n",
        &[
            "field-list @class=\"docinfo\"",
            "field @docinfo=\"author\"",
            "text \"“Quoted” text\"",
            "footnote-reference value=1 +auto \"#a\" link=internal name=\"a\"",
            "footnote value=1 +auto +referenced \"#a\"",
        ],
    );
}
