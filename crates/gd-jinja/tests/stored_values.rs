//! Values templates store in the host's lists and dicts stay the host's, shared as in
//! Jinja; what belongs to a render cannot be called after it.

#![allow(clippy::unwrap_used)]

use gd_jinja::{Environment, ErrorKind, Value};

#[test]
fn stored_values_outlive_the_render() {
    let items = Value::list([Value::from("a")]);
    let empty = Value::dict();
    let ctx = Value::dict();
    ctx.set("items", items.clone());
    ctx.set("d", empty.clone());
    {
        let env = Environment::default();
        let t = env
            .parse_template(
                "page.html",
                "{% set n = 1 %}{% set late = [] %}\
                 {% do items.append('b' ~ n) %}{% do items.extend(['c', ['x' ~ n]]) %}\
                 {% do items.append(late) %}{% do late.append('after' ~ n) %}\
                 {% do items.insert(1, 'lit') %}{% do d.update(k='v' ~ n) %}\
                 {% do d.setdefault('lit', ('t', 'u')) %}{% set ns = namespace(a=1) %}\
                 {% do d.update(ns=ns) %}{% set ns.a = 'two' * 2 %}{% do items.append(range) %}\
                 {% for x in [1] %}{% do items.append(loop) %}{% endfor %}",
            )
            .unwrap();
        assert_eq!(t.render(&ctx).unwrap(), "");
    }
    assert_eq!(
        items.repr(),
        "['a', 'lit', 'b1', 'c', ['x1'], ['after1'], <built-in function range>, <LoopContext 1/1>]"
    );
    assert_eq!(empty.repr(), "{'k': 'v1', 'lit': ('t', 'u'), 'ns': <Namespace {'a': 'twotwo'}>}");
    assert_eq!(empty.get("ns").get("a").to_string(), "twotwo");
    // A second render, in another environment, sees the values the first one stored.
    let again = Environment::default()
        .render_str("{{ items[2] }} {{ items[5][0] }} {{ d.ns.a }} {{ items[6](2)|list }}", &ctx)
        .unwrap();
    assert_eq!(again, "b1 after1 twotwo [0, 1]");
}

#[test]
fn stored_macros_cannot_be_called_after_their_render() {
    let items = Value::list([]);
    let ctx = Value::dict();
    ctx.set("items", items.clone());
    let env = Environment::default();
    let first = env.render_str(
        "{% macro card(x) %}<{{ x }}>{% endmacro %}{% do items.append(card) %}\
         {% do items.append(joiner(',')) %}{{ items[0](1) }}",
        &ctx,
    );
    assert_eq!(first.unwrap(), "<1>");
    assert_eq!(items.len(), 2);
    let later = env.render_str("{{ items[0](1) }}", &ctx).unwrap_err();
    assert_eq!(later.kind, ErrorKind::Runtime);
    assert!(
        later.message.contains("'card'") && later.message.contains("render that has ended"),
        "{}",
        later.message
    );
    assert!(later.hint.contains("import"), "{}", later.hint);
    assert_eq!(env.render_str("{{ items[1]() }}", &ctx).unwrap_err().kind, ErrorKind::Runtime);
}

#[test]
fn lists_extend_with_themselves() {
    let out = Environment::default().render_str(
        "{% set l = [7] * 2000 %}{% do l.extend(l) %}{{ l|length }} {{ l|sum }}",
        &Value::default(),
    );
    assert_eq!(out.unwrap(), "4000 28000");
}

#[test]
fn failed_renders_leave_stored_values_whole() {
    let items = Value::list([]);
    let ctx = Value::dict();
    ctx.set("items", items.clone());
    let err = Environment::default()
        .render_str("{% do items.append('kept' ~ 1) %}{{ missing.attr }}", &ctx)
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Undefined);
    assert_eq!(items.repr(), "['kept1']");
}
