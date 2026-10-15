//! Built-in tests and globals, inheritance, include and import.

#![allow(clippy::unwrap_used)]

mod common;

use common::{check, check_ctx, check_error, check_error_with, check_with, run};
use gd_jinja::Options;

#[test]
fn builtin_tests() {
    check_ctx(
        "{{ x is defined }} {{ y is defined }} {{ y is undefined }} {{ none is none }}",
        "True False True True",
        "{'x': 1}",
    );
    let cases = [
        (
            "{{ 3 is odd }} {{ 4 is even }} {{ 9 is divisibleby 3 }} {{ 9 is divisibleby(2) }}",
            "True True True False",
        ),
        (
            "{{ 1 is number }} {{ 1.5 is float }} {{ 1 is integer }} {{ true is integer }}",
            "True True True False",
        ),
        (
            "{{ true is boolean }} {{ 1 is boolean }} {{ true is true }} {{ 1 is true }}",
            "True False True False",
        ),
        (
            "{{ false is false }} {{ 0 is false }} {{ 'a' is string }} {{ 1 is string }}",
            "True False True False",
        ),
        (
            "{{ {} is mapping }} {{ [] is mapping }} {{ [] is sequence }} {{ 'x' is sequence }}",
            "True False True True",
        ),
        (
            "{{ [] is iterable }} {{ 3 is iterable }} {{ range is callable }} {{ 1 is callable }}",
            "True False True False",
        ),
        (
            "{{ 'abc' is lower }} {{ 'aBc' is lower }} {{ 'ABC' is upper }} {{ '1' is upper }}",
            "True False True False",
        ),
        (
            "{{ 1 is eq 1 }} {{ 1 is equalto 2 }} {{ 1 is ne 2 }} {{ 1 is lt 2 }} {{ 2 is le 2 }}",
            "True False True True True",
        ),
        (
            "{{ 3 is gt 2 }} {{ 3 is greaterthan 4 }} {{ 3 is ge 3 }} {{ 1 is lessthan 0 }}",
            "True False True False",
        ),
        (
            "{{ 2 is in [1, 2] }} {{ 'x' is in 'abc' }} {{ 'upper' is filter }} {{ 'odd' is test \
                }} {{ 'nope' is filter }}",
            "True False True True False",
        ),
        ("{{ '<b>'|safe is escaped }} {{ '<b>' is escaped }}", "True False"),
        (
            "{% set a = [1] %}{% set b = a %}{{ a is sameas b }} {{ a is sameas [1] }} \
             {{ none is sameas none }} {{ 1 is sameas true }}",
            "True False True False",
        ),
        ("{{ x is not defined }} {{ 3 is not even }} {{ not 3 is even }}", "True True True"),
        ("{{ 1 if x is defined else 2 }}", "2"),
    ];
    for (source, expected) in cases {
        check(source, expected);
    }
    check_error("{{ 1 is nosuchtest }}", "No test named 'nosuchtest'.");
}

#[test]
fn builtin_globals() {
    check(
        "{{ range(3)|list }} {{ range(2, 5)|list }} {{ range(10, 0, -3)|list }} {{ range(0)|list \
            }}",
        "[0, 1, 2] [2, 3, 4] [10, 7, 4, 1] []",
    );
    check(
        "{{ dict(a=1) }} {{ dict([('b', 2)]) }} {{ dict({'c': 3}, d=4) }}",
        "{'a': 1} {'b': 2} {'c': 3, 'd': 4}",
    );
    check(
        "{% set c = cycler('a', 'b') %}{{ c.next() }}{{ c.next() }}{{ c.next() }}\
         {{ c.current }}{% do c.reset() %}{{ c.next() }}",
        "ababa",
    );
    check("{% set j = joiner('|') %}{% for x in 'abc' %}{{ j() }}{{ x }}{% endfor %}", "a|b|c");
    check("{% set j = joiner() %}{{ j() }}{{ j() }}{{ j() }}", ", , ");
    check("{{ namespace(a=1).a }}", "1");
    check(
        "{{ gettext('Hello') }} {{ _('Hi') }} {{ ngettext('%(num)d apple', '%(num)d apples', 3) }\
            }",
        "Hello Hi %(num)d apples",
    );
    check_error("{{ range(1, 2, 0) }}", "range() arg 3 must not be zero");
    check_error("{{ range('a') }}", "'str' object cannot be interpreted as an integer");
}

