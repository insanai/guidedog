//! Ported from `code_test.odin`.

use super::*;
use crate::*;
#[test]
fn c_program() {
    expect_tokens(
        Lexer::C,
        "#include <stdio.h>\nint main(void) {\n    return 0;\n}\n",
        concat!(
            "[cp]#include[w] [cpf]<stdio.h>[w]\\n[kt]int[w] [nf]main[p]([kt]void[p])[w] [p]{",
            "[w]\\n    [k]return[w] [mi]0[p];[w]\\n[p]}[w]\\n"
        ),
    );
    expect_tokens(
        Lexer::C,
        "printf(\"hi\\n\", 'x', 0x1F, 1.5e3, 0755); /* c */ // d",
        concat!(
            "[n]printf[p]([s]\"hi[se]\\n[s]\"[p],[w] [sc]'x'[p],[w] [mh]0x1F[p],[w] ",
            "[mf]1.5e3[p],[w] [mo]0755[p]);[w] [cm]/* c */[w] [c1]// d"
        ),
    );
    expect_tokens(
        Lexer::C,
        "#define MAX(a, b) \\\n  ((a) > (b)) // max\nx",
        "[cp]#define MAX(a, b) \\\\n  ((a) > (b)) [c1]// max[w]\\n[n]x",
    );
    expect_tokens(
        Lexer::C,
        "char *s = L\"w\";",
        concat!("[kt]char[w] [o]*[n]s[w] [o]=[w] ", "[sa]L[s]\"w\"[p];"),
    );
}

#[test]
fn cpp_raw_strings() {
    expect_tokens(
        Lexer::Cpp,
        "auto s = R\"x(a)\"\nb)x\";",
        "[k]auto[w] [n]s[w] [o]=[w] [sa]R[s]\"[dl]x([s]a)\"\\nb[dl])x[s]\"[p];",
    );
    expect_tokens(Lexer::Cpp, "u8R\"(raw)\"", "[sa]u8R[s]\"[dl]([s]raw[dl])[s]\"");
    expect_tokens(
        Lexer::Cpp,
        "class Foo : public Bar {};",
        "[k]class[w] [nc]Foo[w] [o]:[w] [k]public[w] [n]Bar[w] [p]{};",
    );
    expect_tokens(
        Lexer::Cpp,
        "namespace a { int x = 1'000; }",
        "[k]namespace[w] [nn]a[w] [p]{[w] [kt]int[w] [n]x[w] [o]=[w] [mi]1'000[p];[w] [p]}",
    );
    // An unterminated raw string runs to the end of the input.
    expect_tokens(Lexer::Cpp, "R\"(abc", "[sa]R[s]\"[dl]([s]abc");
}

#[test]
fn rust_lifetimes_and_chars() {
    expect_tokens(
        Lexer::Rust,
        "fn f<'a>(x: &'a str) -> char { 'x' }",
        concat!(
            "[k]fn[w] [nf]f[o]<[nl]'a[o]>[p]([n]x[o]:[w] [o]&[nl]'a[w] [kt]str[p])[w] ",
            "[o]->[w] [kt]char[w] [p]{[w] [sc]'x'[w] [p]}"
        ),
    );
    expect_tokens(
        Lexer::Rust,
        "let c = '\\n'; let s: &'static str = \"\\u{1F600}\";",
        concat!(
            "[kd]let[w] [n]c[w] [o]=[w] [sc]'\\n'[p];[w] [kd]let[w] [n]s[o]:[w] [o]&",
            "[nl]'static[w] [kt]str[w] [o]=[w] [s]\"[se]\\u{1F600}[s]\"[p];"
        ),
    );
    expect_tokens(
        Lexer::Rust,
        "println!(\"{}\", b'a', r#\"x\"y\"#);",
        "[fm]println![p]([s]\"{}\"[p],[w] [sa]b[sc]'a'[p],[w] [sa]r[s]#\"x\"y\"#[p]);",
    );
    expect_tokens(
        Lexer::Rust,
        "#[derive(Debug)]\nstruct P;\n/* a /* b */ c */ ///doc",
        concat!(
            "[cp]#[derive(Debug)][w]\\n[k]struct[w] [nc]P[p];[w]\\n[cm]/* a /* b */ c */[w] ",
            "[sd]///doc"
        ),
    );
}

