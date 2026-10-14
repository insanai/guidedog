//! Hostile source must give bounded output in bounded time, with a warning where
//! something is left out.

mod common;

use std::fmt::Write;
use std::time::Instant;

use gd_autodoc::{Config, DirectiveOption, Output, Request, document};

fn run_directive(files: &[(&str, &str)], request: Request) -> Output {
    let root = common::temp_root(files);
    let mut session = common::session(&root);
    document(&mut session, &Config::default(), &request)
}

fn request(directive: &str, argument: &str, options: Vec<DirectiveOption>) -> Request {
    Request {
        directive: directive.into(),
        argument: argument.into(),
        options,
        ..Request::default()
    }
}

#[test]
fn multiplying_aliases_and_constants_stay_bounded() {
    // Each alias and constant uses the next four times: 4^20 nodes when expanded.
    let mut b = String::new();
    for i in 0..20 {
        let n = i + 1;
        let _ = writeln!(b, "T{i} = tuple[T{n}, T{n}, T{n}, T{n}]");
        let _ = writeln!(b, "C{i} = (C{n}, C{n}, C{n}, C{n})");
    }
    b.push_str("T20 = int\nC20 = 1\n\ndef f(x: T0 = C0) -> T0:\n    '''Doc.'''\n");
    let start = Instant::now();
    let out = run_directive(&[("alias.py", &b)], request("autofunction", "alias.f", vec![]));
    assert!(start.elapsed().as_secs() < 20);
    assert!(!out.lines.is_empty() && out.lines.len() < 100);
    let total: usize = out.lines.iter().map(String::len).sum();
    assert!(total < 1 << 20, "{total} bytes of output");
}

#[test]
fn classes_that_contain_themselves_stop_with_a_warning() {
    // Each nested class brings the outer class back in under its own name, three times
    // over: 3^24 directives when followed to the depth limit.
    let source = r#"class Outer:
    """Outer."""

    class A:
        """A."""
        from loop import Outer

    class B:
        """B."""
        from loop import Outer

    class C:
        """C."""
        from loop import Outer
"#;
    let members = vec![DirectiveOption::flag("members")];
    let out = run_directive(&[("loop.py", source)], request("autoclass", "loop.Outer", members));
    assert!(out.has_message("autodoc.limit"));
    assert!(out.lines.len() < 40 * 10_000);
}

#[test]
fn too_deeply_nested_modules_are_reported() {
    let deep = format!("X = 1{}\n", " + 1".repeat(100_000));
    let out = run_directive(&[("deep.py", &deep)], request("autodata", "deep.X", vec![]));
    assert!(out.has_message("autodoc.too_deep"));
    let message = out.messages.iter().find(|m| m.code == "autodoc.too_deep").expect("reported");
    assert!(message.to_string().starts_with("-- SOURCE NESTS TOO DEEPLY "));
}

#[test]
fn unknown_directives_and_missing_modules_are_reported() {
    let out = run_directive(&[("m.py", "X = 1\n")], request("autonothing", "m.X", vec![]));
    assert!(out.has_message("autodoc.directive") && out.lines.is_empty());
    let out = run_directive(&[("m.py", "X = 1\n")], request("autodata", "nowhere.X", vec![]));
    assert!(out.has_message("autodoc.import") && out.lines.is_empty());
}
