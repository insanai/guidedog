//! Tags: if, for, set, with, filter, do, raw, autoescape, macros, call blocks and
//! whitespace control.

#![allow(clippy::unwrap_used)]

mod common;

use common::{check, check_ctx, check_error, check_opts};
use gd_jinja::{Autoescape, Options};

fn escaping() -> Options {
    Options { autoescape: Autoescape::Always, ..Options::default() }
}

#[test]
fn if_elif_else() {
    let src = "{% if n > 2 %}big{% elif n == 2 %}two{% elif n %}small{% else %}zero{% endif %}";
    check_ctx(src, "big", "{'n': 5}");
    check_ctx(src, "two", "{'n': 2}");
    check_ctx(src, "small", "{'n': 1}");
    check_ctx(src, "zero", "{'n': 0}");
    check("{% if missing %}a{% else %}b{% endif %}", "b");
    check("{% if [] %}a{% endif %}{% if {} %}b{% endif %}{% if 'x' %}c{% endif %}", "c");
}

#[test]
fn for_loops_and_loop_object() {
    check(
        "{% for x in 'abc' %}{{ loop.index }}{{ loop.index0 }}{{ loop.revindex }}\
         {{ loop.revindex0 }}{{ loop.first }}{{ loop.last }}{{ loop.length }};{% endfor %}",
        "1032TrueFalse3;2121FalseFalse3;3210FalseTrue3;",
    );
    check("{% for x in [1, 2, 3] %}{{ loop.cycle('odd', 'even') }} {% endfor %}", "odd even odd ");
    check(
        "{% for x in [1, 2, 3] %}{{ loop.previtem }}-{{ loop.nextitem }} {% endfor %}",
        "-2 1-3 2- ",
    );
    check(
        "{% for x in [1, 1, 2, 3, 3] %}{% if loop.changed(x) %}{{ x }}{% endif %}{% endfor %}",
        "123",
    );
    check("{% for k, v in {'a': 1, 'b': 2}.items() %}{{ k }}={{ v }};{% endfor %}", "a=1;b=2;");
    check("{% for k in {'a': 1, 'b': 2} %}{{ k }}{% endfor %}", "ab");
    check("{% for a, b in [(1, 2), (3, 4)] %}{{ a + b }},{% endfor %}", "3,7,");
    check("{% for x in [] %}x{% else %}empty{% endfor %}", "empty");
    check("{% for x in missing %}x{% else %}none{% endfor %}", "none");
    check(
        "{% for x in range(10) if x is odd %}{{ x }}{{ loop.index }}{{ loop.length }},{% endfor %\
            }",
        "115,325,535,745,955,",
    );
    check("{% for x in [1, 2] if x > 5 %}x{% else %}no{% endfor %}", "no");
    check("{% set x = 'outer' %}{% for x in [1] %}{{ x }}{% endfor %}{{ x }}", "1outer");
    check(
        "{% for i in range(2) %}{% for j in range(2) %}{{ loop.depth }}{{ i }}{{ j }} \
         {% endfor %}{% endfor %}",
        "100 101 110 111 ",
    );
    check(
        "{% for x in [1, 2, 3, 4] %}{% if x == 3 %}{% break %}{% endif %}{{ x }}{% endfor %}",
        "12",
    );
    check(
        "{% for x in [1, 2, 3, 4] %}{% if x is even %}{% continue %}{% endif %}{{ x }}{% endfor %\
            }",
        "13",
    );
    check("{% for i in range(3) %}{% set x = i * 2 %}{{ x }}{% endfor %}[{{ x }}]", "024[]");
    check(
        "{% for x in [1, 2] %}{% for y in 'ab' %}{{ loop.index }}{% endfor %}{{ loop.index }}{% e\
            ndfor %}",
        "121122",
    );
    check("{{ loop }}{% for x in [1] %}{{ loop }}{% endfor %}", "<LoopContext 1/1>");
    check_error("{% for x in 42 %}{% endfor %}", "'int' object is not iterable");
    check_error("{% for a, b in [1] %}{% endfor %}", "cannot unpack");
}

