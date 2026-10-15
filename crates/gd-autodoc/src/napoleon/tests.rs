//! Sphinx's tests of napoleon's helpers (`test_ext_napoleon_docstring.py`): type
//! specifications, the name pattern, escaping, and the member filter. Whole docstrings
//! are compared in `tests/napoleon.rs`.

use super::patterns::{
    is_enumerated, is_google_section, is_numeric, match_name, split_xref_or_code,
};
use super::types::{TokenType, convert_type_spec, recombine_sets, token_type, tokenize};
use super::*;

#[test]
fn token_types() {
    let cases = [
        ("1", TokenType::Literal),
        ("-4.6", TokenType::Literal),
        ("2j", TokenType::Literal),
        ("'string'", TokenType::Literal),
        ("\"another_string\"", TokenType::Literal),
        ("{1, 2}", TokenType::Literal),
        ("{'va{ue', 'set'}", TokenType::Literal),
        ("optional", TokenType::Control),
        ("default", TokenType::Control),
        (", ", TokenType::Delimiter),
        (" of ", TokenType::Delimiter),
        (" or ", TokenType::Delimiter),
        (": ", TokenType::Delimiter),
        ("True", TokenType::Obj),
        ("None", TokenType::Obj),
        ("name", TokenType::Obj),
        (":py:class:`Enum`", TokenType::Reference),
    ];
    for (token, expected) in cases {
        assert_eq!(token_type(token).0, expected, "{token}");
    }
}

#[test]
fn malformed_tokens_are_reported() {
    let cases = [
        ("{1, 2", "invalid value set (missing closing brace)"),
        ("}", "invalid value set (missing opening brace)"),
        ("'abc", "malformed string literal (missing closing quote)"),
        ("def'", "malformed string literal (missing opening quote)"),
        ("\"ghi", "malformed string literal (missing closing quote)"),
        ("jkl\"", "malformed string literal (missing opening quote)"),
    ];
    for (token, what) in cases {
        let (kind, problem) = token_type(token);
        assert_eq!(kind, TokenType::Literal);
        assert_eq!(problem.map(|p| p.what), Some(what), "{token}");
    }
}

#[test]
fn type_specs_are_tokenized() {
    let cases: &[(&str, &[&str])] = &[
        ("str", &["str"]),
        ("defaultdict", &["defaultdict"]),
        ("int, float, or complex", &["int", ", ", "float", ", or ", "complex"]),
        (
            "int or float or None, optional",
            &["int", " or ", "float", " or ", "None", ", ", "optional"],
        ),
        (
            "list of list of int or float, optional",
            &["list", " of ", "list", " of ", "int", " or ", "float", ", ", "optional"],
        ),
        (
            "tuple of list of str, float, or int",
            &["tuple", " of ", "list", " of ", "str", ", ", "float", ", or ", "int"],
        ),
        ("{\"F\", \"C\", \"N\"}", &["{", "\"F\"", ", ", "\"C\"", ", ", "\"N\"", "}"]),
        (
            "{'F', 'C', 'N'}, default: 'F'",
            &["{", "'F'", ", ", "'C'", ", ", "'N'", "}", ", ", "default", ": ", "'F'"],
        ),
        (
            "{'F', 'C', 'N or C'}, default 'F'",
            &["{", "'F'", ", ", "'C'", ", ", "'N or C'", "}", ", ", "default", " ", "'F'"],
        ),
        ("str, default: 'F or C'", &["str", ", ", "default", ": ", "'F or C'"]),
        ("int, default: None", &["int", ", ", "default", ": ", "None"]),
        ("int, default None", &["int", ", ", "default", " ", "None"]),
        ("int, default :obj:`None`", &["int", ", ", "default", " ", ":obj:`None`"]),
        ("\"ma{icious\"", &["\"ma{icious\""]),
        ("'with \\'quotes\\''", &["'with \\'quotes\\''"]),
    ];
    for (spec, expected) in cases {
        assert_eq!(tokenize(spec), *expected, "{spec}");
    }
}

#[test]
fn set_tokens_are_recombined() {
    let cases: &[(&[&str], &[&str])] = &[
        (&["{", "1", ", ", "2", "}"], &["{1, 2}"]),
        (
            &["{", "\"F\"", ", ", "\"C\"", ", ", "\"N\"", "}", ", ", "optional"],
            &["{\"F\", \"C\", \"N\"}", ", ", "optional"],
        ),
        (
            &["{", "'F'", ", ", "'C'", ", ", "'N'", "}", ", ", "default", ": ", "None"],
            &["{'F', 'C', 'N'}", ", ", "default", ": ", "None"],
        ),
        (&["{", "1", ", ", "2"], &["{1, 2"]),
        (
            &["\"F\"", ", ", "\"C\"", ", ", "\"N\"", "}", ", ", "optional"],
            &["\"F\"", ", ", "\"C\"", ", ", "\"N\"", "}", ", ", "optional"],
        ),
        (
            &["{", "1", ", ", "2", ", ", "default", ": ", "None"],
            &["{1, 2", ", ", "default", ": ", "None"],
        ),
    ];
    for (tokens, expected) in cases {
        assert_eq!(recombine_sets(tokens), *expected, "{tokens:?}");
    }
}

