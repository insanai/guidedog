use crate::*;

#[test]
fn syntax_keeps_exact_file_and_utf8_spans() {
    let source = "% a comment\n{ c'4 ^\"cafe\" % { ignored\n d8[ e] }";
    let syntax = parse_lilypond(source, "music.rst", &mut Options::default()).unwrap();
    for node in &syntax.nodes {
        assert!(source.is_char_boundary(node.span.start));
        assert!(source.is_char_boundary(node.span.end));
        assert_eq!(node.span.file, FileId(0));
    }
    assert_eq!(syntax.files.file(FileId(0)).unwrap().text, source);
    assert!(syntax.nodes.iter().any(|node| matches!(&node.kind,
        ExprKind::Atom(TokenKind::Text(s)) if s == "cafe")));
}

#[test]
fn includes_are_lexical_nested_and_keep_resolved_sources() {
    let mut seen = Vec::new();
    let mut include = |request: IncludeRequest<'_>| {
        seen.push((request.from, request.name.to_string()));
        Ok(match request.name {
            "part.ily" => IncludedFile {
                name: "parts/part.ily".into(),
                text: "c4 \\include \"detail.ily\"".into(),
            },
            "detail.ily" => IncludedFile { name: "parts/detail.ily".into(), text: "d4".into() },
            _ => {
                return Err(IncludeRefusal { message: "missing".into(), hint: "fix path".into() });
            }
        })
    };
    let syntax = parse_lilypond(
        "{ \\include \"part.ily\" e4 }",
        "main.ly",
        &mut Options { include: Some(&mut include), ..Options::default() },
    )
    .unwrap();
    assert_eq!(syntax.files.files.len(), 3);
    assert_eq!(seen, [(FileId(0), "part.ily".into()), (FileId(1), "detail.ily".into())]);
    assert!(syntax.nodes.iter().any(|n| n.span.file == FileId(2)));
}

#[test]
fn includes_refuse_cycles_missing_policy_and_expansion_budgets() {
    let failed =
        parse_lilypond("\\include \"x.ily\"", "main.ly", &mut Options::default()).unwrap_err();
    assert_eq!(failed.error.kind, ErrorKind::Include);
    let mut cycle =
        |_: IncludeRequest<'_>| Ok(IncludedFile { name: "main.ly".into(), text: "c4".into() });
    let failed = parse_lilypond(
        "\\include \"x.ily\"",
        "main.ly",
        &mut Options { include: Some(&mut cycle), ..Options::default() },
    )
    .unwrap_err();
    assert!(failed.error.message.contains("cycle"));
    let source = "\\include \"x\" \\include \"x\"";
    let mut repeated =
        |_: IncludeRequest<'_>| Ok(IncludedFile { name: "x".into(), text: "c4 ".repeat(8) });
    let failed = parse_lilypond(
        source,
        "main.ly",
        &mut Options {
            limits: Limits { source_bytes: source.len() + 30, ..Limits::default() },
            include: Some(&mut repeated),
        },
    )
    .unwrap_err();
    assert_eq!(failed.error.kind, ErrorKind::ReadLimit);
}

#[test]
fn language_includes_need_no_file_policy() {
    let syntax =
        parse_lilypond("\\include \"english.ly\" { cs4 df }", "main.ly", &mut Options::default())
            .unwrap();
    assert_eq!(syntax.files.files.len(), 1);
    assert!(syntax.nodes.iter().any(|n| matches!(&n.kind,
        ExprKind::Atom(TokenKind::Command(name)) if name == "language")));
}

#[test]
fn syntax_and_work_limits_fail_without_panicking() {
    for source in ["{ c4", "c4 }", "<< c4 >", "%{ unfinished", "\"unterminated", "\\"] {
        assert!(parse_lilypond(source, "bad.ly", &mut Options::default()).is_err(), "{source}");
    }
    let mut options =
        Options { limits: Limits { depth: 4, ..Limits::default() }, ..Options::default() };
    assert!(parse_lilypond("{{{{{{c4}}}}}}", "deep.ly", &mut options).is_err());
    options.limits.work = 2;
    assert_eq!(
        parse_lilypond("{ c4 d e f }", "long.ly", &mut options).unwrap_err().error.kind,
        ErrorKind::ReadLimit
    );
}
