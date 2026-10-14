use crate::{
    Catalog, ErrorKind, Message, ReferenceStyle, WriteOptions, parse_po, parse_po_bytes, write_po,
};

const SAMPLE: &str = r#"# German translation.
# Copyright (C) 2026
#, fuzzy
msgid ""
msgstr ""
"Project-Id-Version: Demo 1.0\n"
"Language: de\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\n"

# A translator's note.
#. Extracted for translators.
#: index.rst:12 guide/usage.rst:3
#, c-format, fuzzy
#| msgid "Old %d file"
msgid "Hello, %s"
msgstr "Hallo, %s"

msgctxt "menu"
msgid "Open"
msgstr "Öffnen"

msgid ""
"Line one\n"
"line \"two\"\t\\ end"
msgstr "Zeile\x41\101\n"

msgid "file"
msgid_plural "files"
msgstr[0] "Datei"
msgstr[1] "Dateien"

msgid "untranslated"
msgstr ""

#~ msgid "gone"
#~ msgstr "weg"

#, fuzzy
#~| msgid "older"
#~ msgid "gone too"
#~ msgstr "auch weg"
"#;

#[test]
fn po_reads_every_construct() {
    let cat = parse_po(SAMPLE, "de.po").expect("SAMPLE parses");
    let header = cat.header.as_ref().expect("a header");
    assert!(header.fuzzy);
    assert_eq!(header.comments.len(), 2);
    assert_eq!(cat.header_field("language"), Some("de"));
    assert_eq!(cat.messages.len(), 7);
    let m = &cat.messages[0];
    assert_eq!(m.id, "Hello, %s");
    assert_eq!(m.strs[0], "Hallo, %s");
    assert!(m.fuzzy);
    assert_eq!(m.flags[0], "c-format");
    assert_eq!(m.comments[0], "A translator's note.");
    assert_eq!(m.notes[0], "Extracted for translators.");
    assert_eq!(m.references, ["index.rst:12", "guide/usage.rst:3"]);
    assert_eq!(m.previous.as_ref().map(|p| p.id.as_str()), Some("Old %d file"));
    assert_eq!(m.line, 11);
    let open = &cat.messages[1];
    assert_eq!(open.ctx.as_deref(), Some("menu"));
    assert_eq!(open.strs[0], "Öffnen");
    let escapes = &cat.messages[2];
    assert_eq!(escapes.id, "Line one\nline \"two\"\t\\ end");
    assert_eq!(escapes.strs[0], "ZeileAA\n");
    let files = &cat.messages[3];
    assert_eq!(files.id_plural.as_deref(), Some("files"));
    assert_eq!(files.strs.len(), 2);
    assert!(cat.messages[5].obsolete);
    let last = &cat.messages[6];
    assert!(last.obsolete && last.fuzzy);
    assert_eq!(last.previous.as_ref().map(|p| p.id.as_str()), Some("older"));
    assert_eq!(cat.plural.nplurals, 2);
}

#[test]
fn po_round_trips() {
    let cat = parse_po(SAMPLE, "de.po").expect("SAMPLE parses");
    let once = write_po(&cat, &WriteOptions::default());
    let again = parse_po(&once, "again.po").expect("the written text parses");
    let twice = write_po(&again, &WriteOptions::default());
    assert_eq!(twice, once);
    assert_eq!(again.messages.len(), cat.messages.len());
    for (m, n) in cat.messages.iter().zip(&again.messages) {
        assert!(m.same_key(n));
        assert_eq!(n.strs, m.strs);
        assert_eq!(n.fuzzy, m.fuzzy);
        assert_eq!(n.obsolete, m.obsolete);
    }
}

#[test]
fn po_writes_msgmerge_layout() {
    let mut cat = Catalog::new("x.po");
    let mut m = Message::new(
        "A long message that certainly does not fit on one line of seventy-six characters.",
    );
    m.references = vec!["index.rst:1".into(), "index.rst:22".into()];
    m.comments.push("Note".into());
    m.fuzzy = true;
    m.flags.push("python-format".into());
    cat.add_message(m);
    let mut n = Message::new("two\nlines");
    n.strs[0] = "zwei\nZeilen".into();
    cat.add_message(n);
    let out = write_po(&cat, &WriteOptions::default());
    let want = r#"# Note
#: index.rst:1 index.rst:22
#, fuzzy, python-format
msgid ""
"A long message that certainly does not fit on one line of seventy-six "
"characters."
msgstr ""

msgid ""
"two\n"
"lines"
msgstr ""
"zwei\n"
"Zeilen"
"#;
    assert_eq!(out, want);
    for line in out.lines() {
        assert!(line.chars().count() <= 76, "too wide: {line:?}");
    }
}

