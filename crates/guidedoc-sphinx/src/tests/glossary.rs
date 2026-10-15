//! Tests of the glossary directive and its terms.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

#[test]
fn glossary_terms_and_ids() {
    expect_dump(
        concat!(
            ".. glossary::\n",
            "   :sorted:\n",
            "\n",
            "   environment\n",
            "      A structure.\n",
            "\n",
            "   API\n",
            "   *Application* interface : iface\n",
            "      Several terms.\n",
            "\n",
            "   .. a comment\n",
            "      still comment\n",
        ),
        concat!(
            "document value=1\n",
            "  glossary @sorted=\"1\"\n",
            "    definition-list @class=\"glossary\"\n",
            "      definition-item\n",
            "        term @id=\"term-API\" @name=\"API\"\n",
            "          text \"API\"\n",
            "          index @entries=\"!single: API\" @targets=\"term-API\"\n",
            "        term @id=\"term-Application-interface\" @name=\"Application interface\"\n",
            "          emphasis\n            text \"Application\"\n",
            "          text \" interface\"\n",
            "          index @entries=\"!single: Application interface\" @key=\"iface\" ",
            "@targets=\"term-Application-interface\"\n",
            "        definition\n          paragraph\n            text \"Several terms.\"\n",
            "      definition-item\n",
            "        term @id=\"term-environment\" @name=\"environment\"\n",
            "          text \"environment\"\n",
            "          index @entries=\"!single: environment\" @targets=\"term-environment\"\n",
            "        definition\n          paragraph\n            text \"A structure.\"\n",
        ),
        None,
    );
}

#[test]
fn glossary_problems_warn() {
    expect_codes(
        ".. glossary::\n\n   a\n      x\n   b\n      y\n",
        "sphinx.glossary.content",
        None,
    );
    expect_codes(
        ".. glossary::\n\n   a\n      x\n\n   a\n      y\n",
        "sphinx.glossary.duplicate",
        None,
    );
}

#[test]
fn productionlist_tokens() {
    expect_dump(
        concat!(
            ".. productionlist::\n",
            "   try_stmt: `try1_stmt` | \"x\"\n",
            "           : `~other:tok`\n",
        ),
        concat!(
            "document value=1\n",
            "  production-list\n",
            "    production \"try_stmt\" @id=\"grammar-token-try_stmt\"\n",
            "      text \" \"\n",
            "      xref link=unresolved name=\"try1_stmt\" @reftype=\"token\"\n",
            "        code \"try1_stmt\"\n",
            "      text \" | \\\"x\\\"\"\n",
            "    production\n",
            "      text \" \"\n",
            "      xref link=unresolved name=\"other:tok\" @reftype=\"token\"\n",
            "        code \"tok\"\n",
        ),
        None,
    );
    expect_contains(
        ".. productionlist:: grammar\n   expr: `term`\n",
        &[
            "production-list @group=\"grammar\"",
            "production \"expr\" @id=\"grammar-token-grammar-expr\"",
            "name=\"grammar:term\"",
        ],
        None,
    );
}

// Sphinx joins a line that ends in a backslash with the next, and ends the productions
// at the first line without a colon.
#[test]
fn productionlist_continued_lines() {
    expect_contains(
        ".. productionlist:: cont\n   A: B C \\\n      E F\n",
        &[
            "production-list @group=\"cont\"",
            "production \"A\" @id=\"grammar-token-cont-A\"",
            "text \" B C    E F\"",
        ],
        None,
    );
    expect_contains(
        ".. productionlist:: FirstLine: x\n   Second: y\n   no colon\n",
        &["production \"FirstLine\"", "production \"Second\""],
        None,
    );
}
