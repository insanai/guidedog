//! Autoescaping, undefined modes, error reports, limits, i18n, the host API and loaders.

#![allow(clippy::unwrap_used)]

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use common::{check_error, check_error_with, check_with, run};
use gd_jinja::{Autoescape, Environment, ErrorKind, Loader, Options, UndefinedMode, Value};

fn escaping() -> Options {
    Options { autoescape: Autoescape::Always, ..Options::default() }
}

fn undefined(mode: UndefinedMode) -> Options {
    Options { undefined: mode, ..Options::default() }
}

#[test]
fn autoescape_interactions() {
    let on = || escaping();
    check_with(
        "{{ s }}|{{ s|safe }}|{{ s|e }}|{{ s|e|e }}",
        "&lt;b&gt;|<b>|&lt;b&gt;|&lt;b&gt;",
        "{'s': '<b>'}",
        on(),
        &[],
    );
    let cases = [
        ("{{ '<a>'|safe ~ '<b>' }}", "<a>&lt;b&gt;"),
        ("{{ '<a>'|safe + '<b>' }}", "<a>&lt;b&gt;"),
        ("{{ '<a>' ~ '<b>' }}", "&lt;a&gt;&lt;b&gt;"),
        ("{{ ['<a>', '<b>'|safe]|join(', ') }}", "&lt;a&gt;, <b>"),
        ("{{ ['<a>', '<b>']|join('<br>'|safe) }}", "&lt;a&gt;<br>&lt;b&gt;"),
        ("{% set x %}<i>{% endset %}{{ x }}", "<i>"),
        ("{% macro m(v) %}<p>{{ v }}</p>{% endmacro %}{{ m('<x>') }}", "<p>&lt;x&gt;</p>"),
        (
            "{% macro m() %}<p>{{ caller() }}</p>{% endmacro %}{% call m() %}<i>{{ '&' }}</i>{% e\
                ndcall %}",
            "<p><i>&amp;</i></p>",
        ),
        ("{{ ('<b>%s</b>'|safe)|format('<i>') }}", "<b>&lt;i&gt;</b>"),
        ("{{ '<b>{}</b>'|safe }}{{ ('<b>{}</b>'|safe).format('&') }}", "<b>{}</b><b>&amp;</b>"),
        ("{{ ('<b>'|safe).upper() }}", "<B>"),
        ("{{ ('a'|safe).replace('a', '<') }}", "&lt;"),
        ("{{ 'a<'|replace('<', '>') }}", "a&gt;"),
        ("{% filter upper %}<b>{{ '<i>' }}{% endfilter %}", "<B>&LT;I&GT;"),
        ("{{ [\"<'>\"] }}", "[&#34;&lt;&#39;&gt;&#34;]"),
        ("{% autoescape false %}{{ '<b>'|e }}{{ '<b>' }}{% endautoescape %}", "&lt;b&gt;<b>"),
    ];
    for (source, expected) in cases {
        check_with(source, expected, "", on(), &[]);
    }
    check_with("{{ x }}", "&lt;&amp;&gt;&#34;&#39;", "{'x': '<&>\"\\''}", on(), &[]);
    check_with(
        "{% trans %}<b>{{ x }}</b>{% endtrans %}",
        "<b>&lt;i&gt;</b>",
        "{'x': '<i>'}",
        on(),
        &[],
    );
}

#[test]
fn autoescape_by_extension() {
    let extensions = vec![".html".to_string(), ".xml".to_string()];
    let env = Environment::new(Options {
        autoescape: Autoescape::ForExtensions(extensions),
        ..Options::default()
    });
    env.add_template("a.html", "{{ '<' }}").unwrap();
    env.add_template("a.txt", "{{ '<' }}{% include 'a.html' %}").unwrap();
    let t = env.load_template("a.txt").unwrap();
    assert_eq!(t.render(&Value::default()).unwrap(), "<&lt;");
}

