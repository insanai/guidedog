use super::*;

fn ok(source: &str) -> Table {
    match parse(source) {
        Ok(t) => t,
        Err(e) => panic!("{source:?}: {e}"),
    }
}

fn err(source: &str) -> Error {
    match parse(source) {
        Ok(t) => panic!("{source:?} parsed as {t:?}"),
        Err(e) => e,
    }
}

fn at<'t>(t: &'t Table, path: &[&str]) -> &'t Value {
    t.path(path).unwrap_or_else(|| panic!("missing {path:?}"))
}

#[test]
fn reads_scalars() {
    let t = ok("a = 1\nb = 0x0 # c\nc = 0o17\nd = 0b101\ne = 1_000\nf = 3.5e2\ng = true\n\
                h = -inf\ni = nan\nj = +1.0\n");
    assert_eq!(at(&t, &["a"]).as_integer(), Some(1));
    assert_eq!(at(&t, &["c"]).as_integer(), Some(15));
    assert_eq!(at(&t, &["d"]).as_integer(), Some(5));
    assert_eq!(at(&t, &["e"]).as_integer(), Some(1000));
    assert_eq!(at(&t, &["f"]).as_float(), Some(350.0));
    assert_eq!(at(&t, &["g"]).as_bool(), Some(true));
    assert_eq!(at(&t, &["h"]).as_float(), Some(f64::NEG_INFINITY));
    assert!(at(&t, &["i"]).as_float().is_some_and(f64::is_nan));
    assert_eq!(at(&t, &["j"]).line, 10);
    assert_eq!(t.keys().collect::<Vec<_>>(), ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]);
}

#[test]
fn reads_every_string_kind() {
    let t = ok(concat!(
        "a = \"tab\\there \\u00e9 \\U0001F600 \\\"q\\\"\"\n",
        "b = 'C:\\path'\n",
        "c = \"\"\"\nline one\nline two\\\n    joined\"\"\"\n",
        "d = '''\nraw \\n text'''\n",
        "e = \"\"\"two quotes\"\"\"\"\"\n",
        "f = ''''one quote''''\n",
    ));
    assert_eq!(at(&t, &["a"]).as_str(), Some("tab\there é 😀 \"q\""));
    assert_eq!(at(&t, &["b"]).as_str(), Some("C:\\path"));
    assert_eq!(at(&t, &["c"]).as_str(), Some("line one\nline twojoined"));
    assert_eq!(at(&t, &["d"]).as_str(), Some("raw \\n text"));
    assert_eq!(at(&t, &["e"]).as_str(), Some("two quotes\"\""));
    assert_eq!(at(&t, &["f"]).as_str(), Some("'one quote'"));
}

#[test]
fn reads_tables_keys_and_arrays() {
    let t = ok(concat!(
        "title = \"x\"\n",
        "site.\"base url\" = \"/\"\n",
        "[server]\nhost = \"h\"\nports = [ 80, 443, ] # trailing comma\n",
        "[server.tls]\non = { enabled = true, nested.deep = 1 }\n",
        "[[products]]\nname = \"a\"\n[[products]]\nname = \"b\"\n[products.meta]\nx = 1\n",
        "matrix = [[1, 2], [\"a\"],\n  # comment\n  []]\n",
    ));
    assert_eq!(at(&t, &["site", "base url"]).as_str(), Some("/"));
    assert_eq!(at(&t, &["server", "ports"]).as_array().map(<[Value]>::len), Some(2));
    assert_eq!(at(&t, &["server", "tls", "on", "nested", "deep"]).as_integer(), Some(1));
    let products = at(&t, &["products"]).as_array().unwrap_or_default();
    assert_eq!(products.len(), 2);
    assert_eq!(
        products[1].get("meta").and_then(|m| m.get("x")).and_then(Value::as_integer),
        Some(1)
    );
    let matrix =
        at(&t, &["products"]).as_array().and_then(|p| p.last()).and_then(|p| p.get("meta"));
    assert_eq!(
        matrix.and_then(|m| m.get("matrix")).and_then(Value::as_array).map(<[Value]>::len),
        Some(3)
    );
    assert_eq!(products[1].line, 10);
}

#[test]
fn reads_datetimes() {
    let t = ok("a = 1979-05-27T07:32:00Z\nb = 1979-05-27 00:32:00.999999-07:00\n\
                c = 1979-05-27T07:32:00\nd = 1979-05-27\ne = 07:32:00.5\n");
    let text = |k: &str| at(&t, &[k]).as_datetime().map(ToString::to_string);
    assert_eq!(text("a").as_deref(), Some("1979-05-27T07:32:00Z"));
    assert_eq!(text("b").as_deref(), Some("1979-05-27T00:32:00.999999-07:00"));
    assert_eq!(text("c").as_deref(), Some("1979-05-27T07:32:00"));
    assert_eq!(text("d").as_deref(), Some("1979-05-27"));
    assert_eq!(text("e").as_deref(), Some("07:32:00.5"));
    assert_eq!(err("a = 2023-02-29").kind, ErrorKind::Datetime);
    assert_eq!(err("a = 07:32").kind, ErrorKind::Datetime);
    assert_eq!(Datetime::parse("2024-02-29").map(|d| d.date.is_some()), Some(true));
}

