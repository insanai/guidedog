//! The parser is recursive descent over nested templates, declarators, and expressions.
//! Nesting past `max_depth`, or past the stack budget when `max_depth` is huge, is a
//! `Limit`, never a stack overflow.

use std::time::{Duration, Instant};

use guidedoc_cpp::{
    CppError, Directive, ErrorKind, Limits, Stop, parse_declaration, parse_xref_target,
};

fn nested(before: &str, open: &str, n: usize, middle: &str, close: &str, after: &str) -> String {
    format!("{before}{}{middle}{}{after}", open.repeat(n), close.repeat(n))
}

/// A deeply nested case: what it parses as, and its text.
enum Case {
    Declaration(Directive, String),
    Xref(String),
}

fn deep_cases(n: usize) -> Vec<Case> {
    use Case::{Declaration, Xref};
    vec![
        Declaration(Directive::Function, nested("void f(", "A<", n, "int", ">", ")")),
        Declaration(Directive::Function, nested("void f(int ", "*", n, "p", "", ")")),
        Declaration(Directive::Function, nested("void f(int a = ", "(", n, "1", ")", ")")),
        Declaration(Directive::Member, nested("int x = ", "{", n, "1", "}", "")),
        Declaration(Directive::Member, nested("int x = ", "-", n, "1", "", "")),
        Declaration(
            Directive::Function,
            nested("", "template<", n, "typename T", "> class", " void f()"),
        ),
        Xref(nested("", "A<", n, "int", ">", "::f")),
    ]
}

fn parse(case: &Case, limits: &Limits) -> Result<(), CppError> {
    match case {
        Case::Declaration(directive, text) => {
            parse_declaration(text, *directive, limits).map(drop)
        }
        Case::Xref(text) => parse_xref_target(text, true, limits).map(drop),
    }
}

fn text(case: &Case) -> &str {
    let (Case::Declaration(_, t) | Case::Xref(t)) = case;
    &t[..t.len().min(40)]
}

/// Runs `f` on a thread with room for the stack budget and the work below the guard.
fn on_big_stack(f: impl FnOnce() + Send + 'static) {
    let handle = std::thread::Builder::new().stack_size(16 << 20).spawn(f);
    let joined = handle.map(std::thread::JoinHandle::join);
    assert!(matches!(joined, Ok(Ok(()))), "the test thread failed");
}

#[test]
fn deeply_nested_declarations_hit_a_limit() {
    on_big_stack(|| {
        for case in deep_cases(1000) {
            // Parentheses, the costliest nesting, take about 1.5 KiB of stack per level
            // optimized and 4.4 KiB unoptimized for three units of depth, so the depth
            // limit comes first in both builds (see `stack_per_level`).
            let result = parse(&case, &Limits::default());
            let kind = result.as_ref().err().map(|e| e.kind);
            assert_eq!(kind, Some(ErrorKind::Limit(Stop::Depth)), "{}", text(&case));
        }
    });
}

#[test]
fn nesting_past_a_huge_depth_limit_hits_the_stack_limit() {
    on_big_stack(|| {
        let limits = Limits { max_depth: 10_000_000, ..Limits::default() };
        for case in deep_cases(4000) {
            let result = parse(&case, &limits);
            let kind = result.as_ref().err().map(|e| e.kind);
            assert_eq!(kind, Some(ErrorKind::Limit(Stop::Stack)), "{}", text(&case));
            let error = result.err();
            assert_eq!(error.as_ref().map(|e| e.code), Some("core.limit.stack"));
            assert_eq!(error.map(|e| e.limit), Some(Limits::default().stack_bytes));
        }
    });
}

#[test]
fn moderately_nested_declarations_still_parse() {
    on_big_stack(|| {
        for case in deep_cases(12) {
            // Some are invalid signatures (a warning); none may stop at a limit.
            let result = parse(&case, &Limits::default());
            assert!(
                !result.as_ref().is_err_and(CppError::is_limit),
                "{}: {result:?}",
                text(&case)
            );
        }
    });
}

/// A fold is tried only when its `...` is there, so nested parentheses no longer take
/// time exponential in their depth (40 levels took minutes).
#[test]
fn nested_parentheses_in_expressions_parse_in_linear_time() {
    on_big_stack(|| {
        let text = nested("void f(int a = ", "(", 60, "1", ")", ")");
        let start = Instant::now();
        let result = parse_declaration(&text, Directive::Function, &Limits::default());
        assert!(result.is_ok(), "{result:?}");
        assert!(start.elapsed() < Duration::from_secs(10));
    });
}

#[test]
fn a_depth_limit_names_the_limit_it_passed() {
    let limits = Limits { max_depth: 8, ..Limits::default() };
    let text = nested("void f(", "A<", 20, "int", ">", ")");
    let error = parse_declaration(&text, Directive::Function, &limits).err();
    assert_eq!(error.as_ref().map(|e| e.code), Some("core.limit.depth"));
    assert_eq!(error.as_ref().map(|e| e.limit), Some(8));
    assert_eq!(error.map(|e| e.span), Some(0..text.len()));
}

/// Returns the deepest nesting of `make(n)` that parses within `limits` (an invalid
/// signature counts; only a limit does not).
fn deepest(make: &dyn Fn(usize) -> Case, limits: &Limits) -> usize {
    let fits = |n: usize| !parse(&make(n), limits).is_err_and(|e| e.is_limit());
    let (mut ok, mut bad) = (0, 1);
    while fits(bad) {
        (ok, bad) = (bad, bad * 2);
    }
    while bad - ok > 1 {
        let mid = usize::midpoint(ok, bad);
        if fits(mid) { ok = mid } else { bad = mid }
    }
    ok
}

/// Prints, for each deep case, the stack one level of nesting takes and the guarded
/// depth it counts: `cargo test --release -p guidedoc-cpp --test limits -- --ignored
/// --nocapture stack_per_level`.
#[test]
#[ignore = "a measurement"]
fn stack_per_level() {
    on_big_stack(|| {
        let budget = 4 << 20;
        let by_stack = Limits { max_depth: usize::MAX, stack_bytes: budget };
        let by_depth = Limits { max_depth: 4096, stack_bytes: 64 << 20 };
        for k in 0..deep_cases(1).len() {
            let make = |n: usize| deep_cases(n).swap_remove(k);
            let levels = deepest(&make, &by_stack).max(1);
            let depth_levels = deepest(&make, &by_depth).max(1);
            let bytes = budget / levels;
            let units = 4096.0 / depth_levels as f64;
            let at_default = Limits::default().max_depth as f64 / units * bytes as f64;
            println!(
                "{:<28} {bytes:>6} B/level {units:>5.2} depth/level, \
                 max_depth 256 needs {:>4.0} KiB",
                text(&make(2)),
                at_default / 1024.0
            );
        }
    });
}