#[test]
fn lipsum_is_deterministic() {
    let render = |source: &str, options: Options| run(source, "", options, &[]).unwrap();
    let a = render("{{ lipsum(2) }}", Options::default());
    assert_eq!(a, render("{{ lipsum(2) }}", Options::default()));
    assert!(a.starts_with("<p>") && a.matches("<p>").count() == 2, "{a}");
    let c = render("{{ lipsum(1, false, 5, 5) }}", Options::default());
    assert_eq!(c.split_whitespace().count(), 5);
    assert!(c.ends_with('.'));
    assert_ne!(a, render("{{ lipsum(2) }}", Options { random_seed: 7, ..Options::default() }));
}

const BASES: &[(&str, &str)] = &[
    (
        "base.html",
        "<title>{% block title %}Base{% endblock %}</title>\
         <body>{% block body %}{% block inner %}I{% endblock %}{% endblock body %}</body>",
    ),
    (
        "mid.html",
        "{% extends 'base.html' %}{% block title %}Mid-{{ super() }}{% endblock %}{% block inner \
            %}M{% endblock %}",
    ),
    ("req.html", "{% block needed required %}{# note #}  {% endblock %}"),
    ("self.html", "{% block a %}A{% endblock %}|{{ self.a() }}"),
    (
        "scoped.html",
        "{% for i in [1, 2] %}{% block item scoped %}{{ i }}{% endblock %}{% endfor %}",
    ),
    ("unscoped.html", "{% for i in [1, 2] %}{% block item %}[{{ i }}]{% endblock %}{% endfor %}"),
    ("uses_var.html", "{% block b %}{{ title }}{% endblock %}"),
];

#[test]
fn inheritance() {
    let ok =
        |source: &str, expected: &str| check_with(source, expected, "", Options::default(), BASES);
    let base = "<title>Base</title><body>I</body>";
    ok("{% extends 'base.html' %}", base);
    ok(
        "{% extends 'mid.html' %}{% block title %}Top-{{ super() }}{% endblock %}",
        "<title>Top-Mid-Base</title><body>M</body>",
    );
    ok(
        "{% extends 'mid.html' %}{% block body %}B{{ super() }}{% endblock %}",
        "<title>Mid-Base</title><body>BM</body>",
    );
    ok(
        "{% extends 'base.html' %}ignored{% block title %}T{% endblock %}ignored",
        "<title>T</title><body>I</body>",
    );
    ok("{% extends 'req.html' %}{% block needed %}ok{% endblock %}", "ok");
    ok("{% extends 'self.html' %}{% block a %}B{% endblock %}", "B|B");
    ok("{% extends 'scoped.html' %}{% block item %}<{{ i }}>{% endblock %}", "<1><2>");
    ok("{% extends 'unscoped.html' %}", "[][]");
    ok("{% extends 'uses_var.html' %}{% set title = 'T' %}", "T");
    check_with("{% extends name %}", base, "{'name': 'base.html'}", Options::default(), BASES);
    ok("{% if false %}{% extends 'mid.html' %}{% endif %}x", "x");
    ok("{% extends ['nope.html', 'base.html'] %}", base);
    let fails =
        |source: &str, want: &str| check_error_with(source, want, "", Options::default(), BASES);
    fails("{% extends 'req.html' %}", "Required block 'needed' not found");
    fails("{% extends 'nope.html' %}", "template 'nope.html' not found");
    fails("{% extends 'base.html' %}{% extends 'base.html' %}", "extended multiple times");
    check_error("{% block a %}{{ super() }}{% endblock %}", "no parent block called 'a'");
    check_error(
        "{% block a %}{% endblock %}{% block a %}{% endblock %}",
        "block 'a' defined twice",
    );
}

const PARTS: &[(&str, &str)] = &[
    ("part.html", "[{{ name }}]"),
    ("loopvar.html", "<{{ i }}>"),
    ("setter.html", "{% set leaked = 1 %}"),
    (
        "macros.html",
        "{% macro hi(n) %}Hi {{ n }}{% endmacro %}{% set version = 2 %}\
         {% set _private = 3 %}{% macro uses_ctx() %}{{ name }}{% endmacro %}",
    ),
    ("child.html", "{% extends 'layout.html' %}{% block c %}child{% endblock %}"),
    ("layout.html", "L({% block c %}{% endblock %})"),
];

