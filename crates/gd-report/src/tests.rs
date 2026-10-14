use super::*;

fn plain() -> Style {
    Style::default()
}

#[test]
fn renders_the_elm_shape() {
    let source = "Intro\n\nSee instal_ for details.\n";
    let start = source.find("instal_").unwrap_or(0);
    let report = Report::error("", "UNRESOLVED REFERENCE", "No target is named `instal`.")
        .hint("Define it with `.. _instal:` above the section.")
        .hint("Did you mean `install`?")
        .at("intro.rst", source, start, start + 7);
    let text = render_text(&report, &plain());
    let expected = "-- UNRESOLVED REFERENCE ------------------------------------------ \
                    intro.rst:3:5\n\nNo target is named `instal`.\n\n\
                    3| See instal_ for details.\n       ^^^^^^^\n\n\
                    Hint: Define it with `.. _instal:` above the section.\n\
                    Did you mean `install`?\n";
    assert_eq!(text, expected);
    assert_eq!(text.lines().next().map(display_width), Some(80));
}

#[test]
fn shows_the_line_before_for_context() {
    let source = "a = 1\nb = = 2\n";
    let report = Report::error("toml.syntax", "SYNTAX ERROR", "Unexpected `=`.").at(
        "conf.toml",
        source,
        10,
        11,
    );
    let text = render_text(&report, &plain());
    assert!(text.contains("1| a = 1\n2| b = = 2\n       ^\n"), "{text}");
    assert!(text.starts_with("-- SYNTAX ERROR [toml.syntax] ---"));
}

#[test]
fn carets_follow_wide_characters_and_escapes() {
    let source = "漢字\u{1b}x = bad";
    let start = source.find("bad").unwrap_or(0);
    let report = Report::error("", "T", "m").at("f", source, start, start + 3);
    let text = render_text(&report, &plain());
    assert!(text.contains("1| 漢字\\u001bx = bad\n"), "{text}");
    // 4 cells + 6 cells + "x = " = 14 cells before the span.
    assert!(text.contains(&format!("\n{}^^^\n", " ".repeat(3 + 14))), "{text}");
    assert_eq!(report.column, 8);
}

#[test]
fn long_lines_are_windowed_around_the_span() {
    let source = format!("{}target{}", "a".repeat(300), "b".repeat(300));
    let report = Report::error("", "T", "m").at("f", &source, 300, 306);
    let text = render_text(&report, &plain());
    let line = text.lines().find(|l| l.starts_with("1| ")).unwrap_or("");
    assert!(line.starts_with("1| …") && line.ends_with('…'));
    assert!(display_width(line) <= 3 + 160);
    let carets = text.lines().find(|l| l.trim_start().starts_with('^')).unwrap_or("");
    let at = line.chars().count() - line[line.find("target").unwrap_or(0)..].chars().count();
    assert_eq!(carets.find('^'), Some(at));
}

#[test]
fn severities_and_colors() {
    let report = Report {
        severity: Severity::Warning,
        title: "UNUSED".into(),
        path: "a\u{1b}b".into(),
        ..Report::default()
    };
    let text = render_text(&report, &plain());
    assert!(text.starts_with("-- WARNING: UNUSED ---"));
    assert!(text.trim_end().ends_with("a\\u001bb"));
    let colored = render_text(&report, &Style { color: true, width: 80 });
    assert!(colored.contains("\x1b[1;33mWARNING: UNUSED\x1b[0m"));
}

#[test]
fn json_has_the_stable_fields() {
    let report = Report::error("x.y", "T", "say \"hi\"\n").hint("h").at("p", "ab\ncd", 4, 5);
    let json = render_json(&report);
    assert_eq!(
        json,
        "{\"schema\":1,\"code\":\"x.y\",\"severity\":\"error\",\"title\":\"T\",\
         \"message\":\"say \\\"hi\\\"\\n\",\"hint\":\"h\",\"hints\":[\"h\"],\"path\":\"p\",\
         \"line\":2,\"column\":2,\"end_line\":2,\"end_column\":3,\"source\":\"cd\",\
         \"category\":\"\"}"
    );
}

#[test]
fn positions_count_scalars_and_clamp() {
    assert_eq!(line_column("ab\nçd", 5), (2, 2));
    assert_eq!(line_column("ab\nçd", 4), (2, 1));
    assert_eq!(line_column("ab", 99), (1, 3));
    assert_eq!(line_bounds("a\r\nb", 0), (0, 1));
    assert_eq!(line_offset("a\nb\nc", 3), Some(4));
    assert_eq!(line_offset("a\nb", 3), None);
    let mut r = Report::default();
    set_line_column(&mut r, "x\nçab\n", 2, 3);
    assert_eq!((r.line, r.column), (2, 3));
    assert_eq!(r.excerpt.map(|e| e.start), Some(3));
}

#[test]
fn suggestions_and_wrapping() {
    assert_eq!(suggest("instal", ["install", "index"]), Some("install"));
    assert_eq!(suggest("zzz", ["install"]), None);
    assert_eq!(suggest("install", ["install", "instal"]), None);
    assert_eq!(edit_distance("kitten", "sitting"), 3);
    assert_eq!(wrap("aa bb cc", 5), "aa bb\ncc\n");
    assert_eq!(wrap("  aa bb\nx", 5), "  aa\n  bb\nx\n");
}