#[test]
fn go_programs() {
    expect_tokens(
        Lexer::Go,
        "package main\nfunc (r *T) Name() string { return `raw` }",
        concat!(
            "[kn]package[w] [nn]main[w]\\n[kd]func[w] [p]([nx]r[w] [o]*[nx]T[p])[w] [nf]Name",
            "[p]()[w] [kt]string[w] [p]{[w] [k]return[w] [s]`raw`[w] [p]}"
        ),
    );
    expect_tokens(
        Lexer::Go,
        "x := len(s) + 'a'",
        concat!("[nx]x[w] [o]:=[w] [nb]len[p]([nx]s[p])[w] ", "[o]+[w] [sc]'a'"),
    );
}

#[test]
fn java_programs() {
    expect_tokens(
        Lexer::Java,
        "@Override\npublic String get(int i) { return this.name; }",
        concat!(
            "[nd]@Override[w]\\n[k]public[w] [n]String[w] [nf]get[p]([kt]int[w] [n]i[p])[w] ",
            "[p]{[w] [k]return[w] [k]this[p].[na]name[p];[w] [p]}"
        ),
    );
    expect_tokens(Lexer::Java, "class A {}", "[kd]class[w] [nc]A[w] [p]{}");
}

#[test]
fn kotlin_programs() {
    expect_tokens(
        Lexer::Kotlin,
        "fun greet(n: String) = \"Hi $n, ${n.length}\"",
        concat!(
            "[kd]fun[w] [nf]greet[p]([n]n[o]:[w] [kt]String[p])[w] [o]=[w] [s]\"Hi [si]$n[s], ",
            "[si]${[n]n[o].[n]length[si]}[s]\""
        ),
    );
}

#[test]
fn csharp_programs() {
    expect_tokens(
        Lexer::Csharp,
        "var s = $\"{x} {{y}}\" + @\"C:\\a\"\"b\";",
        concat!(
            "[k]var[w] [n]s[w] [o]=[w] [s]$\"[si]{[n]x[si]}[s] [se]{{[s]y[se]}}[s]\"[w] [o]+",
            "[w] [s]@\"C:\\a[se]\"\"[s]b\"[p];"
        ),
    );
    expect_tokens(
        Lexer::Csharp,
        "namespace App { class P { static void Main() {} } }",
        concat!(
            "[k]namespace[w] [nn]App[w] [p]{[w] [k]class[w] [nc]P[w] [p]{[w] [k]static[w] ",
            "[kt]void[w] [nf]Main[p]()[w] [p]{}[w] [p]}[w] [p]}"
        ),
    );
}

#[test]
fn swift_programs() {
    expect_tokens(
        Lexer::Swift,
        "func hi(_ n: String) -> String { \"Hi \\(n)\" }",
        concat!(
            "[kd]func[w] [nf]hi[p]([n]_[w] [n]n[o]:[w] [nb]String[p])[w] [o]->[w] [nb]String",
            "[w] [p]{[w] [s]\"Hi [si]\\([n]n[si])[s]\"[w] [p]}"
        ),
    );
}

#[test]
fn zig_programs() {
    expect_tokens(
        Lexer::Zig,
        "const std = @import(\"std\");\nconst s =\n    \\\\line\n;",
        concat!(
            "[k]const[w] [n]std[w] [o]=[w] [nb]@import[p]([s]\"std\"[p]);[w]\\n[k]const[w] ",
            "[n]s[w] [o]=[w]\\n    [sh]\\\\line[w]\\n[p];"
        ),
    );
}

