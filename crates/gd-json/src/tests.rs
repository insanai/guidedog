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
    let a = v.get("a").and_then(Value::as_array).cloned().unwrap_or_default();
    assert_eq!(a[0].as_i64(), Some(1));
    assert_eq!(a[1].as_f64(), Some(-2.5));
    assert!(a[2].is_f64() && a[2].as_f64() == Some(300.0));
    assert_eq!(a[3].as_bool(), Some(true));
    assert!(a[5].is_null());
    assert_eq!(v.get("b").and_then(|b| b.get("c")).and_then(Value::as_str), Some("d"));
    let keys = v.as_object().map(|o| o.keys().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(keys, Some(vec!["a", "b", "e"]));
    assert_eq!(ok("9223372036854775808").as_u64(), Some(9223372036854775808));
    assert_eq!(ok("\u{feff}0"), Value::from(0));
    let kinds: Vec<&str> = a.iter().map(kind_name).collect();
    assert_eq!(kinds, ["an integer", "a number", "a number", "a boolean", "a boolean", "null"]);
}

#[test]
fn reads_escapes_and_surrogate_pairs() {
    let v = ok(r#""a\"b\\c\/d\b\f\n\r\té😀\ud83d\ude00""#);
    assert_eq!(v.as_str(), Some("a\"b\\c/d\u{8}\u{c}\n\r\té😀😀"));
    assert_eq!(err(r#""\ud83d""#).kind, ErrorKind::String);
    assert_eq!(err(r#""\x""#).kind, ErrorKind::String);
    assert_eq!(err("\"a\nb\"").kind, ErrorKind::String);
    assert_eq!(err("\"open").kind, ErrorKind::Eof);
}

#[test]
fn refuses_malformed_text() {
    let cases = [
        ("01", ErrorKind::Number),
        ("-", ErrorKind::Eof),
        ("1.", ErrorKind::Eof),
        ("1e", ErrorKind::Eof),
        ("1x", ErrorKind::Syntax),
        ("1e999", ErrorKind::Number),
        ("[1.e5]", ErrorKind::Number),
        ("+1", ErrorKind::Syntax),
        ("[1,]", ErrorKind::Syntax),
        ("[1 2]", ErrorKind::Syntax),
        ("[1", ErrorKind::Eof),
        ("{\"a\" 1}", ErrorKind::Syntax),
        ("{a: 1}", ErrorKind::Syntax),
        ("tru", ErrorKind::Eof),
        ("trux", ErrorKind::Syntax),
        ("1 2", ErrorKind::Syntax),
        ("", ErrorKind::Eof),
        ("{\"a\": 1, \"a\": 2}", ErrorKind::Duplicate),
    ];
    for (text, kind) in cases {
        assert_eq!(err(text).kind, kind, "{text}: {}", err(text));
    }
    let e = err("{\n  \"a\": 1,\n  \"b\" 2\n}");
    assert_eq!((e.line, e.column), (3, 7));
    assert_eq!(e.message, "Expected a colon after this object key.");
    let e = err("[1,\n 2,\n");
    assert_eq!((e.kind, e.line, e.column, e.offset), (ErrorKind::Eof, 2, 4, 7));
}

#[test]
fn limits_bound_depth_and_allow_duplicates() {
    let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert_eq!(err(&deep).kind, ErrorKind::Depth);
    let most = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
    assert!(parse(&most).is_ok());
    let limits = Limits { max_depth: 2, ..Limits::default() };
    assert!(parse_with("[[1]]", &limits).is_ok());
    let e = parse_with("[{\"a\": [1]}]", &limits).err();
    assert_eq!(e.map(|e| (e.kind, e.column)), Some((ErrorKind::Depth, 8)));
    let loose = Limits { allow_duplicate_keys: true, ..Limits::default() };
    let v = parse_with("{\"a\": 1, \"b\": 0, \"a\": 2}", &loose).unwrap_or_default();
    assert_eq!(to_string(&v), "{\"a\":2,\"b\":0}");
}

#[test]
fn large_objects_keep_order_and_find_keys() {
    let members: Vec<String> = (0..100).map(|i| format!("\"k{i}\": {i}")).collect();
    let text = format!("{{{}}}", members.join(", "));
    let v = ok(&text);
    assert_eq!(v.get("k77").and_then(Value::as_i64), Some(77));
    assert_eq!(v.as_object().and_then(|o| o.keys().nth(42)).map(String::as_str), Some("k42"));
    let dup = format!("{{{}, \"k50\": 0}}", members.join(", "));
    assert_eq!(err(&dup).kind, ErrorKind::Duplicate);
    let mut o: Object = (0..40).map(|i| (format!("x{i}"), Value::from(i))).collect();
    assert_eq!(o.shift_remove("x3"), Some(Value::from(3)));
    assert_eq!(o.get("x39"), Some(&Value::from(39)));
    assert_eq!(o.keys().nth(3).map(String::as_str), Some("x4"));
    assert_eq!(o.len(), 39);
}

#[test]
fn writes_compact_and_pretty() {
    let v = ok(concat!(
        "{\"s\": \"<a & \\\"b\\\">\\n\\u0001\\u2028\", ",
        "\"n\": [1, 2.0, 1e300, {}, []], \"z\": null}"
    ));
    let compact = to_string(&v);
    assert_eq!(
        compact,
        concat!(
            "{\"s\":\"\\u003ca \\u0026 \\\"b\\\"\\u003e\\n\\u0001\\u2028\",",
            "\"n\":[1,2.0,1e+300,{},[]],\"z\":null}"
        )
    );
    assert_eq!(ok(&compact), v);
    let pretty = to_string_pretty(&v);
    assert!(pretty.starts_with("{\n  \"s\": \""), "{pretty}");
    assert!(pretty.contains("\"n\": [\n    1,\n    2.0,\n    1e+300,\n    {},\n    []\n  ],"));
    assert_eq!(ok(&pretty), v);
    assert_eq!(to_string(&Value::from(f64::NAN)), "null");
    assert_eq!(to_string(&Value::from(None::<i64>)), "null");
    let mut out = String::from("x = ");
    write_pretty(&mut out, &ok("[1]"), 1);
    assert_eq!(out, "x = [\n    1\n  ]");
    let mut out = String::new();
    write_string(&mut out, "</script>");
    assert_eq!(out, "\"\\u003c/script\\u003e\"");
}

#[test]
fn errors_render_as_reports() {
    let text = "{\"a\": 1, \"a\": 2}";
    let e = err(text);
    let report = gd_report::render_text(&e.report("data.json", text), &Default::default());
    assert!(report.starts_with("-- DUPLICATE KEY [json.duplicate] ---"), "{report}");
    assert!(report.contains("1| {\"a\": 1, \"a\": 2}\n            ^^^\n"), "{report}");
    assert!(report.contains("Hint: "), "{report}");
    let text = "[1,";
    let report = gd_report::render_text(&err(text).report("x.json", text), &Default::default());
    assert!(report.starts_with("-- JSON ENDS EARLY [json.eof] ---"), "{report}");
    assert!(report.contains("close the open brackets"), "{report}");
}

#[test]
fn converts_with_serde() {
    let v = ok("{\"name\": \"guide\", \"pages\": [1, 2]}");
    let pages: Vec<u32> =
        from_value(v.get("pages").cloned().unwrap_or_default()).unwrap_or_default();
    assert_eq!(pages, [1, 2]);
    let e = from_value::<Vec<u32>>(v.clone()).err();
    assert_eq!(e.as_ref().map(|e| (e.kind, e.line)), Some((ErrorKind::Data, 0)));
    assert_eq!(to_value(&vec!["a", "b"]).ok(), Some(ok("[\"a\", \"b\"]")));
}
