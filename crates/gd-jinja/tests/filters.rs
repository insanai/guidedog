//! The built-in filters.

#![allow(clippy::unwrap_used)]

mod common;

use common::{check, check_ctx, check_error, check_error_with, check_opts};
use gd_jinja::{Autoescape, Options};

const USERS: &str = "{'users': [{'n': 'a', 'c': 'x', 'age': 30}, {'n': 'b', 'c': 'y', 'age': 20}, \
                     {'n': 'c', 'c': 'x', 'age': 25}]}";

fn escaping() -> Options {
    Options { autoescape: Autoescape::Always, ..Options::default() }
}

#[test]
fn filters_for_text() {
    check("{{ 'hELLO wORLD'|capitalize }}|{{ 'aB'|lower }}|{{ 'aB'|upper }}", "Hello world|ab|AB");
    check("{{ \"hello world-foo (bar) it's\"|title }}", "Hello World-Foo (Bar) It's");
    check(
        "[{{ 'ab'|center(6) }}] [{{ 'ab'|center(7) }}] [{{ 'abc'|center(2) }}]",
        "[  ab  ] [   ab  ] [abc]",
    );
    check("[{{ '  x  '|trim }}] [{{ 'xxaxx'|trim('x') }}]", "[x] [a]");
    check(
        "{{ 'aaa'|replace('a', 'b') }} {{ 'aaa'|replace('a', 'b', 2) }} {{ 'ab'|replace('', '-') \
            }}",
        "bbb bba -a-b-",
    );
    check("{{ 'abc'|reverse }} {{ [1, 2]|reverse|list }}", "cba [2, 1]");
    check("{{ 'Hello world, foo-bar!'|wordcount }}", "4");
    check(
        "{{ '<p>Hello <b>World</b>\n  &amp; you<!-- x --></p>'|striptags }}",
        "Hello World & you",
    );
    check("{{ 1|string ~ 2 }} {{ [1]|string }}", "12 [1]");
    check("{{ {'a': [1, 'x']}|pprint }}", "{'a': [1, 'x']}");
}

#[test]
fn filter_truncate_and_wordwrap() {
    check(
        "{{ 'foo bar baz qux'|truncate(9) }}|{{ 'foo bar baz qux'|truncate(9, true) }}|\
         {{ 'foo bar baz qux'|truncate(11) }}|{{ 'foo bar baz qux'|truncate(9, end=' >>') }}",
        "foo...|foo ba...|foo bar baz qux|foo >>",
    );
    check("{{ 'foo bar baz qux'|truncate(12, leeway=0) }}", "foo bar...");
    check("{{ 'short'|truncate(3, leeway=0) }}", "...");
    check_error("{{ 'abc'|truncate(2) }}", "expected length >= 3, got 2");
    check(
        "{{ 'The quick brown fox jumps over the lazy dog'|wordwrap(10) }}",
        "The quick\nbrown fox\njumps over\nthe lazy\ndog",
    );
    check(
        "{{ 'abcdefghijkl'|wordwrap(5) }}|{{ 'abcdefghijkl'|wordwrap(5, false) }}",
        "abcde\nfghij\nkl|abcdefghijkl",
    );
    check("{{ 'a b c d'|wordwrap(3, wrapstring='<br>') }}", "a b<br>c d");
    check("{{ 'well-known-thing'|wordwrap(6) }}", "well-\nknown-\nthing");
    check("{{ 'one two\n\nthree four'|wordwrap(7) }}", "one two\n\nthree\nfour");
}

#[test]
fn filter_indent() {
    let ctx = "{'s': 'a\\nb\\n\\nc'}";
    check_ctx("{{ s|indent }}", "a\n    b\n\n    c", ctx);
    check_ctx("{{ s|indent(2, true) }}", "  a\n  b\n\n  c", ctx);
    check_ctx("{{ s|indent(first=true, blank=true) }}", "    a\n    b\n    \n    c", ctx);
    check_ctx("{{ s|indent('> ') }}", "a\n> b\n\n> c", ctx);
    check("{{ 'a\\n'|indent }}|", "a\n|");
}