#[test]
fn undefined_modes() {
    let strict = || undefined(UndefinedMode::Strict);
    check_with("{{ x is defined }}{{ x|default('d') }}", "Falsed", "", strict(), &[]);
    for source in [
        "{{ x }}",
        "{% if x %}{% endif %}",
        "{% for i in x %}{% endfor %}",
        "{{ x == 1 }}",
        "{{ x ~ 'a' }}",
        "{{ x|length }}",
    ] {
        check_error_with(source, "'x' is undefined", "", strict(), &[]);
    }
    check_error_with(
        "{{ d.nope }}",
        "'dict object' has no attribute 'nope'",
        "{'d': {}}",
        strict(),
        &[],
    );
    check_with("[{{ d.nope }}]", "[]", "{'d': {}}", Options::default(), &[]);
    check_error("{{ x + 1 }}", "'x' is undefined");
    check_error("{{ x < 1 }}", "'x' is undefined");
    check_error("{{ -x }}", "'x' is undefined");
    check_with(
        "[{{ a.b.c }}][{{ a['b'].c() }}]",
        "[][]",
        "",
        undefined(UndefinedMode::Chainable),
        &[],
    );
    check_with(
        "{{ foo }} {{ d.bar }}",
        "{{ foo }} {{ no such element: dict object['bar'] }}",
        "{'d': {}}",
        undefined(UndefinedMode::Debug),
        &[],
    );
}

#[test]
fn syntax_errors_name_the_line() {
    let at = |source: &str, want: &str, line: usize| {
        let err = check_error(source, want);
        assert_eq!(err.line, line, "{source:?}");
        assert_eq!(err.kind, ErrorKind::Syntax, "{source:?}");
    };
    at("a\n{{ x", "unexpected end of template, expected 'end of print statement'", 2);
    at(
        "{% if x %}\nopen",
        "Unexpected end of template. Jinja was looking for the following tags: 'elif' or 'else' \
         or 'endif'. The innermost block that needs to be closed is 'if'.",
        2,
    );
    at("\n\n{% while x %}", "Encountered unknown tag 'while'.", 3);
    check_error(
        "{% for x in y %}{% endif %}",
        "Encountered unknown tag 'endif'. Jinja was looking for the following tags: 'endfor' or \
         'else'. The innermost block that needs to be closed is 'for'.",
    );
    check_error(
        "{% if a %}{% for x in y %}{% endif %}",
        "You probably made a nesting mistake. Jinja is expecting this tag, but currently looking \
         for 'endfor' or 'else'.",
    );
    let cases = [
        ("{{ 1 + }}", "unexpected 'end of print statement'"),
        ("{{ )", "unexpected ')'"),
        ("{{ a b }}", "expected token 'end of print statement', got 'b'"),
        ("{% for %}", "Expected an expression, got 'end of statement block'"),
        ("{# open", "Missing end of comment tag"),
        ("{{ 'abc }}", "unexpected end of string"),
        ("{{ a | }}", "expected token 'name', got 'end of print statement'"),
        ("{% break %}", "'break' outside of a loop"),
        ("{% macro m(a=1, b) %}{% endmacro %}", "non-default argument follows"),
        ("{% block a %}{% endblock b %}", "mismatching name of block"),
        ("{% raw %}x", "Missing end of raw directive"),
        ("{% call m %}{% endcall %}", "expected call"),
        ("{{ 'a' @ 'b' }}", "unexpected char '@'"),
        (
            "{% trans %}{% if x %}{% endif %}{% endtrans %}",
            "control structures in translatable sections are not allowed",
        ),
    ];
    for (source, want) in cases {
        check_error(source, want);
    }
}

#[test]
fn runtime_errors_name_template_and_line() {
    assert_eq!(check_error("line1\nline2 {{ 1 // 0 }}", "integer division").line, 2);
    assert_eq!(check_error("\n{{ 'x'|nosuch }}", "No filter named 'nosuch'.").line, 2);
    let templates = [("inner.html", "ok\n\n{{ boom.x }}")];
    let err = run("{% include 'inner.html' %}", "", Options::default(), &templates).unwrap_err();
    assert_eq!(err.template, "inner.html");
    assert_eq!(err.line, 3);
    assert_eq!(err.excerpt, "{{ boom.x }}");
    assert_eq!(err.kind, ErrorKind::Undefined);
    let report = err.to_string();
    assert!(report.starts_with("-- UNDEFINED VALUE ---"), "{report}");
    assert!(report.contains("inner.html:3\n\n'boom' is undefined\n"), "{report}");
    assert!(report.contains("\n3| {{ boom.x }}\n"), "{report}");
    assert!(report.contains("Hint: "), "{report}");
    let err =
        run("{{ uper }}", "{'upper': 1}", undefined(UndefinedMode::Strict), &[]).unwrap_err();
    assert_eq!(err.hint, "Did you mean 'upper'?");
    let err = run("{{ 'a'|uppr }}", "", Options::default(), &[]).unwrap_err();
    assert_eq!(err.hint, "Did you mean the filter 'upper'?");
    let err = run("{% endfro %}", "", Options::default(), &[]).unwrap_err();
    assert_eq!(err.hint, "Did you mean {% endfor %}?");
    let err = run("{{ 1 }}\n{% if %}", "", Options::default(), &[]).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Syntax);
    assert_eq!(err.excerpt, "{% if %}");
}

