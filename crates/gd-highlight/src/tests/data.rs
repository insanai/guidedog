//! Ported from `data_test.odin`.

use super::*;
use crate::*;
#[test]
fn json_documents() {
    expect_tokens(
        Lexer::Json,
        "{\"a\": [1, -2.5e3, true, null], \"b\" : \"x\\\"y\"}",
        concat!(
            "[p]{[nt]\"a\"[p]:[w] [p][[mi]1[p],[w] [mf]-2.5e3[p],[w] [kc]true[p],[w] [kc]null",
            "[p]],[w] [nt]\"b\"[w] [p]:[w] [s2]\"x\\\"y\"[p]}"
        ),
    );
    expect_tokens(Lexer::Json, "// c\n/* d */ []", "[c1]// c[w]\\n[cm]/* d */[w] [p][]");
}

#[test]
fn json_errors() {
    expect_tokens(
        Lexer::Json,
        "{\"a\": tru, 'b': @}",
        "[p]{[nt]\"a\"[p]:[w] [err]tru[p],[w] [err]'b'[p]:[w] [err]@[p]}",
    );
    expect_tokens(Lexer::Json, "[\"open", "[p][[err]\"open");
    assert!(has_errors(Lexer::Json, "{a: 1}"));
    assert!(!has_errors(Lexer::Json, "{\"a\": {\"b\": [0.5]}}"));
}

/// Expands `{P}` to the plain-scalar classes and `{I}` to the indicator classes.
fn yaml(expected: &str) -> String {
    expected.replace("{P}", "[l l-Scalar l-Scalar-Plain]").replace("{I}", "[p p-Indicator]")
}

#[test]
fn yaml_documents() {
    expect_tokens(
        Lexer::Yaml,
        "---\nname: Guidedog # c\nitems:\n  - one\n  - {a: 1, b: [x]}\n",
        &yaml(concat!(
            "[nn]---[w]\\n[nt]name[p]:[w] {P}Guidedog[w] [c1]# c[w]\\n[nt]items[p]:[w]\\n  ",
            "{I}-[w] {P}one[w]\\n  {I}-[w] {I}{[nt]a[p]:[w] {P}1{I},[w] [nt]b[p]:[w] {I}[{P}x",
            "{I}]}[w]\\n"
        )),
    );
    expect_tokens(
        Lexer::Yaml,
        "run: |\n  echo a\n\n  echo b\nnext: &x 'q' # c\nref: *x\n",
        &yaml(concat!(
            "[nt]run[p]:[w] {I}|[w]\\n  [no]echo a[w]\\n\\n  [no]echo b[w]\\n[nt]next[p]:",
            "[w] [nl]&x[w] [s]'q'[w] [c1]# c[w]\\n[nt]ref[p]:[w] [nv]*x[w]\\n"
        )),
    );
}

#[test]
fn toml_documents() {
    expect_tokens(
        Lexer::Toml,
        concat!(
            "[project]\nname = \"gd\" # c\nversion = 1_000\nwhen = 1979-05-27T07:32:00Z\n",
            "tags = [\"a\", 'b']\npoint = { x = 1.5, y = inf }\nok = true"
        ),
        concat!(
            "[k][project][w]\\n[n]name[w] [o]=[w] [s2]\"gd\"[w] [c1]# c[w]\\n[n]version[w] ",
            "[o]=[w] [mi]1_000[w]\\n[n]when[w] [o]=[w] [ld]1979-05-27T07:32:00Z[w]\\n[n]tags",
            "[w] [o]=[w] [p][[s2]\"a\"[p],[w] [s1]'b'[p]][w]\\n[n]point[w] [o]=[w] [p]{[w] ",
            "[n]x[w] [o]=[w] [mf]1.5[p],[w] [n]y[w] [o]=[w] [mf]inf[w] [p]}[w]\\n[n]ok[w] ",
            "[o]=[w] [kc]true"
        ),
    );
    expect_tokens(
        Lexer::Toml,
        "s = \"\"\"\na\\tb\"\"\"",
        "[n]s[w] [o]=[w] [s2]\"\"\"\\na[se]\\t[s2]b\"\"\"",
    );
}

#[test]
fn ini_documents() {
    expect_tokens(
        Lexer::Ini,
        "[section]\nkey = value ; c\n# d\nflag\n",
        concat!(
            "[k][section][w]\\n[na]key[w] [o]=[w] [s]value [c1]; c[w]\\n[c1]# d[w]\\n",
            "[na]flag[w]\\n"
        ),
    );
}
