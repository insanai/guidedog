//! Ported from `styles_test.odin` (stylesheets and Typst output).

use crate::*;

fn css(style: Style, selector: &str) -> String {
    style_css(style, selector)
}

/// The stylesheet Pygments writes for its default style with ".highlight", written out by
/// hand from the style's definitions and Pygments' ordering rules.
const DEFAULT_CSS: &str = include_str!("data/default.css");

#[test]
fn default_stylesheet_matches_pygments() {
    let got = css(Style::Default, ".highlight");
    for (i, (want, got)) in DEFAULT_CSS.lines().zip(got.lines()).enumerate() {
        assert_eq!(got, want, "line {}", i + 1);
    }
    assert_eq!(got, DEFAULT_CSS);
}

#[test]
fn other_stylesheets() {
    let sphinx = css(Style::Sphinx, ".highlight");
    assert!(sphinx.contains("\n.highlight { background: #eeffcc; }\n"));
    assert!(
        sphinx.contains("\n.highlight .c { color: #408090; font-style: italic } /* Comment */\n")
    );
    assert!(sphinx.contains("\n.highlight .go { color: #333 } /* Generic.Output */\n"));
    assert!(sphinx.contains("\n.highlight .cp { color: #007020 } /* Comment.Preproc */\n"));
    assert!(sphinx.contains(
        "\n.highlight .cs { color: #408090; background-color: #FFF0F0 } /* Comment.Special */"
    ));
    let monokai = css(Style::Monokai, "html[data-theme=\"dark\"] .highlight");
    assert!(monokai.contains(
        "\nhtml[data-theme=\"dark\"] .highlight { background: #272822; color: #F8F8F2 }\n"
    ));
    assert!(monokai.contains(" .err { color: #ED007E; background-color: #1E0010 } /* Error */"));
    let friendly = css(Style::Friendly, ".highlight");
    assert!(friendly.starts_with(concat!(
        "pre { line-height: 125%; }\n",
        "td.linenos .normal { color: #666666; background-color: transparent;"
    )));
    for style in Style::ALL {
        assert_eq!(find_style(style_name(style)), Some(style));
    }
    assert_eq!(find_style("solarized"), None);
}

#[track_caller]
fn expect_lines(style: Style, lines: &[&str]) {
    let got = css(style, ".highlight");
    for line in lines {
        let found = got.contains(&format!("\n{line}\n")) || got.ends_with(&format!("\n{line}"));
        assert!(found, "{} lacks {line}", style_name(style));
    }
}

#[test]
fn generated_stylesheets() {
    assert_eq!(find_style("trac"), Some(Style::Trac));
    assert!(!style_info(Style::Trac).dark);
    expect_lines(
        Style::Trac,
        &[
            ".highlight .hll { background-color: #ffffcc }",
            ".highlight { background: #ffffff; }",
            ".highlight .c { color: #998; font-style: italic } /* Comment */",
            ".highlight .err { color: #A61717; background-color: #E3D2D2 } /* Error */",
            ".highlight .k { font-weight: bold } /* Keyword */",
            ".highlight .o { font-weight: bold } /* Operator */",
            ".highlight .cp { color: #999; font-weight: bold } /* Comment.Preproc */",
            concat!(
                ".highlight .cs { color: #999; font-weight: bold; font-style: italic } ",
                "/* Comment.Special */"
            ),
            ".highlight .gd { color: #000; background-color: #FDD } /* Generic.Deleted */",
            ".highlight .kt { color: #458; font-weight: bold } /* Keyword.Type */",
            ".highlight .ow { font-weight: bold } /* Operator.Word */",
            ".highlight .bp { color: #999 } /* Name.Builtin.Pseudo */",
            ".highlight .il { color: #099 } /* Literal.Number.Integer.Long */",
        ],
    );
    assert!(!css(Style::Trac, ".highlight").contains("/* Name */"));
    // zenburn is dark, sets every colour, and styles the root token, which Text inherits.
    assert!(style_info(Style::Zenburn).dark);
    expect_lines(
        Style::Zenburn,
        &[
            concat!(
                "td.linenos .normal { color: #5d6262; background-color: #353535; ",
                "padding-left: 5px; padding-right: 5px; }"
            ),
            concat!(
                "span.linenos.special { color: #7a8080; background-color: #353535; ",
                "padding-left: 5px; padding-right: 5px; }"
            ),
            ".highlight .hll { background-color: #484848 }",
            ".highlight { background: #3f3f3f; color: #DCDCCC }",
            ".highlight .err { color: #E37170; font-weight: bold } /* Error */",
            ".highlight .ge { color: #FFF; font-weight: bold } /* Generic.Emph */",
            concat!(
                ".highlight .gi { color: #709080; font-weight: bold; background-color: #313C36 } ",
                "/* Generic.Inserted */"
            ),
            ".highlight .w { color: #DCDCCC } /* Text.Whitespace */",
        ],
    );
}