#[test]
fn filters_for_numbers() {
    check(
        "{{ -3|abs }} {{ -2.5|abs }} {{ '3.5'|float }} {{ 'x'|float }} {{ 3|float }}",
        "3 2.5 3.5 0.0 3.0",
    );
    check("{{ 'x'|float(1.5) }} {{ '1e3'|float }}", "1.5 1000.0");
    check(
        "{{ '42'|int }} {{ '3.7'|int }} {{ 'x'|int }} {{ 'x'|int(5) }} {{ 3.99|int }}",
        "42 3 0 5 3",
    );
    check(
        "{{ '0x1A'|int(0, 16) }} {{ '11'|int(base=2) }} {{ '1_000'|int }} {{ ' 12 '|int }}",
        "26 3 1000 12",
    );
    check("{{ true|int }} {{ none|int }} {{ '-5'|int }}", "1 0 -5");
    check(
        "{{ 42.55|round }} {{ 42.55|round(1, 'floor') }} {{ 2.5|round }} {{ 3.5|round }}",
        "43.0 42.5 2.0 4.0",
    );
    check(
        "{{ 42|round }} {{ 2.675|round(2) }} {{ 1.1|round(0, 'ceil') }} {{ -1.5|round }}",
        "42 2.67 2.0 -2.0",
    );
    check("{{ 1234.5678|round(-2) }} {{ 0.5|round }}", "1200.0 0.0");
    check("{{ [1, 2, 3]|sum }} {{ [1, 2]|sum(start=10) }} {{ [0.5, 1]|sum }}", "6 13 1.5");
    check_ctx("{{ users|sum(attribute='age') }}", "75", USERS);
    check(
        "{{ 1|filesizeformat }} {{ 100|filesizeformat }} {{ 1000|filesizeformat }}",
        "1 Byte 100 Bytes 1.0 kB",
    );
    check(
        "{{ 1500000|filesizeformat }} {{ 1024|filesizeformat(true) }} \
         {{ 3000000000|filesizeformat }} {{ 2048000|filesizeformat(binary=true) }}",
        "1.5 MB 1.0 KiB 3.0 GB 2.0 MiB",
    );
    check_error("{{ 1.5|round(method='x') }}", "method must be common, ceil or floor");
}

#[test]
fn filter_format() {
    check(
        "{{ '%s-%d'|format('a', 3) }} {{ '%(x)s!'|format(x=1) }} {{ '%.2f'|format(3.14159) }}",
        "a-3 1! 3.14",
    );
    check("{{ '%5s|%-5s|'|format('a', 'b') }}{{ '%05d'|format(42) }}", "    a|b    |00042");
    check("{{ '%x %o %e %r %%'|format(255, 8, 1234.5, 'a') }}", "ff 10 1.234500e+03 'a' %");
    check("{{ '%+d % d %i'|format(5, 5, 7.9) }}", "+5  5 7");
    check("{{ '%s and %s' % ('a', 'b') }} {{ '%d%%' % 50 }}", "a and b 50%");
    check("{{ '%g %g %g'|format(0.0001, 1e20, 100) }}", "0.0001 1e+20 100");
    check("{{ '%.3g|%10.4f|%-8.2e|'|format(3.14159, 2.5, 1234.5) }}", "3.14|    2.5000|1.23e+03|");
    check_error("{{ '%s %s'|format(1) }}", "not enough arguments for format string");
    check_error("{{ '%s'|format(1, 2) }}", "not all arguments converted");
    check_error("{{ '%d'|format('x') }}", "%d format: a real number is required, not str");
}

#[test]
fn filters_for_sequences() {
    check(
        "{{ [1, 2, 3, 4, 5]|batch(2)|list }} {{ [1, 2, 3]|batch(2, 'x')|list }}",
        "[[1, 2], [3, 4], [5]] [[1, 2], [3, 'x']]",
    );
    check("{{ [1, 2, 3, 4, 5, 6, 7]|slice(3)|list }}", "[[1, 2, 3], [4, 5], [6, 7]]");
    check("{{ [1, 2, 3, 4, 5, 6, 7]|slice(3, 0)|list }}", "[[1, 2, 3], [4, 5, 0], [6, 7, 0]]");
    check(
        "{{ 'abc'|length }} {{ [1, 2]|count }} {{ {'a': 1}|length }} {{ missing|length }}",
        "3 2 1 0",
    );
    check(
        "{{ [1, 2, 3]|first }} {{ 'abc'|last }} [{{ []|first }}] {{ 'ab'|list }}",
        "1 c [] ['a', 'b']",
    );
    check("{{ {'a': 1}|items|list }} {{ missing|items|list }}", "[('a', 1)] []");
    check_ctx(
        "{{ [1, 2, 3]|join('-') }} {{ users|join(', ', attribute='n') }}",
        "1-2-3 a, b, c",
        USERS,
    );
    check("{{ [1, '2']|map('int')|sum }} {{ ['a', 'b']|map('upper')|join }}", "3 AB");
    check_ctx(
        "{{ users|map(attribute='n')|list }} {{ users|map(attribute='z', default='-')|join }}",
        "['a', 'b', 'c'] ---",
        USERS,
    );
    check("{{ ['x', 'y']|map('replace', 'x', 'z')|join }}", "zy");
    check(
        "{{ [1, 3, 2]|max }} {{ [1, 3, 2]|min }} {{ ['a', 'B']|max }} \
         {{ ['a', 'B']|max(case_sensitive=true) }} [{{ []|max }}]",
        "3 1 B a []",
    );
    check_ctx(
        "{{ (users|max(attribute='age')).n }} {{ (users|min(attribute='age')).n }}",
        "a b",
        USERS,
    );
    check(
        "{{ [1, 2, 1, 3]|unique|list }} {{ ['a', 'A', 'b']|unique|list }} {{ ['a', 'A']|unique(tr\
            ue)|list }}",
        "[1, 2, 3] ['a', 'b'] ['a', 'A']",
    );
    check("{{ [5]|random }} {{ [1, 2, 3]|random in [1, 2, 3] }}", "5 True");
}

