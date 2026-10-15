//! Hostile nesting and recursion end in a Limit error, never a stack overflow.

#![allow(clippy::unwrap_used)]

mod common;

use common::{check_with, on_big_stack, run};
use gd_jinja::{
    DEFAULT_MAX_RECURSION, DEFAULT_STACK_BYTES, Environment, ErrorKind, Options, Value,
};

const DEEP: usize = 100_000;

/// A budget the default limits must fit in with room to spare: half the default in an
/// optimized build, all of it in an unoptimized one, whose frames are larger.
const MARGIN: usize =
    if cfg!(debug_assertions) { DEFAULT_STACK_BYTES } else { DEFAULT_STACK_BYTES / 2 };

fn wrap(open: &str, inner: &str, close: &str, n: usize) -> String {
    format!("{}{inner}{}", open.repeat(n), close.repeat(n))
}

#[track_caller]
fn expect_limit(what: &str, source: &str, want: &str, options: Options) {
    let err = run(source, "", options, &[]).expect_err(what);
    assert_eq!(err.kind, ErrorKind::Limit, "{what}: {}", err.message);
    assert!(err.message.contains(want), "{what}: {:?} lacks {want:?}", err.message);
}

#[test]
fn deeply_nested_syntax_is_a_limit_error() {
    let nests = "nests more than 100 levels";
    let expression = |inner: String| format!("{{{{ {inner} }}}}");
    let cases = [
        ("parentheses", expression(wrap("(", "1", ")", DEEP))),
        ("lists", expression(wrap("[", "1", "]", DEEP))),
        ("dicts", expression(wrap("{'a': ", "1", "}", DEEP))),
        ("calls", expression(wrap("f(", "1", ")", DEEP))),
        ("tags", wrap("{% if 1 %}", "x", "{% endif %}", DEEP)),
        ("elif", format!("{{% if 0 %}}{}{{% endif %}}", "{% elif 0 %}".repeat(DEEP))),
        // Chains build left-deep trees, so they nest too.
        ("plus", expression(format!("1{}", " + 1".repeat(DEEP)))),
        ("and", expression(format!("1{}", " and 1".repeat(DEEP)))),
        ("not", expression(format!("{}1", "not ".repeat(DEEP)))),
        ("minus", expression(format!("{}1", "- ".repeat(DEEP)))),
        ("filters", expression(format!("1{}", "|abs".repeat(DEEP)))),
        ("attributes", expression(format!("x{}", ".a".repeat(DEEP)))),
        ("tests", expression(format!("1{}", " is number".repeat(DEEP)))),
        ("conditions", expression(format!("1{}", " if 1 else 2".repeat(DEEP)))),
    ];
    for (what, source) in cases {
        expect_limit(what, &source, nests, Options::default());
    }
    let err = Environment::default().evaluate(&wrap("(", "1", ")", DEEP), &Value::default());
    assert_eq!(err.unwrap_err().kind, ErrorKind::Limit);
}

#[test]
fn nesting_up_to_the_limit_renders() {
    // 98 brackets, plus the output expression and its tag, is 100 levels.
    let expression = |inner: String| format!("{{{{ {inner} }}}}");
    for budget in [DEFAULT_STACK_BYTES, MARGIN] {
        let options = || Options { stack_bytes: budget, ..Options::default() };
        check_with(&expression(wrap("(", "7", ")", 98)), "7", "", options(), &[]);
        check_with(
            &expression(wrap("[", "7", "]", 98)),
            &wrap("[", "7", "]", 98),
            "",
            options(),
            &[],
        );
        check_with(&expression(wrap("{'a': ", "7", "}", 98) + "|length"), "1", "", options(), &[]);
        check_with(&wrap("{% if 1 %}", "x", "{% endif %}", 99), "x", "", options(), &[]);
        check_with(&expression(format!("1{}", " + 1".repeat(98))), "99", "", options(), &[]);
    }
    let raised = || Options { max_nesting: 120, ..Options::default() };
    expect_limit("raised", &expression(wrap("(", "7", ")", 150)), "levels", raised());
    check_with(&expression(wrap("(", "7", ")", 110)), "7", "", raised(), &[]);
}

