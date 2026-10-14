use crate::{
    MergeOptions, Stats, WriteOptions, format_stats, merge, parse_po, similarity,
    sort_by_reference, stats, write_po,
};

const OLD: &str = r#"msgid ""
msgstr ""
"Language: de\n"
"POT-Creation-Date: 2026-01-01 00:00+0000\n"
"Content-Type: text/plain; charset=UTF-8\n"

# keep this comment
#: index.rst:3
msgid "Welcome to the project."
msgstr "Willkommen beim Projekt."

#: index.rst:9
msgid "Install the package with the installer."
msgstr "Installieren Sie das Paket mit dem Installationsprogramm."

#: old.rst:1
msgid "A removed paragraph."
msgstr "Ein entfernter Absatz."

#: old.rst:2
msgid "Never translated."
msgstr ""
"#;

const NEW: &str = r#"msgid ""
msgstr ""
"POT-Creation-Date: 2026-09-29 10:00+0000\n"
"Content-Type: text/plain; charset=UTF-8\n"

#: usage.rst:2
msgid "Something new entirely."
msgstr ""

#: index.rst:3
msgid "Welcome to the project."
msgstr ""

#: index.rst:10
msgid "Install the package with the new installer."
msgstr ""
"#;

#[test]
fn merge_updates_like_msgmerge() {
    let def = parse_po(OLD, "de.po").expect("OLD parses");
    let reference = parse_po(NEW, "index.pot").expect("NEW parses");
    let out = merge(&def, &reference, &MergeOptions::default());
    assert_eq!(out.messages.len(), 4);
    let fresh = &out.messages[0];
    assert_eq!(fresh.id, "Something new entirely.");
    assert!(!fresh.is_translated() && !fresh.fuzzy);
    let kept = &out.messages[1];
    assert_eq!(kept.strs[0], "Willkommen beim Projekt.");
    assert!(!kept.fuzzy);
    assert_eq!(kept.comments[0], "keep this comment");
    let changed = &out.messages[2];
    assert!(changed.fuzzy);
    let previous = changed.previous.as_ref().map(|p| p.id.as_str());
    assert_eq!(previous, Some("Install the package with the installer."));
    assert_eq!(changed.strs[0], "Installieren Sie das Paket mit dem Installationsprogramm.");
    assert_eq!(changed.references[0], "index.rst:10");
    let gone = &out.messages[3];
    assert!(gone.obsolete);
    assert_eq!(gone.id, "A removed paragraph.");
    assert_eq!(out.header_field("POT-Creation-Date"), Some("2026-09-29 10:00+0000"));
    assert_eq!(out.header_field("Language"), Some("de"));
    let s = stats(&out);
    assert_eq!(s, Stats { translated: 1, fuzzy: 1, untranslated: 1, obsolete: 1 });
    assert_eq!(format_stats(&s), "1 translated, 1 fuzzy, 1 untranslated.");
    let text = write_po(&out, &WriteOptions::default());
    assert!(text.contains("#~ msgid \"A removed paragraph.\"\n"));
    assert!(text.contains("#, fuzzy\n#| msgid \"Install the package with the installer.\"\n"));
}

#[test]
fn merge_without_fuzzy_matching() {
    let def = parse_po(OLD, "de.po").expect("OLD parses");
    let reference = parse_po(NEW, "index.pot").expect("NEW parses");
    let options = MergeOptions { fuzzy_matching: false, keep_obsolete: false };
    let out = merge(&def, &reference, &options);
    assert_eq!(out.messages.len(), 3);
    assert!(!out.messages[2].is_translated());
}

#[test]
fn merge_resurrects_obsolete_messages() {
    let def = parse_po("#~ msgid \"back\"\n#~ msgstr \"zurück\"\n", "de.po")
        .expect("an obsolete message parses");
    let reference = parse_po("msgid \"back\"\nmsgstr \"\"\n", "x.pot").expect("parses");
    let out = merge(&def, &reference, &MergeOptions::default());
    assert_eq!(out.messages.len(), 1);
    assert_eq!(out.messages[0].strs[0], "zurück");
    assert!(!out.messages[0].obsolete && out.messages[0].fuzzy);
}

#[test]
fn merge_fits_plural_forms() {
    let header = "msgid \"\"\nmsgstr \"Plural-Forms: nplurals=3; plural=0;\\n\"\n";
    let def = parse_po(header, "ru.po").expect("the header parses");
    let template = "msgid \"file\"\nmsgid_plural \"files\"\nmsgstr[0] \"\"\nmsgstr[1] \"\"\n";
    let reference = parse_po(template, "x.pot").expect("the template parses");
    let out = merge(&def, &reference, &MergeOptions::default());
    assert_eq!(out.messages[0].strs.len(), 3);
}

#[test]
fn similarity_measures() {
    assert_eq!(similarity("abc", "abc"), 1.0);
    assert_eq!(similarity("", ""), 1.0);
    assert_eq!(similarity("abc", "xyz"), 0.0);
    assert_eq!(similarity("abcd", "abxd"), 0.75);
    assert!(crate::fuzzy::similarity_bound(b"abcd", b"abxd") >= similarity("abcd", "abxd"));
}

#[test]
fn sorting_by_reference() {
    let text = "#: b.rst:2\nmsgid \"b2\"\nmsgstr \"\"\n\n#: a.rst:10 a.rst:9\nmsgid \"a10\"\n\
                msgstr \"\"\n\nmsgid \"none\"\nmsgstr \"\"\n\n#: a.rst:2\nmsgid \"a2\"\n\
                msgstr \"\"\n";
    let mut cat = parse_po(text, "x.pot").expect("parses");
    sort_by_reference(&mut cat);
    let ids: Vec<&str> = cat.messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["a2", "a10", "b2", "none"]);
    assert_eq!(cat.messages[1].references[0], "a.rst:9");
}