#[test]
fn limits_stop_runaway_templates() {
    let with = |o: Options| o;
    check_error_with(
        "{% macro r() %}{{ r() }}{% endmacro %}{{ r() }}",
        "recursion limit of 20",
        "",
        with(Options { max_recursion: 20, ..Options::default() }),
        &[],
    );
    check_error_with(
        "{% for i in range(100) %}{% endfor %}",
        "iteration limit",
        "",
        Options { max_iterations: 50, ..Options::default() },
        &[],
    );
    check_error_with(
        "{{ range(1000) }}",
        "exceeds the iteration limit",
        "",
        Options { max_iterations: 100, ..Options::default() },
        &[],
    );
    check_error_with(
        "{% for i in range(50) %}xxxxxxxxxx{% endfor %}",
        "output limit of 100",
        "",
        Options { max_output: 100, ..Options::default() },
        &[],
    );
    common::on_big_stack(|| {
        let templates = [("self.html", "{% include 'self.html' %}")];
        check_error_with(
            "{% include 'self.html' %}",
            "recursion limit",
            "",
            Options::default(),
            &templates,
        );
        check_error_with(
            "{% for x in items recursive %}{{ loop(items) }}{% endfor %}",
            "recursion limit",
            "{'items': [1]}",
            Options::default(),
            &[],
        );
    });
}

#[test]
fn max_output_bounds_each_value() {
    let options = Options { max_output: 1000, ..Options::default() };
    let err = run("{{ ('x' * 60000000)|length }}", "", options, &[]).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Limit);
    assert!(err.message.contains("output limit of 1000"), "{}", err.message);
}

#[test]
fn values_too_large_to_count_are_memory_errors() {
    let err =
        run("line one\n{{ 'x' * 9223372036854775807 }}", "", Options::default(), &[]).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Memory);
    assert_eq!(err.line, 2);
    assert!(err.to_string().contains("OUT OF MEMORY"), "{err}");
}

#[test]
fn i18n_trans_tag() {
    let cases = [
        ("{% trans %}Hello {{ user }}!{% endtrans %}", "Hello Ann!", "{'user': 'Ann'}"),
        ("{% trans name=user|upper %}Hi {{ name }}{% endtrans %}", "Hi ANN", "{'user': 'Ann'}"),
        (
            "{% trans count=n %}{{ count }} item{% pluralize %}{{ count }} items{% endtrans %}",
            "1 item",
            "{'n': 1}",
        ),
        (
            "{% trans count=n %}{{ count }} item{% pluralize %}{{ count }} items{% endtrans %}",
            "3 items",
            "{'n': 3}",
        ),
        ("{% trans %}100% sure{% endtrans %}", "100% sure", ""),
        ("{% trans x=1 %}100% {{ x }}{% endtrans %}", "100% 1", ""),
        ("{% trans trimmed %}\n  Hello\n    world  {% endtrans %}", "Hello world", ""),
        ("{% trans %}{{ a }}{% pluralize %}{{ a }}s{% endtrans %}", "2s", "{'a': 2}"),
    ];
    for (source, expected, ctx) in cases {
        check_with(source, expected, ctx, Options::default(), &[]);
    }
}

#[test]
fn host_api_values_filters_tests_globals() {
    let mut env = Environment::new(escaping());
    env.add_filter("shout", |c| {
        let times = c.arg_count(1, "times", 1)?;
        Ok(Value::from(gd_jinja::to_str(&c.args[0]).to_uppercase().repeat(times)))
    });
    env.add_test("long", |c| Ok(Value::from(c.args[0].len() > 3)));
    env.add_global("site", Value::from("Guidedog"));
    env.add_function("greet", |c| match c.args.first() {
        Some(name) => Ok(Value::markup(&format!("<b>{name}</b>"))),
        None => Err(c.error("greet needs a name")),
    });
    let ctx = Value::dict();
    ctx.set("name", Value::from("<ann>"));
    ctx.set("n", Value::from(3));
    ctx.set("f", Value::from(1.5));
    ctx.set("ok", Value::from(true));
    ctx.set("items", Value::list([Value::from(1), Value::from("x"), Value::None]));
    ctx.set("html", Value::markup("<br>"));
    ctx.set("pair", Value::tuple([Value::from(1), Value::from(2)]));
    let t = env
        .parse_template(
            "page.html",
            "{{ name|shout(2) }} {{ 'abcd' is long }} {{ site }} {{ greet('x') }} {{ n }} {{ f }} \
             {{ ok }} {{ items }} {{ html }} {{ pair }}",
        )
        .unwrap();
    assert_eq!(
        t.render(&ctx).unwrap(),
        "&lt;ANN&gt;&lt;ANN&gt; True Guidedog <b>x</b> 3 1.5 True [1, &#39;x&#39;, None] <br> (1\
            , 2)"
    );
    let bad = env.parse_template("bad.html", "{{ greet() }}").unwrap();
    let err = bad.render(&ctx).unwrap_err();
    assert_eq!(err.message, "greet needs a name");
    assert_eq!(err.template, "bad.html");
    assert_eq!(ctx.get("n"), Value::from(3));
    assert_eq!(ctx.get("items").len(), 3);
    assert_eq!(env.evaluate("n * 2 + items|length", &ctx).unwrap(), Value::from(9));
}

