//! Glossary translations keep index entries and stable links in sites and books.

use guidedog_tests::{Scratch, guidedog, has};

use crate::{build, quickstart};

const GLOSSARY: &str = "Repro\n=====\n\n.. glossary::\n\n\
    \x20  *Admission*\n      Letting a request through.\n\n\
    \x20  Session\n      A token held by the client.\n\n\
    \x20  Gate\n      The surface without inspection.\n\n\
    See :term:`Admission` and :term:`Session`.\n";

/// Updates real gettext catalogs rather than bypassing extraction or MO compilation.
fn translated_project(language: &str, term: &str, session: &str) -> Scratch {
    let dir = quickstart("translated-glossary", &[("index.rst", GLOSSARY)]);
    build(&dir.path, 0, &["gettext"]);
    guidedog(&dir.path, 0, &["intl", "update", "-l", language]);
    let path = format!("locales/{language}/LC_MESSAGES/index.po");
    let mut po = dir.read(&path);
    for (id, translated) in [("*Admission*", term), ("Session", session)] {
        let empty = format!("msgid \"{id}\"\nmsgstr \"\"");
        assert!(po.contains(&empty), "{po}");
        po = po.replace(&empty, &format!("msgid \"{id}\"\nmsgstr \"{translated}\""));
    }
    dir.write(&path, &po);
    dir
}

fn check_index(dir: &Scratch, term: &str, session: &str) {
    let page = dir.read("_build/html/index.html");
    let index = dir.read("_build/html/genindex.html");
    for (name, anchor) in [(term, "Admission"), (session, "Session"), ("Gate", "Gate")] {
        has(&page, &format!("id=\"term-{anchor}\""));
        has(&index, name);
        has(&index, &format!("href=\"index.html#term-{anchor}\""));
        assert_eq!(page.matches(&format!("id=\"term-{anchor}\"")).count(), 1);
    }
    has(&page, "href=\"#term-Admission\"");
    has(&page, "href=\"#term-Session\"");
}

#[test]
fn glossary_indexes_survive_translation_and_cached_rebuilds() {
    for (language, term, session) in [
        ("ja", "許可", "セッション"),
        ("zh_CN", "许可", "会话"),
        ("ko", "허가", "세션"),
        ("hi", "अनुमति", "सत्र"),
        ("ar", "السماح", "جلسة"),
        ("de", "Zulassung", "Sitzung"),
    ] {
        let dir = translated_project(language, term, session);
        let args = ["html", "-W", "-D", &format!("language={language}")];
        build(&dir.path, 0, &args);
        check_index(&dir, term, session);
        build(&dir.path, 0, &args);
        check_index(&dir, term, session);
        let path = format!("locales/{language}/LC_MESSAGES/index.po");
        dir.write(&path, &dir.read(&path).replace(session, "Revised"));
        build(&dir.path, 0, &args);
        check_index(&dir, term, "Revised");
        assert!(dir.exists(&format!("locales/{language}/LC_MESSAGES/index.mo")));
    }
}

#[test]
fn glossary_indexes_are_shared_by_sites_and_pdf_books() {
    let dir = translated_project("de", "Zulassung", "Sitzung");
    build(&dir.path, 0, &["pdf", "-W", "-D", "language=de"]);
    let typst = dir.read("_build/pdf/book-de.typ");
    let index = typst.split("#gd-index(\n").nth(1).expect("the book index");
    for (name, anchor) in [("Zulassung", "Admission"), ("Sitzung", "Session"), ("Gate", "Gate")] {
        has(index, &format!("(\"{name}\", "));
        has(index, &format!("\"index-term-{anchor}\""));
    }
    assert!(dir.exists("_build/pdf/demo-de.pdf"));
}

#[test]
fn native_and_untranslated_glossaries_keep_their_entries() {
    let native = "Native\n======\n\n.. glossary::\n\n   許可\n      A definition.\n";
    let dir = quickstart("native-glossary", &[("index.rst", native)]);
    build(&dir.path, 0, &["html", "-W", "-D", "language=ja"]);
    has(&dir.read("_build/html/genindex.html"), "許可");
    has(&dir.read("_build/html/genindex.html"), "index.html#term-0");
    has(&dir.read("_build/html/index.html"), "id=\"term-0\"");
    dir.write("index.rst", GLOSSARY);
    build(&dir.path, 0, &["html", "-W", "-D", "language=en"]);
    check_index(&dir, "Admission", "Session");
}