#[test]
fn filters_select_reject_sort() {
    check(
        "{{ range(6)|select('odd')|list }} {{ range(6)|reject('odd')|list }}",
        "[1, 3, 5] [0, 2, 4]",
    );
    check(
        "{{ [0, 1, '', none, 'x']|select|list }} {{ [0, 1, '']|reject|list }}",
        "[1, 'x'] [0, '']",
    );
    check(
        "{{ range(10)|select('divisibleby', 3)|list }} {{ range(5)|select('>', 2)|list }}",
        "[0, 3, 6, 9] [3, 4]",
    );
    check("{{ range(5)|select('in', [1, 4])|list }}", "[1, 4]");
    check_ctx(
        "{{ users|selectattr('c', 'equalto', 'x')|map(attribute='n')|join }} \
         {{ users|rejectattr('c', 'eq', 'x')|map(attribute='n')|join }}",
        "ac b",
        USERS,
    );
    check("{{ [{'f': 1}, {'f': 0}, {}]|selectattr('f')|list }}", "[{'f': 1}]");
    check(
        "{{ [3, 1, 2]|sort }} {{ [3, 1, 2]|sort(reverse=true) }} {{ ['b', 'a', 'C']|sort }}",
        "[1, 2, 3] [3, 2, 1] ['a', 'b', 'C']",
    );
    check("{{ ['b', 'a', 'C']|sort(case_sensitive=true) }}", "['C', 'a', 'b']");
    check_ctx(
        "{{ users|sort(attribute='age')|map(attribute='n')|join }} \
         {{ users|sort(attribute='c,age')|map(attribute='n')|join }}",
        "bca cab",
        USERS,
    );
    check("{{ {'b': 1, 'a': 3, 'C': 2}|dictsort }}", "[('a', 3), ('b', 1), ('C', 2)]");
    check("{{ {'b': 1, 'a': 3, 'C': 2}|dictsort(true) }}", "[('C', 2), ('a', 3), ('b', 1)]");
    check("{{ {'b': 1, 'a': 3}|dictsort(by='value', reverse=true) }}", "[('a', 3), ('b', 1)]");
    check_error("{{ [1, 'a']|sort }}", "'<' not supported between instances of 'str' and 'int'");
    check_error("{{ [1]|sort(bogus=1) }}", "sort() got an unexpected keyword argument 'bogus'");
}

#[test]
fn filter_groupby() {
    check_ctx(
        "{% for g in users|groupby('c') %}{{ g.grouper }}:\
         {{ g.list|map(attribute='n')|join(',') }};{% endfor %}",
        "x:a,c;y:b;",
        USERS,
    );
    check_ctx(
        "{% for c, items in users|groupby('c') %}{{ c }}{{ items|length }}{% endfor %}",
        "x2y1",
        USERS,
    );
    check_ctx("{{ (users|groupby('c'))[1] }}", "('y', [{'n': 'b', 'c': 'y', 'age': 20}])", USERS);
    check(
        "{% for g in [{'c': 'NY'}, {'c': 'LA'}, {'c': 'ny'}]|groupby('c') %}\
         {{ g.grouper }}{{ g.list|length }};{% endfor %}",
        "LA1;NY2;",
    );
    check(
        "{% for g in [{'c': 'NY'}, {'c': 'ny'}]|groupby('c', case_sensitive=true) %}\
         {{ g.grouper }};{% endfor %}",
        "NY;ny;",
    );
    check(
        "{% for g in [{'a': {'b': 2}}, {'a': {'b': 1}}]|groupby('a.b') %}{{ g.grouper }}{% endfo\
            r %}",
        "12",
    );
    check(
        "{% for g in [{'c': 1}, {}]|groupby('c', default=0) %}{{ g.grouper }}{% endfor %}",
        "01",
    );
}