#[test]
fn recursive_loops() {
    let tree = "{'items': [{'n': 'a', 'c': [{'n': 'b', 'c': []}, {'n': 'c', 'c': \
                [{'n': 'd', 'c': []}]}]}, {'n': 'e', 'c': []}]}";
    check_ctx(
        "{% for i in items recursive %}<{{ i.n }}{{ loop.depth }}\
         {% if i.c %}[{{ loop(i.c) }}]{% endif %}>{% endfor %}",
        "<a1[<b2><c2[<d3>]>]><e1>",
        tree,
    );
    check_ctx(
        "{% for i in items recursive %}{{ loop.depth0 }}{{ loop(i.c) }}{% endfor %}",
        "01120",
        tree,
    );
    check_error("{% for x in [1] %}{{ loop([]) }}{% endfor %}", "non recursive loop");
}

#[test]
fn set_and_namespace() {
    check("{% set a = 1 %}{% set b, c = 2, 3 %}{{ a }}{{ b }}{{ c }}", "123");
    check("{% set a, b = 'xy' %}{{ b }}{{ a }}", "yx");
    check("{% set x %}Hello {{ 'World' }}{% endset %}[{{ x }}]", "[Hello World]");
    check("{% set x | upper | replace('L', 'l') %}hello{% endset %}{{ x }}", "HEllO");
    check(
        "{% set ns = namespace(found=false, n=0) %}{% for i in [1, 2, 3] %}\
         {% set ns.n = ns.n + i %}{% if i == 2 %}{% set ns.found = true %}{% endif %}\
         {% endfor %}{{ ns.found }} {{ ns.n }}",
        "True 6",
    );
    check("{% set ns = namespace({'a': 1}, b=2) %}{{ ns.a }}{{ ns.b }}{{ ns.c }}", "12");
    check("{% set ns = namespace() %}{% set ns.x = 5 %}{{ ns }}", "<Namespace {'x': 5}>");
    check(
        "{% set total = 0 %}{% for i in [1, 2] %}{% set total = total + i %}{{ total }}{% endfor \
            %}{{ total }}",
        "130",
    );
    check_error("{% set d = {} %}{% set d.x = 1 %}", "non-namespace object");
    check_error("{% set a, b = [1, 2, 3] %}", "too many values to unpack (expected 2)");
    check_error("{% set true = 1 %}", "can't assign to 'Const'");
}

#[test]
fn with_filter_do_raw_autoescape() {
    check("{% with a = 1, b = 2 %}{{ a + b }}{% endwith %}[{{ a }}]", "3[]");
    check("{% set a = 5 %}{% with a = 1, b = a %}{{ b }}{% endwith %}", "5");
    check("{% with %}{% set x = 1 %}{{ x }}{% endwith %}[{{ x }}]", "1[]");
    check("{% filter upper %}hello {{ 'x' }}{% endfilter %}", "HELLO X");
    check("{% filter trim | center(7) %}  ab  {% endfilter %}|", "   ab  |");
    check("{% set l = [] %}{% do l.append(1) %}{{ l }}", "[1]");
    check("{% raw %}{{ not }} {% if %}{% endraw %}", "{{ not }} {% if %}");
    check("{%- raw -%}  a  {%- endraw -%}", "a");
    check("{% autoescape true %}{{ '<b>' }}{% endautoescape %}{{ '<b>' }}", "&lt;b&gt;<b>");
    check_opts(
        "{% autoescape false %}{{ '<b>' }}{% endautoescape %}{{ '<b>' }}",
        "<b>&lt;b&gt;",
        escaping(),
    );
    check("{# a comment #}x{# multi\nline #}y", "xy");
}