#[test]
fn stylesheets_with_subtypes() {
    // Types outside the standard set are written when a style names them, with the class
    // Pygments makes from the nearest standard ancestor's.
    expect_lines(
        Style::GithubDark,
        &[
            concat!(
                ".highlight .g-Underline { color: #E6EDF3; text-decoration: underline } ",
                "/* Generic.Underline */"
            ),
            ".highlight .w { color: #6E7681 } /* Text.Whitespace */",
        ],
    );
    expect_lines(
        Style::GruvboxDark,
        &[".highlight .c-PreProc { color: #8EC07C; font-style: italic } /* Comment.PreProc */"],
    );
    expect_lines(
        Style::Lilypond,
        &[
            ".highlight .-ChordModifier { color: #976806 } /* ChordModifier */",
            ".highlight .-String-Escape { color: #C46C6C } /* String.Escape */",
            concat!(
                ".highlight .nb-MusicCommand { color: #08547A; font-weight: bold } ",
                "/* Name.Builtin.MusicCommand */"
            ),
        ],
    );
    let lilypond = css(Style::Lilypond, ".highlight");
    assert!(!lilypond.contains("-String-Symbol")); // noinherit of nothing.
    assert!(!lilypond.contains("-Pitch"));
    assert!(!css(Style::Default, ".highlight").contains("g-Underline"));
    assert!(css(Style::None, ".highlight").ends_with("\n.highlight { background: #ffffff; }"));
    assert_eq!(find_style(" Solarized-Dark "), Some(Style::SolarizedDark));
    assert!(style_info(Style::SolarizedDark).dark);
}

fn typst(lexer: Lexer, code: &str) -> String {
    let mut out = String::new();
    write_code_typst(lexer, code, &mut out);
    out
}

#[test]
fn typst_output() {
    assert_eq!(
        typst(Lexer::Python, "def f():\n    return \"a\\\\b\"\n"),
        concat!(
            "(((\"k\", \"def\"), (\"w\", \" \"), (\"nf\", \"f\"), (\"p\", \"():\"), ), ",
            "((\"\", \"    \"), (\"k\", \"return\"), (\"\", \" \"), (\"s2\", \"\\\"a\"), ",
            "(\"se\", \"\\\\\\\\\"), (\"s2\", \"b\\\"\"), ), )"
        )
    );
    assert_eq!(typst(Lexer::Text, "a\n\nb"), "(((\"\", \"a\"), ), (), ((\"\", \"b\"), ), )");
    assert_eq!(typst(Lexer::Text, ""), "()");
    // Rust strings are valid UTF-8, so the Odin test's invalid byte is U+FFFD here.
    assert_eq!(
        typst(Lexer::Text, "\t\u{1}\u{fffd}\u{e9}"),
        "(((\"\", \"\\t\\u{1}\u{fffd}é\"), ), )"
    );
    assert_eq!(
        typst(Lexer::Yaml, "- a"),
        "(((\"p\", \"-\"), (\"w\", \" \"), (\"l\", \"a\"), ), )"
    );
    let mut theme = String::new();
    write_typst_theme(Style::Default, &mut theme);
    assert!(theme.starts_with(concat!(
        "(\"background\": rgb(\"#f8f8f8\"), \"text\": (fill: none), \"w\": (fill: ",
        "rgb(\"#bbbbbb\")), \"err\": (fill: none, border: rgb(\"#FF0000\")), \"k\": ",
        "(fill: rgb(\"#008000\"), weight: \"bold\")"
    )));
}