/// Templates that recurse without end in each of the ways templates recurse.
const RUNAWAYS: &[(&str, &str, &str)] = &[
    ("macro", "{% macro f(n) %}{{ f(n + 1) if n >= 0 else n }}{% endmacro %}{{ f(0) }}", ""),
    (
        "tag",
        "{% macro f(n) %}{% if n >= 0 %}{% set x = f(n + 1) %}{{ x }}{% endif %}{% endmacro %}{{ \
            f(0) }}",
        "",
    ),
    (
        "loop",
        "{% for x in items recursive %}{% if loop.depth > 0 %}{{ loop(items) }}{% endif %}{% endf\
            or %}",
        "{'items': [1]}",
    ),
    ("include", "{% include 'self.html' %}", ""),
    (
        "call block",
        "{% macro f(n) %}{% call f(n + 1) %}x{% endcall %}{{ caller() if n > 0 else '' }}{% endma\
            cro %}{{ f(0) }}",
        "",
    ),
];

fn run_runaway(source: &str, ctx: &str, options: Options) -> gd_jinja::Error {
    let templates = [("self.html", "{% if true %}{% include 'self.html' %}{% endif %}")];
    run(source, ctx, options, &templates).expect_err(source)
}

/// The default stack budget holds the default max_recursion with room to spare, so a
/// template that recurses without end meets the recursion limit first.
#[test]
fn the_recursion_limit_comes_before_the_stack_budget() {
    // A thread of its own, with the 2 MiB stack Rust gives spawned threads.
    let worker = std::thread::Builder::new().stack_size(2 << 20).spawn(|| {
        let want = format!("recursion limit of {DEFAULT_MAX_RECURSION}");
        for (what, source, ctx) in RUNAWAYS {
            for budget in [DEFAULT_STACK_BYTES, MARGIN] {
                let err = run_runaway(
                    source,
                    ctx,
                    Options { stack_bytes: budget, ..Options::default() },
                );
                assert!(err.message.contains(&want), "{what}, budget {budget}: {:?}", err.message);
            }
        }
    });
    assert!(matches!(worker.map(std::thread::JoinHandle::join), Ok(Ok(()))));
}

/// Past the stack budget, recursion that max_recursion allows stops with a Limit error.
#[test]
fn the_stack_budget_stops_deep_recursion() {
    on_big_stack(|| {
        let unbounded =
            || Options { max_recursion: 100_000, stack_bytes: 64 << 10, ..Options::default() };
        for (what, source, ctx) in RUNAWAYS {
            let err = run_runaway(source, ctx, unbounded());
            assert!(
                err.kind == ErrorKind::Limit
                    && err.message.contains("stack budget of 65536 bytes"),
                "{what}: {:?} {:?}",
                err.kind,
                err.message
            );
        }
        let parse = Options { max_nesting: 100_000, stack_bytes: 64 << 10, ..Options::default() };
        let source = format!("{{{{ {} }}}}", wrap("(", "1", ")", 5000));
        expect_limit("parse", &source, "stack budget of 65536 bytes", parse);
        // The default budget is the backstop for recursion the limits allow but whose
        // levels are each deep: here every macro call evaluates a list nested 90 deep.
        let default_budget = format!("stack budget of {DEFAULT_STACK_BYTES} bytes");
        let deep = format!(
            "{{% macro f(n) %}}{{{{ {} }}}}{{% endmacro %}}{{{{ f(0) }}}}",
            wrap("[", "f(n + 1)", "]", 90)
        );
        expect_limit("deep levels", &deep, &default_budget, Options::default());
        let err = run_runaway(
            RUNAWAYS[0].1,
            "",
            Options { max_recursion: 100_000, ..Options::default() },
        );
        assert!(err.message.contains(&default_budget), "macro: {:?}", err.message);
    });
}

#[test]
fn self_containing_values_do_not_recurse_forever() {
    let grow = "{% set l = [] %}{% do l.append(l) %}";
    let out = run(
        &format!("{grow}{{{{ l|length }}}}{{{{ l == l }}}}{{{{ [l] == [l] }}}}"),
        "",
        Options::default(),
        &[],
    );
    assert_eq!(out.unwrap(), "1TrueTrue");
    let out = run(&format!("{grow}{{{{ l }}}}"), "", Options::default(), &[]).unwrap();
    assert!(out.contains("[[[...]]]"), "{out}");
    common::check_error(&format!("{grow}{{{{ l|tojson }}}}"), "contains itself");
    let sorted = format!("{grow}{{{{ [[l], [l]]|sort(case_sensitive=false)|length }}}}");
    match run(&sorted, "", Options::default(), &[]) {
        Ok(out) => assert_eq!(out, "2"),
        Err(err) => assert_eq!(err.kind, ErrorKind::Type),
    }
    let deep = "{% set ns = namespace(l=[]) %}{% for i in range(5000) %}{% set ns.l = [ns.l] %}\
                {% endfor %}{{ ns.l }}";
    let out = run(deep, "", Options::default(), &[]).unwrap();
    assert!(out.contains("[...]"));
}