#[test]
fn macros_and_call_blocks() {
    check(
        "{% macro hi(name, greeting='Hello') %}{{ greeting }}, {{ name }}!{% endmacro %}\
         {{ hi('Ann') }} {{ hi('Bob', greeting='Hey') }}",
        "Hello, Ann! Hey, Bob!",
    );
    check("{% macro m(a, b=a * 2) %}{{ a }}{{ b }}{% endmacro %}{{ m(3) }}", "36");
    check(
        "{% macro v() %}{{ varargs }}{{ kwargs }}{% endmacro %}{{ v(1, 2, k=3) }}",
        "(1, 2){'k': 3}",
    );
    check("{% macro m(x) %}[{{ x }}]{% endmacro %}{{ m() }}", "[]");
    check(
        "{% macro list(items) %}<ul>{% for i in items %}<li>{{ caller(i) }}</li>\
         {% endfor %}</ul>{% endmacro %}{% call(item) list([1, 2]) %}#{{ item }}{% endcall %}",
        "<ul><li>#1</li><li>#2</li></ul>",
    );
    check(
        "{% macro box() %}<div>{{ caller() }}</div>{% endmacro %}{% call box() %}inside{% endcal\
            l %}",
        "<div>inside</div>",
    );
    check(
        "{% macro m(a, b) %}{% endmacro %}{{ m.name }} {{ m.arguments }} \
         {{ m.catch_varargs }} {{ m.caller }}",
        "m ('a', 'b') False False",
    );
    check(
        "{% macro fact(n) %}{% if n <= 1 %}1{% else %}{{ n * fact(n - 1) | int }}\
         {% endif %}{% endmacro %}{{ fact(5) }}",
        "120",
    );
    check_opts("{% macro m() %}<b>{% endmacro %}{{ m() }}", "<b>", escaping());
    check("{% set x = 'outer' %}{% macro m() %}{{ x }}{% endmacro %}{{ m() }}", "outer");
    check("{{ m }}{% macro m() %}{% endmacro %}{{ m }}", "<Macro 'm'>");
    check_error(
        "{% macro m(a) %}{% endmacro %}{{ m(1, 2) }}",
        "macro 'm' takes not more than 1 argument(s)",
    );
    check_error(
        "{% macro m() %}{% endmacro %}{{ m(z=1) }}",
        "macro 'm' takes no keyword argument 'z'",
    );
    check_error(
        "{% macro m() %}{% endmacro %}{% call m() %}x{% endcall %}",
        "macro 'm' takes no keyword argument 'caller'",
    );
    check_error(
        "{% macro m(a) %}{% endmacro %}{{ m(1, a=2) }}",
        "macro 'm' got multiple values for argument 'a'",
    );
}

#[test]
fn whitespace_control() {
    let trim = Options { trim_blocks: true, ..Options::default() };
    let both = Options { trim_blocks: true, lstrip_blocks: true, ..Options::default() };
    let lstrip = Options { lstrip_blocks: true, ..Options::default() };
    check("a  {{- 'b' -}}  c", "abc");
    check("a\n  {%- if true -%}\n  b\n  {%- endif -%}\n  c", "abc");
    check("{% for x in [1, 2] %}\n{{ x }}\n{% endfor %}\n", "\n1\n\n2\n");
    check_opts("{% for x in [1, 2] %}\n{{ x }}\n{% endfor %}\n", "1\n2\n", trim.clone());
    check_opts(
        "<div>\n    {% if true %}\n    yes\n    {% endif %}\n</div>",
        "<div>\n    yes\n</div>",
        both.clone(),
    );
    check_opts("    {%+ if true %}x{% endif %}", "    x", lstrip);
    check_opts("{% if true +%}\nx{% endif %}", "\nx", trim);
    check_opts("  {# c #}\nx", "x", both.clone());
    check_opts("  {{ 'v' }}\n", "  v", both);
    check("a\n", "a");
    check_opts("a\n", "a\n", Options { keep_trailing_newline: true, ..Options::default() });
    check("a\r\nb", "a\nb");
    check("x {#- c -#} y", "xy");
}
