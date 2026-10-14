use std::path::PathBuf;

use crate::Lookup;
use crate::{Catalog, LoadOptions, Message, Translator, language_candidates, parse_po};

const FR: &str = r#"msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"

msgid "Next"
msgstr "Suivant"

msgctxt "button"
msgid "Next"
msgstr "Continuer"

msgctxt ""
msgid "Next"
msgstr "Suivant (vide)"

#, fuzzy
msgid "Search"
msgstr "Recherche"

msgid "%d page"
msgid_plural "%d pages"
msgstr[0] "%d page"
msgstr[1] ""
"#;

#[test]
fn lookups_follow_gettext() {
    let mut cat = parse_po(FR, "fr.po").expect("FR parses");
    assert_eq!(cat.text("Next"), "Suivant");
    assert_eq!(cat.with_context("button", "Next"), "Continuer");
    assert_eq!(cat.with_context("", "Next"), "Suivant (vide)");
    assert_eq!(cat.with_context("other", "Next"), "Next");
    assert_eq!(cat.text("Missing"), "Missing");
    assert_eq!(cat.text("Search"), "Search"); // fuzzy is not used...
    cat.allow_fuzzy = true;
    assert_eq!(cat.text("Search"), "Recherche"); // ...unless allowed.
    assert_eq!(cat.plural("%d page", "%d pages", 0), "%d page");
    assert_eq!(cat.plural("%d page", "%d pages", 1), "%d page");
    // An empty form falls back to the English plural, as ngettext does.
    assert_eq!(cat.plural("%d page", "%d pages", 5), "%d pages");
    assert_eq!(cat.plural("%d apple", "%d apples", 1), "%d apple");
    assert_eq!(cat.plural("%d apple", "%d apples", 2), "%d apples");
    assert!(cat.text("").contains("Plural-Forms"));
    cat.allow_fuzzy = false;
    assert_eq!(cat.text("Search"), "Search");
}

#[test]
fn lookups_return_slices_of_the_pool() {
    let cat = parse_po(FR, "fr.po").expect("FR parses");
    let pool = cat.pool();
    let s = cat.translation("Next", Some("button")).expect("a translation");
    // The test is that s lies inside the pool, which only the addresses can show.
    let start = s.as_ptr() as usize - pool.as_ptr() as usize;
    assert!(start + s.len() <= pool.len());
    assert_eq!(&pool[start..start + s.len()], "Continuer");
    assert!(cat.translation("Search", None).is_none());
}

#[test]
fn many_messages_are_found() {
    let mut cat = Catalog::new("");
    let mut keys = Vec::new();
    for i in 0..2000 {
        let id = format!("{}{i}", "x".repeat(i % 7 + 1));
        let mut m = Message::new(&id);
        m.strs[0] = format!("T{id}");
        cat.messages.push(m);
        keys.push(id);
    }
    for k in &keys {
        assert_eq!(cat.text(k), format!("T{k}"));
    }
}

#[test]
fn added_messages_rebuild_the_index() {
    let mut cat = Catalog::new("");
    assert_eq!(cat.text("a"), "a");
    let mut m = Message::new("a");
    m.strs[0] = "b".into();
    cat.add_message(m);
    assert_eq!(cat.text("a"), "b");
}

#[test]
fn language_candidates_fall_back() {
    assert_eq!(language_candidates("pt-BR"), ["pt-BR", "pt_BR", "pt"]);
    assert_eq!(language_candidates("sr@latin").last().map(String::as_str), Some("sr"));
    assert_eq!(language_candidates("de"), ["de"]);
    assert!(language_candidates("").is_empty());
}

/// A directory under the build's target directory, removed when dropped.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn translator_chains_domains() {
    let dir = TempDir(std::env::temp_dir().join(format!("gd-gettext-{}", std::process::id())));
    let messages = dir.0.join("pt").join("LC_MESSAGES");
    std::fs::create_dir_all(&messages).expect("a temporary directory");
    let project = "msgid \"Next\"\nmsgstr \"Seguinte\"\n";
    std::fs::write(messages.join("guidedog.po"), project).expect("a temporary file");
    let mut tr = Translator::new("pt_BR");
    assert_eq!(tr.bind_text_domain("guidedog", &dir.0, &LoadOptions::default()), Ok(true));
    let builtin = parse_po(
        "msgid \"Next\"\nmsgstr \"Próximo\"\nmsgid \"Previous\"\nmsgstr \"Anterior\"\n",
        "embedded",
    )
    .expect("the built-in catalog parses");
    tr.add_catalog("guidedog", builtin);
    tr.text_domain("guidedog");
    assert_eq!(tr.text("Next"), "Seguinte"); // the project's wins...
    assert_eq!(tr.text("Previous"), "Anterior"); // ...then the built-in.
    assert_eq!(tr.domain_text("sphinx", "Next"), "Next");
    assert_eq!(tr.bind_text_domain("sphinx", &dir.0, &LoadOptions::default()), Ok(false));
}