#[test]
fn memory_loader_caches_until_cleared() {
    let sources = Rc::new(RefCell::new(HashMap::from([
        ("base.html".to_string(), "B[{% block x %}{% endblock %}]".to_string()),
        (
            "page.html".to_string(),
            "{% extends 'base.html' %}{% block x %}{{ v }}{% endblock %}".to_string(),
        ),
    ])));
    let shared = sources.clone();
    let loader = Loader::custom(move |name| shared.borrow().get(name).cloned());
    let env = Environment::new(Options { loader: Some(loader), ..Options::default() });
    let ctx = Value::dict();
    ctx.set("v", Value::from(7));
    assert_eq!(env.load_template("page.html").unwrap().render(&ctx).unwrap(), "B[7]");
    sources.borrow_mut().insert("base.html".to_string(), "changed".to_string());
    assert_eq!(env.load_template("page.html").unwrap().render(&ctx).unwrap(), "B[7]");
    env.clear_cache();
    assert_eq!(env.load_template("page.html").unwrap().render(&ctx).unwrap(), "changed");
    assert_eq!(env.load_template("nope.html").unwrap_err().kind, ErrorKind::NotFound);
    let memory = Loader::memory([("a.html", "A")]);
    let env = Environment::new(Options { loader: Some(memory), ..Options::default() });
    assert_eq!(env.load_template("a.html").unwrap().render(&Value::default()).unwrap(), "A");
}

#[test]
fn filesystem_loader_searches_in_order_and_refuses_climbing_out() {
    let root = std::env::temp_dir().join(format!("gd-jinja-loader-{}", std::process::id()));
    let (first, second) = (root.join("first"), root.join("second"));
    std::fs::create_dir_all(first.join("sub")).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let files = [
        (first.join("a.html"), "first-a"),
        (second.join("a.html"), "second-a"),
        (second.join("b.html"), "b+{% include 'sub/c.html' %}"),
        (first.join("sub/c.html"), "c"),
        (root.join("secret.html"), "secret"),
    ];
    for (path, text) in &files {
        std::fs::write(path, text).unwrap();
    }
    let env = Environment::new(Options {
        loader: Some(Loader::filesystem([first, second])),
        ..Options::default()
    });
    for (name, want) in
        [("a.html", "first-a"), ("b.html", "b+c"), ("./sub/c.html", "c"), ("/sub/c.html", "c")]
    {
        let t = env.load_template(name).unwrap_or_else(|err| panic!("{name}: {err}"));
        assert_eq!(t.render(&Value::default()).unwrap(), want);
    }
    for name in ["../secret.html", "sub/../../secret.html", "missing.html"] {
        assert_eq!(env.load_template(name).unwrap_err().kind, ErrorKind::NotFound, "{name}");
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_failed_render_leaves_the_environment_usable() {
    let env = Environment::new(escaping());
    env.add_template("m.html", "{% macro m(x) %}<{{ x }}>{% endmacro %}").unwrap();
    let t = env
        .parse_template(
            "t.html",
            "{% from 'm.html' import m %}{% for i in range(3) %}{{ m(i) }}{{ [i]|tojson }}{% endf\
                or %}{{ 1 // 0 }}",
        )
        .unwrap();
    assert_eq!(t.render(&Value::default()).unwrap_err().kind, ErrorKind::Runtime);
    let t = env.parse_template("t2.html", "{% for i in range(3) %}{{ i }}{% endfor %}").unwrap();
    assert_eq!(t.render(&Value::default()).unwrap(), "012");
}

#[test]
fn the_one_call_path_renders() {
    let ctx = Value::dict();
    ctx.set("name", Value::from("World"));
    assert_eq!(gd_jinja::render_str("Hello {{ name }}!", &ctx).unwrap(), "Hello World!");
}
