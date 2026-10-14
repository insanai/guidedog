use crate::{ErrorKind, compile_plural, parse_plural_forms, plural_forms_for, plural_index};

/// Rules from the GNU gettext manual and CLDR, as translation catalogs carry them, with
/// the form for n = 0, 1, 2, ...
const CASES: [(&str, &str, [usize; 6]); 6] = [
    ("en", "nplurals=2; plural=(n != 1);", [1, 0, 1, 1, 1, 1]),
    ("fr", "nplurals=2; plural=(n > 1);", [0, 0, 1, 1, 1, 1]),
    ("ja", "nplurals=1; plural=0;", [0, 0, 0, 0, 0, 0]),
    (
        "ru",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && \
         (n%100<10 || n%100>=20) ? 1 : 2);",
        [2, 0, 1, 1, 1, 2],
    ),
    (
        "pl",
        "nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        [2, 0, 1, 1, 1, 2],
    ),
    (
        "ar",
        "nplurals=6; plural=n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : \
         n%100>=11 ? 4 : 5;",
        [0, 1, 2, 3, 3, 3],
    ),
];

#[test]
fn plural_rules_of_several_languages() {
    for (language, forms, expect) in CASES {
        let rule = parse_plural_forms(forms).unwrap_or_else(|e| panic!("{language}: {e}"));
        for (n, want) in expect.into_iter().enumerate() {
            assert_eq!(plural_index(&rule, n as u64), want, "{language} n={n}");
        }
    }
}

#[test]
fn plural_rules_at_larger_numbers() {
    let check = |forms: &str, pairs: &[(u64, usize)]| {
        let rule = parse_plural_forms(forms).expect("the rule compiles");
        for &(n, want) in pairs {
            assert_eq!(plural_index(&rule, n), want, "{forms} n={n}");
        }
    };
    let ru = [(11, 2), (12, 2), (14, 2), (21, 0), (22, 1), (25, 2), (101, 0), (111, 2)];
    check(CASES[3].1, &ru);
    check(CASES[3].1, &[(1_000_001, 0)]);
    check(CASES[4].1, &[(12, 2), (21, 2), (22, 1), (112, 2), (122, 1)]);
    check(CASES[5].1, &[(11, 4), (99, 4), (100, 5), (102, 5), (103, 3), (111, 4)]);
}

#[test]
fn plural_expressions_follow_c() {
    let check = |expr: &str, n: u64, want: usize| {
        let rule = compile_plural(expr, 32).unwrap_or_else(|p| panic!("{expr}: {}", p.message));
        assert_eq!(plural_index(&rule, n), want, "{expr} with n={n}");
    };
    check("1 + 2 * 3", 0, 7);
    check("(1 + 2) * 3", 0, 9);
    check("10 - 2 - 3", 0, 5);
    check("n / 2", 7, 3);
    check("n % 0", 7, 0); // no trap: division by zero is 0.
    check("n / 0", 7, 0);
    check("!n", 0, 1);
    check("!!n", 5, 1);
    check("n != 1", 1, 0);
    check("n <= 2 && n >= 2", 2, 1);
    check("n < 2 || n > 4", 3, 0);
    check("n ? 1 : 2 ? 3 : 4", 0, 3);
    check("0 - 1 > 5", 0, 1); // unsigned arithmetic, as in C.
    check("40", 0, 0); // out of range selects form 0.
}

#[test]
fn plural_refuses_bad_input() {
    let bad = [
        "",
        "n +",
        "(n",
        "n)",
        "n ? 1",
        "x",
        "n = 1",
        "nn",
        "1 2",
        "n <> 1",
        "99999999999999999999999",
        "-n",
        "n & 1",
        "n | 1",
    ];
    for expr in bad {
        assert!(compile_plural(expr, 2).is_err(), "{expr:?} was accepted");
    }
    assert!(compile_plural(&"(".repeat(200), 2).is_err());
    let long = format!("n{}", "+n".repeat(200));
    let problem = compile_plural(&long, 2).expect_err("too long");
    assert_eq!(problem.message, "the expression is too long");
    let err = parse_plural_forms("nplurals=0; plural=0;").expect_err("nplurals=0");
    assert_eq!(err.kind, ErrorKind::Plural);
    let err = parse_plural_forms("plural=n;").expect_err("no nplurals");
    assert_eq!(err.kind, ErrorKind::Plural);
}

#[test]
fn plural_error_points_into_the_header() {
    let err = parse_plural_forms("nplurals=2; plural=n +;").expect_err("incomplete");
    assert_eq!(err.excerpt, "nplurals=2; plural=n +;");
    assert_eq!(&err.excerpt[err.column - 1..], ";");
}

#[test]
fn plural_forms_for_new_catalogs() {
    for language in ["de", "fr", "ru", "pl", "ar", "ja", "pt-BR", "cs", "xx"] {
        let forms = plural_forms_for(language);
        assert!(parse_plural_forms(forms).is_ok(), "{language}: {forms}");
    }
    assert_eq!(plural_forms_for("pt_BR"), "nplurals=2; plural=(n > 1);");
    assert_eq!(plural_forms_for("ja"), "nplurals=1; plural=0;");
}
