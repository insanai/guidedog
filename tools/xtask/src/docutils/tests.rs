use super::*;

#[test]
fn corpus_lines_name_a_source_and_its_settings() {
    let manifest = "# comment\n\ndocs/a.rst\nextra/b.rst smartquotes lang=de\n";
    let entries: Vec<_> = corpus_entries(manifest).collect();
    assert_eq!(entries, [("docs/a.rst", vec![]), ("extra/b.rst", vec!["smartquotes", "lang=de"])]);
}

#[test]
fn settings_become_docutils_options() {
    let options = docutils_options(&["smartquotes", "lang=de", "tab=4", "messages=total"], false);
    let expected = [
        "--writer=xml",
        "--no-xml-declaration",
        "--no-doctype",
        "--syntax-highlight=none",
        "--smart-quotes=yes",
        "--language=de",
        "--tab-width=4",
    ];
    assert_eq!(options.as_deref(), Ok(&expected.map(String::from)[..]));
    let ids = docutils_options(&[], true).unwrap_or_default();
    assert_eq!(ids[..2], ["--writer=html5", "--auto-id-prefix=id"]);
    assert!(docutils_options(&["smartqoutes"], false).is_err());
}

#[test]
fn ids_of_pages_and_of_their_system_messages() {
    let html = concat!(
        "<main id=\"top\"><section id=\"b a\"><p>x</p></section>",
        "<aside class=\"system-message\" id=\"msg1\"><p id=\"in\">y</p></aside></main>",
    );
    let (all, messages) = document_ids(html);
    assert_eq!(all, " a b in msg1 top");
    assert_eq!(messages, " msg1");
}

#[test]
fn ids_header_names_the_task() {
    assert!(IDS_HEADER.ends_with("# cargo xtask rstref --ids.\n"));
    assert_eq!(IDS_HEADER.lines().count(), 3);
}
