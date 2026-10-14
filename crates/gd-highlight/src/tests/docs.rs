//! Ported from `docs_test.odin`.

use super::*;
use crate::*;
#[test]
fn rst_documents() {
    expect_tokens(
        Lexer::Rst,
        "Title\n=====\n\nSome *emph*, **strong**, ``lit`` and :ref:`x`.\n",
        concat!(
            "[gh]Title[]\\n[gh]=====[]\\n\\nSome [ge]*emph*[], [gs]**strong**[], ",
            "[s]``lit``[] and ",
            "[na]:ref:[nv]`x`[].\\n"
        ),
    );
    expect_tokens(
        Lexer::Rst,
        ".. note:: Hi\n\n- item\n1. first\n:field: v\n",
        "[p]..[] [ow]note[p]::[] Hi\\n\\n[m]-[] item\\n[m]1.[] first\\n[nc]:field:[] v\\n",
    );
    expect_tokens(
        Lexer::Rst,
        "Example::\n\n    raw *text*\n\nAfter.\n",
        "[]Example[se]::[]\\n\\n[s]    raw *text*\\n[]\\nAfter.\\n",
    );
    expect_tokens(
        Lexer::Rst,
        ".. _label:\n.. a comment\n   more\n\n`Link <http://x>`_\n",
        concat!(
            "[p]..[] [nt]_label:[]\\n[cp].. a comment\\n   more\\n[]\\n[s]`Link [si]<http://x>",
            "[s]`_[]\\n"
        ),
    );
}

#[test]
fn rst_code_blocks() {
    expect_tokens(
        Lexer::Rst,
        ".. code-block:: python\n   :caption: x\n\n   def f(): pass\n\nEnd\n",
        concat!(
            "[p]..[] [ow]code-block[p]::[] python\\n   [nc]:caption:[] x\\n\\n   [k]def[w] [nf]f",
            "[p]():[] [k]pass[w]\\n[]\\nEnd\\n"
        ),
    );
}

#[test]
fn markdown_documents() {
    expect_tokens(
        Lexer::Markdown,
        "# Title\n## Sub\n- a *b* **c** `d`\n1. [x](http://y)\n> q\n",
        concat!(
            "[gh]# Title[]\\n[gu]## Sub[]\\n[k]-[w] []a [ge]*b*[] [gs]**c**[] [sb]`d`[]\\n",
            "[k]1.[] [[nt]x[]]([na]http://y[])\\n[k]> [ge]q[]\\n"
        ),
    );
    expect_tokens(
        Lexer::Markdown,
        "```python\nprint(1)\n```\ntext\n",
        "[sb]```[nl]python[]\\n[nb]print[p]([mi]1[p])[w]\\n[sb]```[]\\ntext\\n",
    );
    expect_tokens(Lexer::Markdown, "```\nraw\n```\n", "[sb]```\\nraw\\n```[]\\n");
}

#[test]
fn latex_documents() {
    expect_tokens(
        Lexer::Latex,
        "\\section*{Hi} % c\n$x^2 + 1$ and \\[a\\]",
        concat!(
            "[k]\\section*[nb]{[]Hi[nb]}[] [c]% c\\n[s]$[nb]x^[m]2[nb] [o]+[nb] [m]1[s]$[] and ",
            "[sb]\\[[nb]a[sb]\\]"
        ),
    );
}

#[test]
fn typst_documents() {
    expect_tokens(
        Lexer::Typst,
        concat!(
            "= Title\n#set text(size: 11pt)\nSome *bold* and _it_ `raw` $x + 1$ <l> @l\n",
            "#figure(caption: [A])[B] // c\n"
        ),
        concat!(
            "[gh]= Title[]\\n[k]#set[w] [nf]text[p]([n]size[o]:[w] [mi]11[kt]pt[p])[]\\nSome ",
            "[gs]*bold*[] and [ge]_it_[] [sb]`raw`[] [p]$[nv]x[w] [o]+[w] [m]1[p]$[] [nl]<l>[] ",
            "[nl]@l[]\\n[nf]#figure[p]([n]caption[o]:[w] [p][[]A[p]])[[]B[p]][] [c1]// c[]\\n"
        ),
    );
    expect_tokens(
        Lexer::Typst,
        "#let f(x) = x * 2\n#import \"a.typ\": b\n",
        concat!(
            "[kd]#let[w] [nf]f[p]([n]x[p])[w] [o]=[w] [n]x[w] [o]*[w] [mi]2[]\\n[kn]#import[w] ",
            "[s2]\"a.typ\"[o]:[w] [n]b[]\\n"
        ),
    );
}

#[test]
fn diff_documents() {
    expect_tokens(
        Lexer::Diff,
        "diff --git a/x b/x\nindex 1..2\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n ctx\n-old\n+new\n",
        concat!(
            "[gh]diff --git a/x b/x[w]\\n[gh]index 1..2[w]\\n[gd]--- a/x[w]\\n[gi]+++ b/x[w]\\n",
            "[gu]@@ -1,2 +1,2 @@[w]\\n []ctx[w]\\n[gd]-old[w]\\n[gi]+new[w]\\n"
        ),
    );
    expect_tokens(
        Lexer::Diff,
        "3c3\n< a\n---\n> b\nplain",
        concat!("[gu]3c3[w]\\n[gd]< a[w]\\n[gs]---", "[w]\\n[gi]> b[w]\\n[]plain"),
    );
}

#[test]
fn guessing() {
    assert_eq!(guess_lexer("#!/bin/bash\necho hi\n"), Lexer::Bash);
    assert_eq!(guess_lexer("def f(x):\n    return x\n"), Lexer::Python);
    assert_eq!(guess_lexer("{\"a\": 1}"), Lexer::Json);
    assert_eq!(guess_lexer("#include <stdio.h>\nint main(void) {}"), Lexer::C);
    assert_eq!(guess_lexer("fn main() {\n    println!(\"hi\");\n}"), Lexer::Rust);
    assert_eq!(guess_lexer("digraph { a -> b }"), Lexer::Dot);
    assert_eq!(guess_lexer("just some words"), Lexer::Text);
    assert_eq!(resolve_lexer(Lexer::Guess, "SELECT * FROM t WHERE x = 1;"), Lexer::Sql);
}
