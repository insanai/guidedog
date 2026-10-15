//! Elm-style reports for unknown languages, unknown styles and code that does not lex.

use crate::*;

fn report(p: &Problem<'_>, location: &str) -> String {
    let mut out = String::new();
    write_problem(p, location, &mut out);
    out
}

#[test]
fn unknown_language_report() {
    let p = check_code("pyhton", "x = 1").expect("a problem");
    assert_eq!(p.kind, ProblemKind::UnknownLanguage);
    assert_eq!(
        report(&p, "index.rst:12"),
        concat!(
            "-- UNKNOWN LANGUAGE ----------------------------------------------- index.rst:12\n\n",
            "I do not know how to highlight \"pyhton\", so this block is shown as plain text.\n\n",
            "Hint: Did you mean \"python\"?\n"
        )
    );
    assert_eq!(suggest_lexer("yamll"), Some("yaml"));
    assert_eq!(suggest_lexer("cobol"), None);
    assert_eq!(check_code("", "x"), None);
    assert_eq!(check_code("default", "$ make"), None);
    assert_eq!(check_code("python", "x = 1"), None);
}

#[test]
fn lexing_error_report() {
    let p = check_code("json", "{\n  \"é\": tru\n}").expect("a problem");
    assert_eq!(p.kind, ProblemKind::LexingError);
    assert_eq!(p.line, 2);
    assert_eq!(p.column, 8);
    assert_eq!(p.near, "tru");
    assert_eq!(
        report(&p, ""),
        concat!(
            "-- CODE DOES NOT LEX -----------------------------------------------------------\n\n",
            "This block is marked as \"json\", but it does not lex as that language, so the\n",
            "part that does not is marked as an error:\n\n2|   \"é\": tru\n          ^^^\n\n",
            "Hint: Check the block's language, or mark it \"text\" to show it without ",
            "highlighting.\n"
        )
    );
    assert_eq!(p.to_string(), report(&p, ""));
}

#[test]
fn unknown_style_report() {
    let p = check_style("monokia").expect("a problem");
    assert_eq!(p.kind, ProblemKind::UnknownStyle);
    assert_eq!(p.suggestion, Some("monokai"));
    assert_eq!(check_style("sphinx"), None);
}
