use super::parse_file;
use crate::ast::{Expr, Stmt};

fn wrap(open: &str, inner: &str, close: &str, n: usize) -> String {
    format!("{}{inner}{}", open.repeat(n), close.repeat(n))
}

fn parse(body: &str) -> crate::ast::File {
    parse_file(&format!("package p\n{body}"))
}

#[test]
fn declarations_keep_their_spans_and_comments() {
    let src = "package p\n\n// Doc.\n@(private)\nx: int = 1 // line\n";
    let file = parse_file(src);
    assert_eq!(file.errors, 0);
    let Some(Stmt::Value(d)) = file.decls.first() else {
        panic!("no declaration: {:?}", file.decls);
    };
    assert!(d.mutable && d.docs.is_some() && d.comment.is_some());
    let ty = d.ty.map(|t| file.node(t).span).unwrap_or_default();
    assert_eq!(&src[ty.start as usize..ty.end as usize], "int");
    assert_eq!(d.attributes.len(), 1);
}

#[test]
fn procedure_bodies_are_skipped_but_end_the_literal() {
    let src = "package p\nf :: proc(a: int) -> int where a > 0 {\n\tif a { return 1 }\n}\n";
    let file = parse_file(src);
    assert_eq!(file.errors, 0);
    let Some(Stmt::Value(d)) = file.decls.first() else {
        panic!("no declaration");
    };
    let node = file.node(d.values[0]);
    assert!(matches!(node.kind, Expr::ProcLit { body: Some(_) }));
    assert!(src[node.span.start as usize..node.span.end as usize].ends_with('}'));
}

#[test]
fn files_without_a_package_clause_are_not_read() {
    assert!(parse_file("x :: 1\n").package.is_none());
    assert!(parse_file("package builtin\n").package.is_none());
    assert!(parse_file("").package.is_none());
}

#[test]
fn multiline_strings_are_one_token() {
    let file = parse("S :: \"\"\"\n{ unbalanced\n\"\"\"\nT :: 1\n");
    assert_eq!(file.errors, 0);
    assert_eq!(file.decls.len(), 2);
}

/// Hostile nesting is refused before it can exhaust the stack; long flat chains and
/// what procedure bodies hold (skipped) are read.
#[test]
fn deeply_nested_files_are_not_parsed() {
    const N: usize = 100_000;
    let deep = [
        ("parens", format!("X :: {}\n", wrap("(", "1", ")", N))),
        ("braces", format!("X := {}\n", wrap("{", "1", "}", N))),
        ("calls", format!("X :: {}\n", wrap("f(", "1", ")", N))),
        ("pointers", format!("X :: {}int\n", "^".repeat(N))),
        ("slices", format!("X :: {}int\n", "[]".repeat(N))),
        ("maps", format!("X :: {}int\n", "map[int]".repeat(N))),
        ("procs", format!("X :: {}int\n", "proc() -> ".repeat(N))),
        ("casts", format!("X :: {}1\n", "cast(int)".repeat(N))),
        ("casts on lines", format!("X :: ({}1)\n", "cast(int)\n".repeat(N))),
        ("ternaries", format!("X :: 1{}\n", " if a else 1".repeat(N))),
        ("c ternaries", format!("X :: 1{}\n", " ? 1 : 1".repeat(N))),
        ("when", format!("{}{{}}\n", "when A {} else ".repeat(N))),
        ("nested when", wrap("when A {\n", "", "}\n", N)),
        ("attributes", format!("{}x :: 1\n", "@(a) ".repeat(N))),
    ];
    for (name, body) in &deep {
        assert!(parse(body).too_deep, "{name} was parsed");
    }
    let fine = [
        ("parens", format!("X :: {}\n", wrap("(", "1", ")", 30))),
        ("types", "X :: map[string][dynamic]^[]map[int]^[4]proc() -> ^int\n".to_string()),
        ("plus", format!("X :: 1{}\n", " + 1".repeat(N))),
        ("negation", format!("X :: {}1\n", "-".repeat(N))),
        ("else if", format!("f :: proc() {{\n{}{{}}\n}}\n", "if a {} else ".repeat(N))),
        ("else on lines", format!("f :: proc() {{\n{}{{}}\n}}\n", "if a {\n}\nelse ".repeat(N))),
        ("casts", format!("X := [?]int{{{}}}\n", "cast(int)a, ".repeat(N))),
        ("or chain", format!("when A{} {{}}\n", " || A".repeat(N))),
    ];
    for (name, body) in &fine {
        assert!(!parse(body).too_deep, "{name} was refused");
    }
}
