//! A Sphinx-like theme: layouts extending layouts, macros, trans, host functions.

#![allow(clippy::unwrap_used)]

use gd_jinja::{Autoescape, Environment, Loader, Options, Value};

const BASIC: &str = r#"{%- macro relbar() %}
<div class="related"><ul>
{%- for rellink in rellinks|reverse %}
<li><a href="{{ pathto(rellink[0]) }}" title="{{ rellink[1]|striptags|e
    }}">{{ rellink[3] }}</a>{{ reldelim2 }}</li>
{%- endfor %}
{%- block rootrellink %}<li><a href="{{
    pathto(root_doc) }}">{{ shorttitle|e }}</a></li>{% endblock %}
</ul></div>
{%- endmacro %}
<!DOCTYPE html>
<html lang="{{ language }}">
<head><title>{{ title|striptags|e }}{{ titlesuffix }}</title>
{%- for css in css_files %}
<link rel="stylesheet" href="{{ pathto(css, 1) }}" />
{%- endfor %}
{%- block extrahead %}{% endblock %}
</head>
<body>
{%- block relbar1 %}{{ relbar() }}{% endblock %}
<div class="body">{% block body %}{% endblock %}</div>
{%- if theme_show_footer|tobool %}
<footer>{% trans copyright=copyright|e %}&copy; {{ copyright }}.{% endtrans %}</footer>
{%- endif %}
</body></html>"#;

const ALABASTER: &str = r#"{%- extends "basic/layout.html" %}
{%- block extrahead %}
{{ super() }}<meta name="theme" content="alabaster" />
{%- endblock %}
{%- block rootrellink %}<li>{{ _('Home') }}</li>{% endblock %}"#;

const PAGE: &str = r#"{%- extends "alabaster/layout.html" %}
{%- block body %}{{ body }}{% endblock %}"#;

const WANT: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head><title>Intro here &amp;mdash; Docs</title>
<link rel="stylesheet" href="../_static/a.css" />
<link rel="stylesheet" href="../_static/b.css" />
<meta name="theme" content="alabaster" />
</head>
<body>
<div class="related"><ul>
<li><a href="next.html" title="Next">next</a> |</li>
<li><a href="genindex.html" title="General Index">index</a> |</li><li>Home</li>
</ul></div>
<div class="body"><p>Hello</p></div>
<footer>&copy; 2026, A &amp; B.</footer>
</body></html>"#;

fn theme_environment() -> Environment {
    let loader = Loader::memory([
        ("basic/layout.html", BASIC),
        ("alabaster/layout.html", ALABASTER),
        ("page.html", PAGE),
    ]);
    let mut env = Environment::new(Options {
        autoescape: Autoescape::Always,
        loader: Some(loader),
        ..Options::default()
    });
    env.add_function("pathto", |c| {
        let target = gd_jinja::to_str(&c.arg(0, "otheruri")).into_owned();
        Ok(Value::from(if c.arg_flag(1, "resource", false) {
            format!("../{target}")
        } else {
            format!("{target}.html")
        }))
    });
    env.add_filter("tobool", |c| {
        Ok(Value::from(match c.arg(0, "value") {
            Value::Str(text) => text.to_lowercase() == "true" || &*text == "1",
            v => v.is_truthy(),
        }))
    });
    env
}

#[test]
fn sphinx_like_theme_renders() {
    let env = theme_environment();
    let ctx = Value::dict();
    let set = |key: &str, value: Value| ctx.set(key, value);
    set("language", Value::from("en"));
    set("title", Value::from("Intro <em>here</em>"));
    set("titlesuffix", Value::from(" &mdash; Docs"));
    set("css_files", Value::list([Value::from("_static/a.css"), Value::from("_static/b.css")]));
    let link = |a: &str, b: &str, c: &str, d: &str| Value::tuple([a, b, c, d].map(Value::from));
    set(
        "rellinks",
        Value::list([
            link("genindex", "General Index", "I", "index"),
            link("next", "<b>Next</b>", "N", "next"),
        ]),
    );
    set("reldelim2", Value::from(" |"));
    set("root_doc", Value::from("index"));
    set("shorttitle", Value::from("Docs"));
    set("body", Value::markup("<p>Hello</p>"));
    set("theme_show_footer", Value::from("true"));
    set("copyright", Value::from("2026, A & B"));
    let page = env.load_template("page.html").unwrap_or_else(|err| panic!("{err}"));
    let out = page.render(&ctx).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(out, WANT);
}