#[test]
fn filters_default_and_attr() {
    check(
        "{{ missing|default('d') }} [{{ ''|default('d') }}] {{ ''|default('d', true) }}",
        "d [] d",
    );
    check("{{ none|d('x') }} {{ missing|d }}|{{ 0|d(5, boolean=true) }}", "None |5");
    check("[{{ {'a': 1}|attr('a') }}] {{ ('ab'|attr('upper'))() }}", "[] AB");
    check("{{ {'a': 1}.a|default('z') }} {{ {'a': 1}.b|default('z') }}", "1 z");
}

#[test]
fn filters_for_html() {
    check("{{ '<a href=\"x\">\\'&'|e }}", "&lt;a href=&#34;x&#34;&gt;&#39;&amp;");
    check("{{ '<b>'|escape|escape }}", "&lt;b&gt;");
    check("{{ '<b>'|safe|forceescape }}", "&lt;b&gt;");
    check_opts("{{ '<b>'|safe }}{{ '<i>' }}", "<b>&lt;i&gt;", escaping());
    check(
        "{{ {'class': 'a', 'id': none, 'data-x': '<\"'}|xmlattr }}",
        " class=\"a\" data-x=\"&lt;&#34;\"",
    );
    check("<p{{ {'a': 1}|xmlattr(false) }}>", "<pa=\"1\">");
    check_error("{{ {'a b': 1}|xmlattr }}", "Invalid character in attribute name: 'a b'");
    check(
        "{{ 'a b/c?d'|urlencode }} {{ {'a': 'x y', 'b': '&'}|urlencode }} {{ 'é'|urlencode }}",
        "a%20b/c%3Fd a=x+y&b=%26 %C3%A9",
    );
    check("{{ [('a', 1), ('b', 2)]|urlencode }}", "a=1&b=2");
}

#[test]
fn filter_tojson() {
    check("{{ {'b': 1, 'a': '<x>'}|tojson }}", r#"{"a": "\u003cx\u003e", "b": 1}"#);
    check(
        "{{ [1, 2.5, none, true, 'é', \"'&\"]|tojson }}",
        r#"[1, 2.5, null, true, "\u00e9", "\u0027\u0026"]"#,
    );
    check(
        "{{ {'a': [1, 2], 'b': {}}|tojson(2) }}",
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {}\n}",
    );
    check("{{ 'a\"b\\\\c\\n'|tojson }}", r#""a\"b\\c\n""#);
    check("{{ {1: 'x'}|tojson }} {{ (1, 2)|tojson }}", r#"{"1": "x"} [1, 2]"#);
    check_opts("{{ '<'|tojson }}", r#""\u003c""#, escaping());
    check_error("{{ missing|tojson }}", "Object of type Undefined is not JSON serializable");
}

#[test]
fn filter_urlize() {
    check(
        "{{ 'visit http://example.com now'|urlize }}",
        "visit <a href=\"http://example.com\" rel=\"noopener\">http://example.com</a> now",
    );
    check(
        "{{ 'see www.example.org.'|urlize }}",
        "see <a href=\"https://www.example.org\" rel=\"noopener\">www.example.org</a>.",
    );
    check(
        "{{ 'mail me@example.com'|urlize }}",
        "mail <a href=\"mailto:me@example.com\">me@example.com</a>",
    );
    check(
        "{{ '(http://a.com/x_(y))'|urlize }}",
        "(<a href=\"http://a.com/x_(y)\" rel=\"noopener\">http://a.com/x_(y)</a>)",
    );
    check(
        "{{ 'example.com'|urlize }}",
        "<a href=\"https://example.com\" rel=\"noopener\">example.com</a>",
    );
    check(
        "{{ 'http://example.com/long/path'|urlize(15, true, '_blank') }}",
        "<a href=\"http://example.com/long/path\" rel=\"nofollow noopener\" \
         target=\"_blank\">http://example....</a>",
    );
    check_opts(
        "{{ 'a <b> http://x.io'|urlize }}",
        "a &lt;b&gt; <a href=\"http://x.io\" rel=\"noopener\">http://x.io</a>",
        escaping(),
    );
    check("{{ 'plain words, no links'|urlize }}", "plain words, no links");
    check(
        "{{ 'ftp://f.x'|urlize(extra_schemes=['ftp://']) }}",
        "<a href=\"ftp://f.x\" rel=\"noopener\">ftp://f.x</a>",
    );
    let _ = check_error_with;
}
