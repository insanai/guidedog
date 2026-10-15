//! HTML output. Expected HTML is written by hand from Pygments'
//! HtmlFormatter rules and short class names: a span per token, Text bare, one span for
//! neighbours of one class, no span across a newline, and `& < > " '` escaped.

use super::*;
use crate::*;
// Expected HTML is written by hand from Pygments' HtmlFormatter rules and short class
// names: a span per token, Text bare, one span for neighbours of one class, no span
// across a newline, and & < > " ' escaped.

#[test]
fn html_python() {
    assert_eq!(
        html(Lexer::Python, "def foo():\n    return 1\n"),
        concat!(
            "<span class=\"k\">def</span><span class=\"w\"> </span><span class=\"nf\">foo</span>",
            "<span class=\"p\">():</span>\n    <span class=\"k\">return</span> ",
            "<span class=\"mi\">1</span>\n"
        )
    );
    assert_eq!(
        html(Lexer::Python, "x = 'a' # \"c\" & <d>"),
        concat!(
            "<span class=\"n\">x</span> <span class=\"o\">=</span> ",
            "<span class=\"s1\">&#39;a&#39;</span> ",
            "<span class=\"c1\"># &quot;c&quot; &amp; &lt;d&gt;</span>"
        )
    );
    // A token across lines is closed and reopened at the newline.
    assert_eq!(
        html(Lexer::Python, "\"\"\"a\nb\"\"\"\n"),
        concat!(
            "<span class=\"sd\">&quot;&quot;&quot;a</span>\n",
            "<span class=\"sd\">b&quot;&quot;&quot;</span>\n"
        )
    );
    assert_eq!(
        html(Lexer::Python, "@dec\nclass A: pass"),
        concat!(
            "<span class=\"nd\">@dec</span>\n<span class=\"k\">class</span>",
            "<span class=\"w\"> </span><span class=\"nc\">A</span><span class=\"p\">:</span> ",
            "<span class=\"k\">pass</span>"
        )
    );
}

#[test]
fn html_c() {
    assert_eq!(
        html(Lexer::C, "#include <stdio.h>\nint main(void) {\n    printf(\"hi\\n\");\n}\n"),
        concat!(
            "<span class=\"cp\">#include</span><span class=\"w\"> </span>",
            "<span class=\"cpf\">&lt;stdio.h&gt;</span>\n",
            "<span class=\"kt\">int</span><span class=\"w\"> </span>",
            "<span class=\"nf\">main</span><span class=\"p\">(</span>",
            "<span class=\"kt\">void</span><span class=\"p\">)</span>",
            "<span class=\"w\"> </span><span class=\"p\">{</span>\n",
            "<span class=\"w\">    </span><span class=\"n\">printf</span>",
            "<span class=\"p\">(</span><span class=\"s\">&quot;hi</span>",
            "<span class=\"se\">\\n</span><span class=\"s\">&quot;</span>",
            "<span class=\"p\">);</span>\n<span class=\"p\">}</span>\n"
        )
    );
}

#[test]
fn html_json_console_diff() {
    assert_eq!(
        html(Lexer::Json, "{\"a\": [1, true]}"),
        concat!(
            "<span class=\"p\">{</span><span class=\"nt\">&quot;a&quot;</span>",
            "<span class=\"p\">:</span><span class=\"w\"> </span><span class=\"p\">[</span>",
            "<span class=\"mi\">1</span><span class=\"p\">,</span><span class=\"w\"> </span>",
            "<span class=\"kc\">true</span><span class=\"p\">]}</span>"
        )
    );
    assert_eq!(
        html(Lexer::Console, "$ echo hi\nhi\n"),
        concat!(
            "<span class=\"gp\">$ </span><span class=\"nb\">echo</span>",
            "<span class=\"w\"> </span>hi\n<span class=\"go\">hi</span>\n"
        )
    );
    assert_eq!(
        html(Lexer::Diff, "@@ -1 +1 @@\n-a\n+b\n c\n"),
        concat!(
            "<span class=\"gu\">@@ -1 +1 @@</span>\n<span class=\"gd\">-a</span>\n",
            "<span class=\"gi\">+b</span>\n<span class=\"w\"> </span>c\n"
        )
    );
    assert_eq!(html(Lexer::Text, "a < b\n"), "a &lt; b\n");
    assert_eq!(
        html(Lexer::Yaml, "- a"),
        concat!(
            "<span class=\"p p-Indicator\">-</span><span class=\"w\"> </span>",
            "<span class=\"l l-Scalar l-Scalar-Plain\">a</span>"
        )
    );
    assert_eq!(html(Lexer::Python, ""), "");
}

#[test]
fn html_from_tokens_matches_streaming() {
    let mut tokens = Vec::new();
    let mut out = String::new();
    for lexer in Lexer::ALL {
        let code = sample(lexer);
        tokenize_into(lexer, code, &mut tokens);
        out.clear();
        write_html(&tokens, code, &mut out);
        assert_eq!(out, html(lexer, code), "{lexer:?} differs");
    }
}

#[test]
fn buffers_are_reused() {
    let mut tokens = Vec::new();
    tokenize_into(Lexer::Python, sample(Lexer::Python), &mut tokens);
    let capacity = tokens.capacity();
    tokenize_into(Lexer::Python, "x = 1", &mut tokens);
    assert_eq!(tokens.len(), 5);
    assert_eq!(tokens.capacity(), capacity);
    assert_eq!(count_tokens(Lexer::Python, "x = 1"), 5);
}

#[test]
fn one_call_paths() {
    assert_eq!(highlight_html("x = 1", "py"), html(Lexer::Python, "x = 1"));
    assert_eq!(highlight_html("a < b", "no-such-language"), "a &lt; b");
    assert_eq!(highlight_html(">>> 1", "python"), html(Lexer::Pycon, ">>> 1"));
    assert!(highlight_typst("x", "python").starts_with("(((\"n\", \"x\")"));
}

#[test]
fn html_numbered_and_emphasized_lines() {
    let code = "x = \"\"\"a\nb\"\"\"\ny\n";
    let mut out = String::new();
    write_code_html_lines(Lexer::Python, code, HtmlLines { first: 9, emphasized: &[2] }, &mut out);
    assert_eq!(
        out,
        concat!(
            "<span class=\"linenos\"> 9</span><span class=\"n\">x</span> ",
            "<span class=\"o\">=</span> <span class=\"s2\">&quot;&quot;&quot;a</span>\n",
            "<span class=\"hll\"><span class=\"linenos\">10</span>",
            "<span class=\"s2\">b&quot;&quot;&quot;</span>\n</span>",
            "<span class=\"linenos\">11</span><span class=\"n\">y</span>\n"
        )
    );
    out.clear();
    let lines = HtmlLines { emphasized: &[2], ..HtmlLines::default() };
    write_code_html_lines(Lexer::Text, "a\n\nb", lines, &mut out);
    assert_eq!(out, "a\n<span class=\"hll\">\n</span>b");
}
