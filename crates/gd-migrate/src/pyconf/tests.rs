use super::*;

fn scope() -> Scope {
    let mut scope = Scope { year: Some(2026), ..Scope::default() };
    scope.values.insert("name".into(), Value::Str("x".into()));
    scope
}

#[test]
fn python_literals_evaluate_without_python() {
    let mut scope = scope();
    let cases = [
        (r#"'a' "b" '''c'''"#, "abc"),
        (r"r'\d' + '\t'", "\\d\t"),
        ("'%s-%d' % ('v', 3)", "v-3"),
        ("'%(n)s!' % {'n': 'hi'}", "hi!"),
        ("f'{{{name}}}'", "{x}"),
        ("', '.join(['a', 'b'])", "a, b"),
        ("'{} and {0}'.format('p')", "p and p"),
        ("str(0x1F)", "31"),
        ("time.strftime('%Y')", "2026"),
        (r"'\u00e9\x41\101'", "éAA"),
        ("(\"grouped\")", "grouped"),
        ("'héllo'[1]", "é"),
    ];
    for (src, want) in cases {
        let got = eval(src, &mut scope);
        assert_eq!(got, Ok(Value::Str(want.into())), "{src}");
    }
}

#[test]
fn computed_expressions_are_refused() {
    let mut scope = scope();
    let failing = [
        "os.getenv('X')",
        "undefined_name",
        "[x for x in y]",
        "1 if a else 2",
        "f'{name!r}'",
        "b'raw'",
        "'unclosed",
    ];
    for src in failing {
        assert!(eval(src, &mut scope).is_err(), "{src} should not evaluate");
    }
}

#[test]
fn numbers_and_containers() {
    let mut scope = scope();
    assert_eq!(eval("-1_000", &mut scope), Ok(Value::Int(-1000)));
    assert_eq!(eval(".5", &mut scope), Ok(Value::Float("0.5".into())));
    assert_eq!(eval("date.today().year", &mut scope), Ok(Value::Int(2026)));
    assert_eq!(eval("('a',)", &mut scope), Ok(Value::List(vec![Value::Str("a".into())])));
    assert_eq!(
        eval("{'k': True}", &mut scope),
        Ok(Value::Dict(vec![(Value::Str("k".into()), Value::Bool(true))]))
    );
    assert!(eval("name + '!'", &mut scope).is_ok());
    assert!(scope.used.contains("name"));
}

#[test]
fn deep_nesting_is_refused_without_overflow() {
    let mut scope = scope();
    let src = format!("{}1{}", "[".repeat(10_000), "]".repeat(10_000));
    assert!(eval(&src, &mut scope).is_err());
}

#[test]
fn dict_calls_with_keywords_are_literals() {
    let mut scope = scope();
    let got = eval("dict(rankdir=\"LR\", size='\"6.0, 8.0\"',\n fontsize=14, )", &mut scope);
    let want = Value::Dict(vec![
        (Value::Str("rankdir".into()), Value::Str("LR".into())),
        (Value::Str("size".into()), Value::Str("\"6.0, 8.0\"".into())),
        (Value::Str("fontsize".into()), Value::Int(14)),
    ]);
    assert_eq!(got, Ok(want));
    assert_eq!(eval("dict()", &mut scope), Ok(Value::Dict(Vec::new())));
    assert!(eval("dict(other)", &mut scope).is_err());
    assert!(eval("dict(**base)", &mut scope).is_err());
}
