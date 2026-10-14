//! Ported from `markup_test.odin`.

use super::*;
use crate::*;
#[test]
fn xml_documents() {
    expect_tokens(
        Lexer::Xml,
        "<?xml version=\"1.0\"?>\n<a x=\"1\" y='2'>t &amp; u<!-- c --></a>",
        concat!(
            "[cp]<?xml version=\"1.0\"?>[w]\\n[nt]<a[w] [na]x=[s]\"1\"[w] [na]y=[s]'2'[nt]>[]t",
            "[w] [ni]&amp;[w] []u[cm]<!-- c -->[nt]</a>"
        ),
    );
    expect_tokens(Lexer::Xml, "<b/><![CDATA[x<y]]>", "[nt]<b/>[cp]<![CDATA[x<y]]>");
}

#[test]
fn html_documents() {
    expect_tokens(
        Lexer::Html,
        "<!DOCTYPE html>\n<p class=\"x\" hidden>Hi &lt;</p>",
        concat!(
            "[cp]<!DOCTYPE html>[]\\n[p]<[nt]p[] [na]class[o]=[s]\"x\"[] [na]hidden[p]>[]Hi ",
            "[ni]&lt;[p]</[nt]p[p]>"
        ),
    );
    expect_tokens(
        Lexer::Html,
        "<script>var x = 1;</script><style>p { color: red }</style>",
        concat!(
            "[p]<[nt]script[p]>[kd]var[w] [nx]x[w] [o]=[w] [mi]1[p];</[nt]script[p]><[nt]style",
            "[p]>[nt]p[w] [p]{[w] [k]color[p]:[w] [kc]red[w] [p]}</[nt]style[p]>"
        ),
    );
}

#[test]
fn css_stylesheets() {
    expect_tokens(
        Lexer::Css,
        concat!(
            "/* c */\n.a > #b:hover, p::before {\n  margin: 0 auto;\n  width: calc(100% - 2.5em);",
            "\n  --gap: 4px;\n  color: #fff !important;\n}\n"
        ),
        concat!(
            "[c]/* c */[w]\\n[p].[nc]a[w] [o]>[w] [p]#[nn]b[p]:[nd]hover[o],[w] [nt]p[p]::",
            "[nd]before[w] [p]{[w]\\n  [k]margin[p]:[w] [mi]0[w] [kc]auto[p];[w]\\n  [k]width",
            "[p]:[w] [nb]calc[p]([mi]100[kt]%[w] [o]-[w] [mf]2.5[kt]em[p]);[w]\\n  [nv]--gap",
            "[p]:[w] [mi]4[kt]px[p];[w]\\n  [k]color[p]:[w] [mh]#fff[w] [cp]!important[p];",
            "[w]\\n[p]}[w]\\n"
        ),
    );
    expect_tokens(
        Lexer::Css,
        "@media print { a { color: red } }\nb { background: url(x.png) }",
        concat!(
            "[p]@[k]media[w] [nt]print[w] [p]{[w] [nt]a[w] [p]{[w] [k]color[p]:[w] [kc]red[w] ",
            "[p]}[w] [p]}[w]\\n[nt]b[w] [p]{[w] [k]background[p]:[w] [nb]url[p]([sx]x.png[p])",
            "[w] [p]}"
        ),
    );
}
