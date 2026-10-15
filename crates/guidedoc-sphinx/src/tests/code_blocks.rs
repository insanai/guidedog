//! Ported from the Odin package's code_test.odin.

#[allow(unused_imports)]
use crate::config::{Config, TocParents};
#[allow(unused_imports)]
use crate::tests::*;

#[test]
fn code_block_sphinx_options() {
    expect_dump(
        concat!(
            ".. code-block:: python\n",
            "   :linenos:\n",
            "   :lineno-start: 10\n",
            "   :emphasize-lines: 1,3-4\n",
            "   :caption: An *example*\n",
            "   :name: ex-code\n",
            "   :class: wide\n",
            "   :force:\n",
            "\n",
            "   a = 1\n",
            "   b = 2\n",
            "   c = 3\n",
            "   d = 4\n",
        ),
        concat!(
            "document value=1\n",
            "  code-block value=10 \"a = 1\\nb = 2\\nc = 3\\nd = 4\\n\" @language=\"python\" ",
            "@class=\"wide\" @linenos=\"1\" @force=\"1\" @name=\"ex-code\" ",
            "@emphasize-lines=\"1,3-4\" @caption=\"An *example*\"\n",
            "    caption\n      text \"An \"\n      emphasis\n        text \"example\"\n",
        ),
        None,
    );
}

#[test]
fn code_block_language_follows_highlight() {
    expect_dump(
        concat!(
            ".. code-block::\n\n   x\n\n",
            ".. highlight:: c\n   :linenothreshold: 2\n\n",
            ".. sourcecode::\n\n   int a;\n   int b;\n\n",
            ".. code::\n\n   one\n",
        ),
        concat!(
            "document value=1\n",
            "  code-block \"x\\n\" @language=\"default\"\n",
            "  code-block value=1 \"int a;\\nint b;\\n\" @language=\"c\" @linenos=\"1\"\n",
            "  code-block \"one\\n\" @language=\"c\"\n",
        ),
        None,
    );
}

#[test]
fn code_block_dedent() {
    expect_dump(
        concat!(
            ".. code-block:: text\n   :dedent: 2\n\n     a\n       b\n\n",
            ".. code-block:: text\n   :dedent:\n\n     a\n       b\n",
        ),
        concat!(
            "document value=1\n",
            "  code-block \"a\\n  b\\n\" @language=\"text\"\n",
            "  code-block \"a\\n  b\\n\" @language=\"text\"\n",
        ),
        None,
    );
    expect_codes(
        ".. code-block:: text\n   :dedent: 3\n\n     a\n    b\n",
        "sphinx.code.dedent",
        None,
    );
}

#[test]
fn code_block_bad_emphasis_is_refused() {
    let r = read_text(".. code-block:: text\n   :emphasize-lines: 3-1\n\n   a\n", None);
    assert_eq!(r.dump, "document value=1\n");
    assert_eq!(r.codes, "sphinx.code.lines");
    expect_codes(
        ".. code-block:: text\n   :emphasize-lines: 5\n\n   a\n",
        "sphinx.code.lines",
        None,
    );
}

#[test]
fn literalinclude_selection() {
    expect_dump(
        concat!(
            ".. literalinclude:: lines.txt\n   :lines: 2-3\n   :lineno-match:\n\n",
            ".. literalinclude:: lines.txt\n   :start-after: one\n   :end-before: four\n",
            "   :language: text\n\n",
            ".. literalinclude:: lines.txt\n   :start-at: two\n   :end-at: three\n",
            "   :prepend: begin\n   :append: end\n   :caption:\n",
        ),
        concat!(
            "document value=1\n",
            "  code-block value=2 \"two\\nthree\\n\" @language=\"default\" @linenos=\"1\"\n",
            "  code-block \"two\\nthree\\n\" @language=\"text\"\n",
            "  code-block \"begin\\ntwo\\nthree\\nend\\n\" @language=\"default\" ",
            "@caption=\"lines.txt\"\n    caption\n      text \"lines.txt\"\n",
        ),
        None,
    );
}

