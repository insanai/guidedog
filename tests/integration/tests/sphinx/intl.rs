//! Translation end to end: gettext extraction, the sphinx-intl workflow (`guidedog intl`)
//! to a translated site and book, and project catalogs over Guidedog's own words.

use guidedog_tests::{guidedog, has};

use crate::{build, quickstart};

/// Extraction can leave reference appendices out without removing them from the site.
#[test]
fn gettext_exclusions_preserve_the_published_documents() {
    let dir = quickstart(
        "intl-exclude",
        &[
            ("conf.toml", "project = \"Demo\"\ngettext_exclude_patterns = [\"api/**\"]\n"),
            ("index.rst", "Manual\n======\n\n.. toctree::\n\n   api/nested/reference\n"),
            ("api/nested/reference.rst", "Reference\n=========\n\nEnglish API.\n"),
        ],
    );
    build(&dir.path, 0, &["gettext", "-W"]);
    assert!(dir.exists("_build/gettext/index.pot"));
    assert!(!dir.exists("_build/gettext/api.pot"));
    build(&dir.path, 0, &["html", "-W"]);
    has(&dir.read("_build/html/api/nested/reference.html"), "English API.");
    has(&dir.read("_build/html/index.html"), "api/nested/reference.html");
}

/// Fills three messages of the German catalog `po` holds.
fn translate(po: &str) -> String {
    let mut text = po.to_owned();
    for (id, translation) in [
        ("Welcome", "Willkommen"),
        ("Hello *world*, see the site_.", "Hallo *Welt*, siehe site_."),
        ("Details", "Einzelheiten"),
    ] {
        let empty = format!("msgid \"{id}\"\nmsgstr \"\"");
        assert!(text.contains(&empty), "no {id:?} in:\n{text}");
        text = text.replacen(&empty, &format!("msgid \"{id}\"\nmsgstr \"{translation}\""), 1);
    }
    text
}

/// The translation workflow of Sphinx and sphinx-intl: write the message templates,
/// create a German catalog, translate it, build the German site and book; a changed
/// source turns its message fuzzy on the next update.
#[test]
fn translation_workflow_builds_a_translated_site() {
    let index = "Welcome\n=======\n\n.. toctree::\n\n   guide/usage\n\n\
        Hello *world*, see the site_.\n\n.. _site: https://example.org\n\n\
        .. note:: Mind the gap.\n\nDetails\n-------\n\nBack to Details_.\n";
    let dir = quickstart(
        "intl-workflow",
        &[("index.rst", index), ("guide/usage.rst", "Usage\n=====\n\nRun it.\n")],
    );
    build(&dir.path, 0, &["gettext"]);
    let pot = dir.read("_build/gettext/index.pot");
    has(&pot, "#: ../../index.rst:1\nmsgid \"Welcome\"\n");
    has(&pot, "msgid \"Hello *world*, see the site_.\"\n");
    assert!(dir.exists("_build/gettext/guide.pot"));
    let r = guidedog(&dir.path, 0, &["intl", "update", "-l", "de", "--color=never"]);
    has(&r.out, "create");
    let po = "locales/de/LC_MESSAGES/index.po";
    has(&dir.read(po), "\"Language: de\\n\"");
    dir.write(po, &translate(&dir.read(po)));
    let r = guidedog(&dir.path, 0, &["intl", "stat", "-l", "de", "--color=never"]);
    has(&r.out, "3 translated, 0 fuzzy, 2 untranslated.");
    build(&dir.path, 0, &["html", "-W", "-D", "language=de"]);
    // Each language has its own book, named after it.
    build(&dir.path, 0, &["pdf", "-D", "language=de"]);
    assert!(dir.exists("_build/pdf/demo-de.pdf"));
    assert!(!dir.exists("_build/pdf/demo.pdf"));
    let page = dir.read("_build/html/index.html");
    for part in [
        "<html lang=\"de\">",
        "Willkommen",
        "<p>Hallo <em>Welt</em>, siehe <a href=\"https://example.org\">site</a>.</p>",
        // Guidedog's own words, in the page and in the layout.
        "<p class=\"admonition-title\">Anmerkung</p>",
        "<span class=\"gd-sr\">Navigation</span>",
        "Zum Inhalt springen",
        // Anchors do not change with the language.
        "id=\"details\"",
        "href=\"#details\"",
        "Demo Dokumentation",
    ] {
        has(&page, part);
    }
    has(&dir.read("_build/html/search.html"), "<h1 id=\"search-documentation\">Suche</h1>");
    assert!(dir.exists("locales/de/LC_MESSAGES/index.mo"));
    dir.write("index.rst", &format!("{}\nOne more paragraph.\n", dir.read("index.rst")));
    build(&dir.path, 0, &["gettext"]);
    let r = guidedog(&dir.path, 0, &["intl", "update", "-l", "de", "--color=never"]);
    has(&r.out, "update");
    let po = dir.read(po);
    has(&po, "msgid \"One more paragraph.\"");
    has(&po, "msgstr \"Willkommen\"");
}

/// Project catalogs override Guidedog's built-in words; an English build is untouched.
#[test]
fn project_catalogs_override_interface_words() {
    let dir = quickstart(
        "intl-words",
        &[
            ("conf.toml", "project = \"Demo\"\nlanguage = \"fr\"\n"),
            ("index.rst", "Demo\n====\n\n.. warning:: Careful.\n\n.. versionadded:: 2.0\n"),
            ("locales/fr/LC_MESSAGES/sphinx.po", "msgid \"Warning\"\nmsgstr \"Attention !\"\n"),
        ],
    );
    build(&dir.path, 0, &["html", "-W"]);
    let page = dir.read("_build/html/index.html");
    has(&page, "<p class=\"admonition-title\">Attention !</p>");
    has(&page, "Ajouté dans la version 2.0.");
    has(&page, "Documentation Demo");
    build(&dir.path, 0, &["html", "-W", "-D", "language=en"]);
    let english = dir.read("_build/html/index.html");
    has(&english, "<p class=\"admonition-title\">Warning</p>");
    has(&english, "Added in version 2.0.");
}