#[test]
fn refuses_duplicates_and_bad_extensions() {
    for source in [
        "a = 1\na = 2",
        "[a]\n[a]",
        "a = {}\n[a]",
        "a = {x = 1}\na.y = 2",
        "[a]\nb.c = 1\n[a.b]",
        "[a.b.c]\nz = 1\n[a]\nb.c.t = 2",
        "a = [1]\n[[a]]",
        "[[a]]\n[a]",
        "a = 1\na.b = 2",
        "a = 1\n[a.b]",
        "\"a\" = 1\na = 2",
    ] {
        assert_eq!(err(source).kind, ErrorKind::Duplicate, "{source}");
    }
    let e = err("x = 1\ny = 2\nx = 3\n");
    assert_eq!((e.line, e.column), (3, 1));
    assert!(e.message.contains("line 1"), "{}", e.message);
    ok("[a.b]\nx = 1\n[a]\ny = 2\n[fruit]\napple.color = 1\n[fruit.apple.texture]\nz = 1\n");
}

#[test]
fn refuses_malformed_input() {
    let cases = [
        ("a = 01", ErrorKind::Number),
        ("a = 1__0", ErrorKind::Number),
        ("a = _1", ErrorKind::Number),
        ("a = 1.", ErrorKind::Number),
        ("a = .5", ErrorKind::Number),
        ("a = 9223372036854775808", ErrorKind::Number),
        ("a = bare", ErrorKind::Number),
        ("a = \"open", ErrorKind::String),
        ("a = \"\\x\"", ErrorKind::String),
        ("a = \"\\uD800\"", ErrorKind::String),
        ("a = '\u{1}'", ErrorKind::String),
        ("a = \"\"\"x\"\"\"\"\"\"", ErrorKind::String),
        ("a = 1 b = 2", ErrorKind::Syntax),
        ("a = {x = 1,}", ErrorKind::Syntax),
        ("a = {x = 1\n}", ErrorKind::Syntax),
        ("a = [1 2]", ErrorKind::Syntax),
        ("[a", ErrorKind::Syntax),
        ("= 1", ErrorKind::Syntax),
        ("a =", ErrorKind::Syntax),
        ("a = 1 # \u{7f}", ErrorKind::Syntax),
        ("a = -0x1", ErrorKind::Number),
    ];
    for (source, kind) in cases {
        assert_eq!(err(source).kind, kind, "{source}");
    }
}

#[test]
fn deep_nesting_is_refused_without_exhausting_the_stack() {
    for (open, close) in [("[", "]"), ("{a=", "}")] {
        let source = format!("a={}1{}", open.repeat(200_000), close.repeat(200_000));
        let e = err(&source);
        assert_eq!((e.kind, e.line), (ErrorKind::Depth, 1));
    }
}

#[test]
fn errors_render_as_reports() {
    let source = "name = \"guide\"\nversion = 01\n";
    let e = err(source);
    let text = gd_report::render_text(&e.report("conf.toml", source), &Default::default());
    assert!(text.starts_with("-- BAD NUMBER [toml.number] ---"), "{text}");
    assert!(text.contains("conf.toml:2:11"), "{text}");
    assert!(text.contains("2| version = 01\n             ^^\n"), "{text}");
    assert!(text.contains("Hint: Remove the leading zeros"), "{text}");
}

#[test]
fn writes_documents_that_read_back_equal() {
    let source = concat!(
        "title = \"Guide \\\"one\\\"\"\nprolog = '''\nA \\ b\n'''\n",
        "n = 1\nf = 1e300\ng = 2.0\nh = -inf\nwhen = 1979-05-27T07:32:00Z\n",
        "list = [\"aaaaaaaaaaaaaaaa\", \"bbbbbbbbbbbbbbbbbbbb\", ",
        "\"cccccccccccccccccc\", \"ddddddd\"]\n",
        "inline = { \"a b\" = 1, c = [] }\n",
        "[a.b]\nx = 1\n[[a.items]]\ny = 2\n[a.items.sub]\nz = 3\n[[a.items]]\n[empty]\n",
    );
    let table = ok(source);
    let text = to_string(&table);
    assert_eq!(ok(&text), table, "{text}");
    assert!(text.contains("list = [\n    \"aaaa"), "{text}");
    assert!(text.contains("prolog = '''\nA \\ b\n'''"), "{text}");
    assert!(text.contains("[[a.items]]\ny = 2\n\n[a.items.sub]\nz = 3\n"), "{text}");
}

#[test]
fn builds_tables_in_code() {
    let mut t = Table::new();
    for i in 0..40 {
        t.insert(&format!("k{i}"), i as i64);
    }
    assert_eq!(t.get("k33").and_then(Value::as_integer), Some(33));
    assert_eq!(t.remove("k0").and_then(|v| v.as_integer()), Some(0));
    assert_eq!(t.get("k39").and_then(Value::as_integer), Some(39));
    assert_eq!(t.insert("k1", "one").and_then(|v| v.as_integer()), Some(1));
    assert_eq!(t.keys().next(), Some("k1"));
    let mut out = String::new();
    write_value(&mut out, &Value::from(vec![Value::from(1.0), Value::from("x\ty")]));
    assert_eq!(out, "[1.0, \"x\\ty\"]");
}

#[test]
fn parsed_values_own_their_input() {
    let mut input = String::from("a='b'");
    let t = ok(&input);
    input.clear();
    assert_eq!(t.get("a").and_then(Value::as_str), Some("b"));
}

#[test]
fn standard_traits() {
    let t: Table = "a = 1\nb = 2".parse().unwrap_or_default();
    assert_eq!((&t).into_iter().map(|(k, _)| k).collect::<Vec<_>>(), ["a", "b"]);
    assert!("1979-05-27x".parse::<Datetime>().is_err());
    assert_eq!(
        "1979-05-27".parse::<Datetime>().map(|d| d.to_string()).as_deref(),
        Ok("1979-05-27")
    );
}