#[test]
fn odin_programs() {
    expect_tokens(
        Lexer::Odin,
        "package x\n@(private)\nf :: proc(a: int) -> bool { return #force_inline g(a) }",
        concat!(
            "[kn]package[w] [nn]x[w]\\n[nd]@(private)[w]\\n[nf]f[w] [o]::[w] [k]proc[p]([n]a[o]:",
            "[w] [kt]int[p])[w] [o]->[w] [kt]bool[w] [p]{[w] [k]return[w] [kp]#force_inline[w] ",
            "[n]g[p]([n]a[p])[w] [p]}"
        ),
    );
    expect_tokens(
        Lexer::Odin,
        "P :: struct { s: string `raw` }",
        "[nc]P[w] [o]::[w] [k]struct[w] [p]{[w] [n]s[o]:[w] [kt]string[w] [s]`raw`[w] [p]}",
    );
}

#[test]
fn javascript_programs() {
    expect_tokens(
        Lexer::Javascript,
        "const re = /a[/]b/g; let x = a / b; `t ${x + 1}`",
        concat!(
            "[kd]const[w] [nx]re[w] [o]=[w] [sr]/a[/]b/g[p];[w] [kd]let[w] [nx]x[w] [o]=[w] ",
            "[nx]a[w] [o]/[w] [nx]b[p];[w] [sb]`t [si]${[nx]x[w] [o]+[w] [mi]1[si]}[sb]`"
        ),
    );
    expect_tokens(
        Lexer::Javascript,
        "if (x) return new Map(); console.log('s', \"d\", $el)",
        concat!(
            "[k]if[w] [p]([nx]x[p])[w] [k]return[w] [ow]new[w] [nb]Map[p]();[w] [nb]console",
            "[p].[nx]log[p]([s1]'s'[p],[w] [s2]\"d\"[p],[w] [nx]$el[p])"
        ),
    );
}

#[test]
fn typescript_programs() {
    expect_tokens(
        Lexer::Typescript,
        "interface P { name: string }\n@dec class C {}",
        concat!(
            "[kr]interface[w] [nx]P[w] [p]{[w] [nx]name[o]:[w] [kt]string[w] [p]}[w]\\n",
            "[nd]@dec[w] [kd]class[w] [nx]C[w] [p]{}"
        ),
    );
}

#[test]
fn powershell_scripts() {
    expect_tokens(
        Lexer::Powershell,
        "if ($x -eq 1) { Write-Host \"v: $x `n\" } # c",
        concat!(
            "[k]if[w] [p]([nv]$x[w] [ow]-eq[w] [mi]1[p])[w] [p]{[w] [nb]Write-Host[w] ",
            "[s2]\"v: [nv]$x[s2] [se]`n[s2]\"[w] [p]}[w] [c1]# c"
        ),
    );
}

#[test]
fn sql_queries() {
    expect_tokens(
        Lexer::Sql,
        "SELECT name, count(*) FROM t WHERE x = 'it''s' -- c",
        concat!(
            "[k]SELECT[w] [n]name[p],[w] [k]count[p]([o]*[p])[w] [k]FROM[w] [n]t[w] [k]WHERE",
            "[w] [n]x[w] [o]=[w] [s1]'it[se]''[s1]s'[w] [c1]-- c"
        ),
    );
    expect_tokens(
        Lexer::Sql,
        "create table t (id integer primary key);",
        concat!(
            "[k]create[w] [k]table[w] [n]t[w] [p]([n]id[w] [nb]integer[w] [k]primary[w] ",
            "[k]key[p]);"
        ),
    );
}

#[test]
fn dot_graphs() {
    expect_tokens(
        Lexer::Dot,
        "digraph G {\n  a -> b [label=\"x\", shape=<<b>B</b>>];\n}",
        concat!(
            "[k]digraph[w] [n]G[w] [p]{[w]\\n  [n]a[w] [o]->[w] [n]b[w] [p][[na]label[o]=",
            "[s]\"x\"[p],[w] [na]shape[o]=[s]<<b>B</b>>[p]];[w]\\n[p]}"
        ),
    );
}
