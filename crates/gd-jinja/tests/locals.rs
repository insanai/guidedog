//! Per-render functions and filters (`Locals`) that borrow the caller's data.

#![allow(clippy::unwrap_used)]

use std::cell::{Cell, RefCell};

use gd_jinja::{Environment, ErrorKind, Locals, Value};

/// The caller's own data, borrowed by the render's functions.
struct Site {
    pages: Vec<String>,
    calls: Cell<usize>,
}

fn locals_for(site: &Site) -> Locals<'_> {
    let mut locals = Locals::new();
    locals.add_function("page", |c| {
        site.calls.set(site.calls.get() + 1);
        let i = c.arg_count(0, "index", 0)?;
        match site.pages.get(i) {
            Some(p) => Ok(Value::from(p.as_str())),
            None => Err(c.error("no such page")),
        }
    });
    locals.add_filter("shout", |c| {
        Ok(Value::from(
            gd_jinja::to_str(&c.args[0]).to_uppercase() + &"!".repeat(site.pages.len()),
        ))
    });
    locals
}

fn site() -> Site {
    Site { pages: vec!["intro".into(), "usage".into()], calls: Cell::new(0) }
}

#[test]
fn functions_and_filters_borrow_the_callers_data() {
    let site = site();
    let locals = locals_for(&site);
    let env = Environment::default();
    let out = env.render_str_with("{{ page(1) }} {{ page(0)|shout }}", &Value::default(), &locals);
    assert_eq!(out.unwrap(), "usage INTRO!!");
    assert_eq!(site.calls.get(), 2);
}

#[test]
fn locals_are_seen_in_macros_includes_blocks_and_call_blocks() {
    let site = site();
    let locals = locals_for(&site);
    let env = Environment::default();
    env.add_template("part.html", "[{{ page(0) }}]").unwrap();
    env.add_template("base.html", "<{% block b %}{{ page(1)|shout }}{% endblock %}>").unwrap();
    env.add_template("macros.html", "{% macro m() %}m:{{ page(1) }}{% endmacro %}").unwrap();
    let source = "{% extends 'base.html' %}{% block b %}{{ super() }}\
        {% include 'part.html' %}{% macro w() %}({{ caller() }}){% endmacro %}\
        {% call w() %}{{ page(0) }}{% endcall %}\
        {% from 'macros.html' import m %}{{ m() }}{% endblock %}";
    let t = env.parse_template("page.html", source).unwrap();
    let out = t.render_with(&Value::default(), &locals).unwrap();
    assert_eq!(out, "<USAGE!![intro](intro)m:usage>");
}

#[test]
fn locals_shadow_globals_and_filters_but_not_the_context() {
    let site = site();
    let locals = locals_for(&site);
    let mut env = Environment::default();
    env.add_function("page", |_| Ok(Value::from("global")));
    env.add_filter("shout", |_| Ok(Value::from("global filter")));
    let ctx = Value::dict();
    assert_eq!(
        env.render_str_with("{{ page(0) }}{{ 'a'|shout }}", &ctx, &locals).unwrap(),
        "introA!!"
    );
    // Without locals, the environment's own are used.
    assert_eq!(env.render_str("{{ page(0) }}", &ctx).unwrap(), "global");
    ctx.set("page", Value::from("context"));
    assert_eq!(env.render_str_with("{{ page }}", &ctx, &locals).unwrap(), "context");
}

#[test]
fn errors_of_locals_point_at_the_template() {
    let site = site();
    let locals = locals_for(&site);
    let env = Environment::default();
    let err = env.render_str_with("\n{{ page(5) }}", &Value::default(), &locals).unwrap_err();
    assert_eq!(err.line, 2);
    assert!(err.message.contains("no such page"), "{err}");
    let err = env.render_str_with("{{ pag(1) }}", &Value::default(), &locals).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Undefined);
    assert!(err.hint.contains("page"), "{err}");
    let err = env.render_str_with("{{ 1|shot }}", &Value::default(), &locals).unwrap_err();
    assert!(err.hint.contains("shout"), "{err}");
}

#[test]
fn a_local_kept_past_its_render_cannot_be_called() {
    let kept = RefCell::new(Value::default());
    let env = Environment::default();
    {
        let mut locals = Locals::new();
        locals.add_function("page", |_| Ok(Value::from("x")));
        locals.add_function("store", |c| {
            *kept.borrow_mut() = c.arg(0, "f");
            Ok(Value::None)
        });
        env.render_str_with("{{ store(page) }}", &Value::default(), &locals).unwrap();
    }
    let ctx = Value::dict();
    ctx.set("f", kept.borrow().clone());
    let err = env.render_str("{{ f() }}", &ctx).unwrap_err();
    assert!(err.message.contains("render that has ended"), "{err}");
}
