use super::*;

fn ok(text: &str) -> Value {
    match parse(text) {
        Ok(v) => v,
        Err(e) => panic!("{text:?}: {e}"),
    }
}

fn err(text: &str) -> Error {
    match parse(text) {
        Ok(v) => panic!("{text:?} parsed as {v:?}"),
        Err(e) => e,
    }
}

#[test]
fn reads_every_kind() {
    let v = ok(" {\"a\": [1, -2.5, 3e2, true, false, null], \"b\": {\"c\": \"d\"}, \"e\": {}} ");
    let a = v.get("a").and_then(Value::as_array).unwrap_or_default();
    assert_eq!(a[0].as_i64(), Some(1));
    assert_eq!(a[1].as_f64(), Some(-2.5));
    assert_eq!(a[2], Value::Float(300.0));
    assert_eq!(a[3].as_bool(), Some(true));
    assert!(a[5].is_null());
    assert_eq!(v.get("b").and_then(|b| b.get("c")).and_then(Value::as_str), Some("d"));
    assert_eq!(v.as_object().map(|o| o.keys().collect::<Vec<_>>()), Some(vec!["a", "b", "e"]));
    assert_eq!(ok("9223372036854775808"), Value::Float(9223372036854775808.0));
    assert_eq!(ok("\u{feff}0"), Value::Integer(0));
}

#[test]
fn reads_escapes_and_surrogate_pairs() {
    let v = ok(r#""a\"b\\c\/d\b\f\n\r\té😀""#);
    assert_eq!(v.as_str(), Some("a\"b\\c/d\u{8}\u{c}\n\r\té😀"));
    assert_eq!(err(r#""\ud83d""#).kind, ErrorKind::String);
    assert_eq!(err(r#""\ude00""#).kind, ErrorKind::String);
    assert_eq!(err(r#""\x""#).kind, ErrorKind::String);
    assert_eq!(err("\"a\nb\"").kind, ErrorKind::String);
    assert_eq!(err("\"open").kind, ErrorKind::String);
}

#[test]
fn refuses_malformed_text() {
    let cases = [
        ("01", ErrorKind::Number),
        ("-", ErrorKind::Number),
        ("1.", ErrorKind::Number),
        ("1e", ErrorKind::Number),
        ("+1", ErrorKind::Syntax),
        ("[1,]", ErrorKind::Syntax),
        ("[1 2]", ErrorKind::Syntax),
        ("{\"a\" 1}", ErrorKind::Syntax),
        ("{a: 1}", ErrorKind::Syntax),
        ("tru", ErrorKind::Syntax),
        ("1 2", ErrorKind::Syntax),
        ("", ErrorKind::Syntax),
        ("{\"a\": 1, \"a\": 2}", ErrorKind::Duplicate),
    ];
    for (text, kind) in cases {
        assert_eq!(err(text).kind, kind, "{text}");
    }
    let e = err("{\n  \"a\": 1,\n  \"b\" 2\n}");
    assert_eq!((e.line, e.column), (3, 7));
}

#[test]
fn limits_bound_depth_and_allow_duplicates() {
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert_eq!(err(&deep).kind, ErrorKind::Depth);
    let limits = Limits { max_depth: 2, ..Limits::default() };
    assert!(parse_with("[[1]]", &limits).is_ok());
    assert!(parse_with("[[[1]]]", &limits).is_err());
    let loose = Limits { allow_duplicate_keys: true, ..Limits::default() };
    let v = parse_with("{\"a\": 1, \"b\": 0, \"a\": 2}", &loose).unwrap_or_default();
    assert_eq!(to_string(&v), "{\"a\":2,\"b\":0}");
}

#[test]
fn large_objects_use_the_index() {
    let members: Vec<String> = (0..100).map(|i| format!("\"k{i}\": {i}")).collect();
    let text = format!("{{{}}}", members.join(", "));
    let v = ok(&text);
    assert_eq!(v.get("k77").and_then(Value::as_i64), Some(77));
    let dup = format!("{{{}, \"k50\": 0}}", members.join(", "));
    assert_eq!(err(&dup).kind, ErrorKind::Duplicate);
    let mut o: Object = (0..40).map(|i| (format!("x{i}"), i as i64)).collect();
    assert_eq!(o.remove("x3"), Some(Value::Integer(3)));
    assert_eq!(o.get("x39"), Some(&Value::Integer(39)));
    assert_eq!(o.len(), 39);
}

#[test]
fn writes_compact_and_pretty() {
    let v =
        ok("{\"s\": \"<a & \\\"b\\\">\\n\\u0001\", \"n\": [1, 2.0, 1e300, {}, []], \"z\": null}");
    let compact = to_string(&v);
    assert_eq!(
        compact,
        concat!(
            "{\"s\":\"\\u003ca \\u0026 \\\"b\\\"\\u003e\\n\\u0001\",",
            "\"n\":[1,2.0,1e300,{},[]],\"z\":null}"
        )
    );
    assert_eq!(ok(&compact), v);
    let pretty = to_string_pretty(&v);
    assert!(pretty.starts_with("{\n  \"s\": \""), "{pretty}");
    assert!(pretty.contains("\"n\": [\n    1,\n    2.0,\n    1e300,\n    {},\n    []\n  ],"));
    assert_eq!(ok(&pretty), v);
    assert_eq!(to_string(&Value::Float(f64::NAN)), "null");
    assert_eq!(to_string(&Value::from(None::<i64>)), "null");
}

#[test]
fn errors_render_as_reports() {
    let text = "{\"a\": 01}";
    let e = err(text);
    let report = gd_report::render_text(&e.report("data.json", text), &Default::default());
    assert!(report.starts_with("-- BAD JSON NUMBER [json.number] ---"), "{report}");
    assert!(report.contains("1| {\"a\": 01}\n         ^\n"), "{report}");
}

#[test]
fn standard_traits() {
    let v: Value = "[1, {\"a\": null}]".parse().unwrap_or_default();
    assert_eq!(v.to_string(), "[1,{\"a\":null}]");
    let o = v.as_array().and_then(|a| a[1].as_object()).cloned().unwrap_or_default();
    assert_eq!((&o).into_iter().map(|(k, _)| k).collect::<Vec<_>>(), ["a"]);
}
