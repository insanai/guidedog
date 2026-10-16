//! Context reads stay observable when compiled templates are reused.
#![allow(clippy::unwrap_used)]
use gd_jinja::{Environment, Value};

#[test]
fn context_traces_reads_missing_values_and_context_imports() {
    let env = Environment::default();
    let ctx = Value::dict();
    ctx.set("used", Value::from("value"));
    ctx.set("unused", Value::from("other"));
    let Value::Dict(dict) = &ctx else { panic!("dict") };
    dict.begin_key_trace();
    env.render_str("{{ used }}{% if missing %}hidden{% endif %}", &ctx).unwrap();
    let keys = dict.end_key_trace();
    assert!(keys.contains(&"used".into()));
    assert!(keys.contains(&"missing".into()));
    assert!(!keys.contains(&"unused".into()), "{keys:?}");
}