#[test]
fn numpy_type_specs_are_converted() {
    let aliases = vec![("DataFrame".to_owned(), "pandas.DataFrame".to_owned())];
    let cases = [
        ("", ""),
        ("optional", "*optional*"),
        ("str, optional", ":py:class:`str`, *optional*"),
        (
            "int or float or None, default: None",
            ":py:class:`int` or :py:class:`float` or :py:obj:`None`, *default*: :py:obj:`None`",
        ),
        (
            "list of tuple of str, optional",
            ":py:class:`list` of :py:class:`tuple` of :py:class:`str`, *optional*",
        ),
        ("int, default None", ":py:class:`int`, *default* :py:obj:`None`"),
        ("{\"F\", \"C\", \"N\"}", "``{\"F\", \"C\", \"N\"}``"),
        ("{'F', 'C', 'N'}, default: 'N'", "``{'F', 'C', 'N'}``, *default*: ``'N'``"),
        ("{'F', 'C', 'N'}, default 'N'", "``{'F', 'C', 'N'}``, *default* ``'N'``"),
        ("DataFrame, optional", ":py:class:`pandas.DataFrame`, *optional*"),
        ("... or Ellipsis", ":py:obj:`... <Ellipsis>` or :py:obj:`Ellipsis`"),
    ];
    for (spec, expected) in cases {
        assert_eq!(convert_type_spec(spec, &aliases, &mut Vec::new()), expected, "{spec}");
    }
}

#[test]
fn numpy_names_escape_args_and_kwargs() {
    let settings = Napoleon { use_param: false, ..Napoleon::default() };
    let lines: Vec<String> = ["Parameters", "----------", "*args, **kwargs : int", "    Rest."]
        .map(String::from)
        .to_vec();
    let out = convert_docstring(DocstringStyle::Numpy, &lines, &settings, &Default::default());
    assert_eq!(out.lines, [":Parameters: **\\*args, \\*\\*kwargs** (*int*) -- Rest.", ""]);
}

#[test]
fn patterns_match_as_python_does() {
    assert!(is_google_section("Args:"));
    assert!(is_google_section("  Keyword Args:  "));
    assert!(!is_google_section("Args::"));
    assert!(!is_google_section("See: x"));
    assert!(is_enumerated("1. item") && is_enumerated("(a) item") && is_enumerated("iv."));
    assert!(!is_enumerated("1) item") && !is_enumerated("ab. item"));
    assert!(is_numeric("1e3") && is_numeric("1+2j") && is_numeric("inf") && !is_numeric("e5"));
    let m = match_name(":py:class:`int` rest").expect("a role");
    assert_eq!((m.role, m.name, m.ticked), (Some("py:class"), "int", true));
    let m = match_name("  func_name : text").expect("a name");
    assert_eq!((m.role, m.name, m.ticked, m.end), (None, "func_name", false, 12));
    assert_eq!(split_xref_or_code("a :ref:`b: c` d"), ["a ", ":ref:`b: c`", " d"]);
    assert_eq!(split_xref_or_code("``x : y``: z"), ["", "``x : y``", ": z"]);
}

#[test]
fn malformed_fields_keep_the_rest_as_written() {
    let settings = Napoleon::default();
    let lines: Vec<String> =
        ["Summary.", "", "Args:", "    x (int) the x.", "", "Returns:", "    int: y"]
            .map(String::from)
            .to_vec();
    let out = convert_docstring(DocstringStyle::Google, &lines, &settings, &Default::default());
    assert_eq!(out.notes.len(), 1);
    assert_eq!((out.notes[0].code, out.notes[0].line), ("napoleon.field", 3));
    assert_eq!(out.lines[out.lines.len() - 3..], lines[4..]);
    assert_eq!(out.origins.len(), out.lines.len());
}

#[test]
fn unindented_sections_are_noted() {
    let settings = Napoleon::default();
    let lines: Vec<String> = ["Summary.", "", "Returns:", "the value"].map(String::from).to_vec();
    let out = convert_docstring(DocstringStyle::Google, &lines, &settings, &Default::default());
    assert_eq!(out.notes[0].code, "napoleon.section");
    assert_eq!(out.notes[0].line, 2);
    assert_eq!(out.lines, lines);
}

#[test]
fn custom_sections_naming_no_section_are_found() {
    let settings = Napoleon {
        custom_sections: vec![
            NapoleonSection { name: "Side Effects".into(), like: String::new() },
            NapoleonSection { name: "Args2".into(), like: "Argz".into() },
        ],
        ..Napoleon::default()
    };
    assert_eq!(unknown_custom_sections(&settings), ["Argz"]);
}

#[test]
fn the_member_filter_follows_the_settings() {
    let on = Napoleon {
        include_init_with_doc: true,
        include_private_with_doc: true,
        include_special_with_doc: true,
        ..Napoleon::default()
    };
    let off = Napoleon::default();
    for name in ["__init__", "_private", "__special__"] {
        assert!(includes_member(&on, "class", name, true), "{name}");
        assert!(!includes_member(&on, "class", name, false), "{name}");
        assert!(!includes_member(&off, "class", name, true), "{name}");
        assert!(!includes_member(&on, "function", name, true), "{name}");
    }
    assert!(!includes_member(&on, "class", "__weakref__", true));
    assert!(!includes_member(&on, "module", "public", true));
}

#[test]
fn both_styles_run_numpy_first() {
    let lines: Vec<String> = [
        "Summary.",
        "",
        "Parameters",
        "----------",
        "x : int",
        "    The x.",
        "",
        "",
        "Returns:",
        "    int: The y.",
    ]
    .map(String::from)
    .to_vec();
    let out = napoleon_docstring(&lines, &Napoleon::default(), &Default::default());
    let text = out.lines.join("\n");
    assert!(text.contains(":param x: The x.\n:type x: int"), "{text}");
    assert_eq!(
        text,
        "Summary.\n\n:param x: The x.\n:type x: int\n\n:returns: The y.\n:rtype: int\n"
    );
    assert_eq!(out.origins.len(), out.lines.len());
}