#[test]
fn po_reference_styles() {
    let mut cat = Catalog::new("");
    let mut m = Message::new("x");
    m.references = vec!["a.rst:1".into(), "b.rst:2".into()];
    cat.add_message(m);
    let mut o = WriteOptions { references: ReferenceStyle::OnePerLine, ..WriteOptions::default() };
    assert_eq!(write_po(&cat, &o), "#: a.rst:1\n#: b.rst:2\nmsgid \"x\"\nmsgstr \"\"\n");
    o.references = ReferenceStyle::None;
    assert_eq!(write_po(&cat, &o), "msgid \"x\"\nmsgstr \"\"\n");
}

#[track_caller]
fn expect_error(text: &[u8], kind: ErrorKind, line: usize, fragment: &str) {
    let Err(err) = parse_po_bytes(text, "bad.po") else {
        panic!("{:?} parsed", String::from_utf8_lossy(text));
    };
    assert_eq!(err.kind, kind);
    assert_eq!(err.line, line);
    let report = err.to_string();
    assert!(report.contains(fragment), "{report:?} lacks {fragment:?}");
    assert!(report.contains("Hint: "));
    assert!(report.starts_with("-- "));
}

#[test]
fn po_errors_are_explained() {
    use ErrorKind::{Charset, Duplicate, Encoding, Plural, Syntax};
    expect_error(b"msgid \"a\"\nmsgstr \"b\n", Syntax, 2, "no closing quote");
    expect_error(b"msgid \"a\"\nmsgstr \"\\q\"\n", Syntax, 2, "not a C escape");
    expect_error(b"msgid \"a\"\nmsgid \"b\"\nmsgstr \"\"\n", Syntax, 2, "no msgstr");
    expect_error(b"msgid \"a\"\nmsgstr[0] \"b\"\n", Syntax, 2, "msgid_plural");
    let skipped = b"msgid \"a\"\nmsgid_plural \"b\"\nmsgstr[1] \"x\"\n";
    expect_error(skipped, Syntax, 3, "Expected msgstr[0]");
    let twice = b"msgid \"a\"\nmsgstr \"\"\n\nmsgid \"a\"\nmsgstr \"\"\n";
    expect_error(twice, Duplicate, 4, "already defined on line 1");
    expect_error(b"msgstr \"a\"\n", Syntax, 1, "no msgid");
    expect_error(b"msgid \"a\"\n", Syntax, 1, "no msgstr");
    expect_error(b"msgid \"a\" x\nmsgstr \"\"\n", Syntax, 1, "after the closing quote");
    expect_error(b"frobnicate \"a\"\n", Syntax, 1, "not a PO keyword");
    expect_error(b"\"a\"\n", Syntax, 1, "continues no msgid");
    let latin = b"msgid \"\"\nmsgstr \"Content-Type: text/plain; charset=ISO-8859-1\\n\"\n";
    expect_error(latin, Charset, 1, "ISO-8859-1");
    let rule = b"msgid \"\"\nmsgstr \"Plural-Forms: nplurals=2; plural=n !=;\\n\"\n";
    expect_error(rule, Plural, 1, "not valid");
    expect_error(b"msgid \"a\xff\"\nmsgstr \"\"\n", Encoding, 1, "0xff");
    expect_error(b"msgid \"\\377\"\nmsgstr \"\"\n", Syntax, 1, "not UTF-8");
}

#[test]
fn po_error_report_shape() {
    let err = parse_po("msgid \"a\"\nmsgstr \"b\n", "de.po").expect_err("no quote");
    let report = err.to_string();
    let lines: Vec<&str> = report.lines().collect();
    assert!(lines[0].starts_with("-- PO SYNTAX ERROR ---"));
    assert!(lines[0].ends_with(" de.po:2"));
    assert_eq!(lines[0].len(), 80);
    assert_eq!(lines[4], "2| msgstr \"b");
    assert_eq!(lines[5], "          ^");
}

#[test]
fn po_accepts_crlf_bom_and_templates() {
    let text = "\u{feff}msgid \"\"\r\nmsgstr \"Content-Type: text/plain; charset=CHARSET\\n\"\
                \r\n\r\nmsgid \"a\"\r\nmsgstr \"\"\r\n";
    let cat = parse_po(text, "t.pot").expect("a template parses");
    assert_eq!(cat.messages.len(), 1);
    assert_eq!(cat.messages[0].id, "a");
}

#[test]
fn po_escapes_borrow_when_nothing_changes() {
    assert!(matches!(crate::escape_string("plain text"), std::borrow::Cow::Borrowed(_)));
    assert_eq!(crate::escape_string("a\"b\\\n\x01"), "a\\\"b\\\\\\n\\001");
}