#[test]
fn literalinclude_pyobject_and_dedent() {
    expect_dump(
        concat!(
            ".. literalinclude:: example.py\n   :pyobject: hello\n   :lineno-match:\n\n",
            ".. literalinclude:: example.py\n   :pyobject: Greeter.greet\n   :dedent: 4\n",
        ),
        concat!(
            "document value=1\n",
            "  code-block value=4 \"@decorator\\ndef hello(name):\\n    ",
            "\\\"\\\"\\\"Greet.\\\"\\\"\\\"\\n",
            "    print(name)\\n\" @language=\"default\" @linenos=\"1\"\n",
            "  code-block \"def greet(self):\\n    return 1\\n\" @language=\"default\"\n",
        ),
        None,
    );
    expect_codes(
        ".. literalinclude:: example.py\n   :pyobject: Missing\n",
        "sphinx.literalinclude.pattern",
        None,
    );
}

#[test]
fn literalinclude_diff() {
    expect_dump(
        ".. literalinclude:: new.txt\n   :diff: old.txt\n",
        concat!(
            "document value=1\n",
            "  code-block \"--- old.txt\\n+++ new.txt\\n@@ -1,4 +1,5 @@\\n a\\n-b\\n+B\\n c\\n",
            " d\\n+e\\n\" @language=\"udiff\"\n",
        ),
        None,
    );
}

#[test]
fn literalinclude_tab_width_and_errors() {
    expect_dump(
        ".. literalinclude:: tabs.txt\n   :tab-width: 4\n",
        "document value=1\n  code-block \"    x\\n\" @language=\"default\"\n",
        None,
    );
    expect_codes(
        ".. literalinclude:: lines.txt\n   :start-at: a\n   :start-after: b\n",
        "sphinx.literalinclude.options",
        None,
    );
    expect_codes(
        ".. literalinclude:: lines.txt\n   :encoding: latin-1\n",
        "sphinx.encoding",
        None,
    );
    expect_codes(".. literalinclude:: nothere.txt\n", "core.include.missing", None);
    expect_codes(
        ".. literalinclude:: lines.txt\n   :lines: 1,3\n   :lineno-match:\n",
        "sphinx.literalinclude.options",
        None,
    );
}

#[test]
fn math_directive_labels() {
    expect_dump(
        concat!(
            ".. math:: e^{i\\pi} + 1 = 0\n   :label: Euler_1\n\n",
            ".. math::\n   :nowrap:\n\n   a\n\n   b\n\n",
            "See :eq:`Euler_1` and :math:numref:`Euler_1`.\n",
        ),
        concat!(
            "document value=1\n",
            "  math-block \"e^{i\\\\pi} + 1 = 0\" @id=\"equation-euler-1\" @label=\"Euler_1\"\n",
            "  math-block \"a\\n\\nb\" @nowrap=\"1\"\n",
            "  paragraph\n    text \"See \"\n",
            "    xref link=unresolved name=\"Euler_1\" @reftype=\"eq\"\n      code \"Euler_1\"\n",
            "    text \" and \"\n",
            "    xref link=unresolved name=\"Euler_1\" @reftype=\"math:numref\"\n",
            "      code \"Euler_1\"\n    text \".\"\n",
        ),
        None,
    );
}

#[test]
fn math_number_all_labels_every_equation() {
    let mut config = Config::default();
    config.math_number_all = true;
    config.docname = "intro".into();
    expect_dump(
        ".. math:: x\n",
        concat!(
            "document value=1\n",
            "  math-block \"x\" @id=\"equation-intro-0\" @label=\"intro:0\"\n",
        ),
        Some(&config),
    );
    // An empty :label: asks for the same label, as in Sphinx's test-ext-math.
    config.math_number_all = false;
    expect_dump(
        ".. math:: x\n\n.. math:: y\n   :label:\n",
        concat!(
            "document value=1\n  math-block \"x\"\n",
            "  math-block \"y\" @id=\"equation-intro-0\" @label=\"intro:0\"\n",
        ),
        Some(&config),
    );
}