#[test]
fn include_and_import() {
    let ok = |source: &str, expected: &str, ctx: &str| {
        check_with(source, expected, ctx, Options::default(), PARTS)
    };
    ok("{% include 'part.html' %}", "[x]", "{'name': 'x'}");
    ok("{% include 'part.html' without context %}", "[]", "{'name': 'x'}");
    ok("{% for i in [1, 2] %}{% include 'loopvar.html' %}{% endfor %}", "<1><2>", "");
    ok("{% include 'setter.html' %}[{{ leaked }}]", "[]", "");
    ok("{% include 'missing.html' ignore missing %}ok", "ok", "");
    ok("{% include ['missing.html', 'part.html'] %}", "[y]", "{'name': 'y'}");
    ok("{% include 'child.html' %}", "L(child)", "");
    check_error_with(
        "{% include 'missing.html' %}",
        "template 'missing.html' not found",
        "",
        Options::default(),
        PARTS,
    );
    check_error(
        "{% include ['a.html', 'b.html'] %}",
        "none of the templates given were found: a.html, b.html",
    );
    ok(
        "{% import 'macros.html' as m %}{{ m.hi('Ann') }} {{ m.version }} [{{ m._private }}]",
        "Hi Ann 2 []",
        "",
    );
    ok("{% from 'macros.html' import hi, version as v %}{{ hi('Bo') }}{{ v }}", "Hi Bo2", "");
    ok("{% from 'macros.html' import uses_ctx %}[{{ uses_ctx() }}]", "[]", "{'name': 'n'}");
    ok(
        "{% from 'macros.html' import uses_ctx with context %}[{{ uses_ctx() }}]",
        "[n]",
        "{'name': 'n'}",
    );
    ok("{% import 'macros.html' as m with context %}{{ m.uses_ctx() }}", "n", "{'name': 'n'}");
    let fails =
        |source: &str, want: &str| check_error_with(source, want, "", Options::default(), PARTS);
    fails(
        "{% from 'macros.html' import nothing %}{{ nothing() }}",
        "does not export the requested name 'nothing'",
    );
    fails(
        "{% from 'macros.html' import _private %}",
        "names starting with an underline can not be imported",
    );
    ok("{% import 'macros.html' as m %}{{ m }}", "<TemplateModule 'macros.html'>", "");
}

#[test]
fn inheritance_edge_cases() {
    let templates = [
        ("base.html", "[{% block a %}base-a{% endblock %}|{% block b %}base-b{% endblock %}]"),
        ("m.html", "{% macro hi() %}hi{% endmacro %}"),
        ("part.html", "PART"),
        ("l1.html", "{% extends 'base.html' %}{% block a %}l1({{ super() }}){% endblock %}"),
        (
            "l2.html",
            "{% extends 'l1.html' %}{% block a %}l2({{ super() }}){% endblock %}{% block b %}{{ s\
                elf.a() }}{% endblock %}",
        ),
    ];
    let ok = |source: &str, expected: &str| {
        check_with(source, expected, "", Options::default(), &templates)
    };
    ok("{% extends 'base.html' %}{% include 'part.html' %}", "[base-a|base-b]");
    ok("before{% extends 'base.html' %}", "before[base-a|base-b]");
    ok(
        "{% extends 'base.html' %}{% import 'm.html' as m %}{% block a %}{{ m.hi() }}{% endblock \
            %}",
        "[hi|base-b]",
    );
    ok(
        "{% extends 'base.html' %}{% macro x() %}X{% endmacro %}{% block b %}{{ x() }}{% endbloc\
            k %}",
        "[base-a|X]",
    );
    ok(
        "{% extends 'l2.html' %}{% block a %}l3({{ super() }}){% endblock %}",
        "[l3(l2(l1(base-a)))|l3(l2(l1(base-a)))]",
    );
    ok(
        "{% extends 'base.html' %}{% set x %}{% include 'part.html' %}{% endset %}{% block a %}{\
            { x }}{% endblock %}",
        "[PART|base-b]",
    );
    ok(
        "{% extends 'base.html' %}{% block b %}{% block inner %}in{% endblock %}{% endblock %}",
        "[base-a|in]",
    );
}

#[test]
fn macros_imported_keep_their_template() {
    let templates = [
        (
            "forms.html",
            "{% macro input(name, value='', type='text') -%}\n\
             <input type=\"{{ type }}\" name=\"{{ name }}\" value=\"{{ value|e }}\">\n\
             {%- endmacro %}{% macro field(label) %}<label>{{ label }}{{ caller() }}</label>\
             {% endmacro %}",
        ),
        ("err.html", "{% macro bad() %}\n\n{{ 1 // 0 }}{% endmacro %}"),
    ];
    check_with(
        "{% import 'forms.html' as f %}{{ f.input('q', '<x>') }}",
        "<input type=\"text\" name=\"q\" value=\"&lt;x&gt;\">",
        "",
        Options::default(),
        &templates,
    );
    check_with(
        "{% from 'forms.html' import field, input %}{% call field('L') %}{{ input('n', type='pass\
            word') }}{% endcall %}",
        "<label>L<input type=\"password\" name=\"n\" value=\"\"></label>",
        "",
        Options::default(),
        &templates,
    );
    let err =
        run("{% from 'err.html' import bad %}\n{{ bad() }}", "", Options::default(), &templates)
            .unwrap_err();
    assert_eq!(err.template, "err.html");
    assert_eq!(err.line, 3);
}
