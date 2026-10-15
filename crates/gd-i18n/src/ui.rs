//! Guidedog's own words (admonition titles, "Next", "Fig. %s", ...) in the text domain
//! [`UI_DOMAIN`]. Their msgids are Sphinx's English strings where Sphinx has the same
//! string, so a project's `sphinx.po` keeps working. A translator for them chains, first
//! match wins:
//!
//! 1. the project's `<locale_dir>/<language>/LC_MESSAGES/guidedog.po` (or `.mo`),
//! 2. the project's `sphinx.po` (or `.mo`) in the same places, as Sphinx reads it,
//! 3. the catalog built into Guidedog for the language (`locale/`).

use std::path::{Path, PathBuf};

use gd_gettext::{
    Error, LoadOptions, MoCatalog, Translator, find_catalog_file, language_candidates, load_po,
    parse_po,
};
use guidedoc::{ENGLISH_LABELS, LABEL_COUNT, Label, Labels};

/// The text domain of Guidedog's own words.
pub const UI_DOMAIN: &str = "guidedog";

/// The catalogs built into the binary, by language.
const EMBEDDED: [(&str, &str); 7] = [
    ("de", include_str!("../locale/de/LC_MESSAGES/guidedog.po")),
    ("es", include_str!("../locale/es/LC_MESSAGES/guidedog.po")),
    ("fr", include_str!("../locale/fr/LC_MESSAGES/guidedog.po")),
    ("ja", include_str!("../locale/ja/LC_MESSAGES/guidedog.po")),
    ("pt_BR", include_str!("../locale/pt_BR/LC_MESSAGES/guidedog.po")),
    ("ru", include_str!("../locale/ru/LC_MESSAGES/guidedog.po")),
    ("zh_CN", include_str!("../locale/zh_CN/LC_MESSAGES/guidedog.po")),
];

/// The template of the domain: every msgid Guidedog looks up.
pub const UI_TEMPLATE: &str = include_str!("../locale/guidedog.pot");

/// Every renderer label, in declaration order (`guidedoc::Label` has no list of its own).
const ALL_LABELS: [Label; LABEL_COUNT] = [
    Label::Attention,
    Label::Caution,
    Label::Danger,
    Label::Error,
    Label::Hint,
    Label::Important,
    Label::Note,
    Label::Tip,
    Label::Warning,
    Label::SeeAlso,
    Label::Todo,
    Label::Contents,
    Label::VersionAdded,
    Label::VersionChanged,
    Label::VersionDeprecated,
    Label::VersionRemoved,
    Label::SectionAuthor,
    Label::ModuleAuthor,
    Label::CodeAuthor,
    Label::LinkHeading,
    Label::LinkDefinition,
    Label::LinkImage,
    Label::LinkTable,
    Label::LinkCode,
    Label::LinkEquation,
    Label::LinkTerm,
    Label::Parameters,
    Label::Variables,
    Label::Raises,
    Label::Returns,
    Label::ReturnType,
    Label::ReturnValues,
    Label::TemplateParameters,
    Label::Throws,
    Label::Arguments,
    Label::Availability,
];

/// Returns the built-in catalog text for a language, trying it, then its base language
/// (pt-BR, pt_BR, then pt); `None` when Guidedog has none. The text is static.
pub fn embedded_catalog(language: &str) -> Option<&'static str> {
    language_candidates(language)
        .iter()
        .find_map(|c| EMBEDDED.iter().find(|(l, _)| l == c).map(|&(_, text)| text))
}

/// A project catalog that could not be read; its error renders an Elm-style report.
#[derive(Debug)]
pub struct UiProblem {
    /// The catalog's path.
    pub path: PathBuf,
    /// Why it could not be read.
    pub error: Error,
}

/// Makes the translator of Guidedog's own words for `language`, with `dirs` the
/// project's locale directories. A catalog that fails to load is left out and returned
/// among the problems; the others still apply. Fuzzy translations are used when
/// `allow_fuzzy` is set (`gettext_allow_fuzzy_translations`).
pub fn ui_translator(
    language: &str,
    dirs: &[&Path],
    allow_fuzzy: bool,
) -> (Translator, Vec<UiProblem>) {
    let mut tr = Translator::new(language);
    tr.text_domain(UI_DOMAIN);
    let mut problems = Vec::new();
    let options = LoadOptions { allow_fuzzy, ..LoadOptions::default() };
    for domain in [UI_DOMAIN, "sphinx"] {
        for dir in dirs {
            let Some(path) = find_catalog_file(dir, language, domain) else {
                continue;
            };
            if let Err(error) = bind_catalog(&mut tr, &path, &options) {
                problems.push(UiProblem { path, error });
            }
        }
    }
    if let Some(text) = embedded_catalog(language) {
        let name = format!("<guidedog>/{language}/guidedog.po");
        // The built-in catalogs are tested to parse; a failure would only lose words.
        if let Ok(mut cat) = parse_po(text, &name) {
            cat.allow_fuzzy = allow_fuzzy;
            tr.add_catalog(UI_DOMAIN, cat);
        }
    }
    (tr, problems)
}

/// Loads a `.po` or `.mo` file and binds it to the UI domain.
fn bind_catalog(tr: &mut Translator, path: &Path, options: &LoadOptions) -> Result<(), Error> {
    if path.extension().is_some_and(|e| e == "mo") {
        tr.add_catalog(UI_DOMAIN, MoCatalog::load(path, options)?);
    } else {
        tr.add_catalog(UI_DOMAIN, load_po(path, options)?);
    }
    Ok(())
}

/// Translates one of Guidedog's words; without a translator, the word itself.
pub fn ui_text<'a>(tr: Option<&'a Translator>, msgid: &'a str) -> &'a str {
    match tr {
        Some(tr) => tr.domain_text(UI_DOMAIN, msgid),
        None => msgid,
    }
}

/// Returns every renderer label translated, for `RenderOptions::labels`.
pub fn fill_labels(tr: &Translator) -> Labels {
    let mut labels = Labels::default();
    for label in ALL_LABELS {
        labels.set(label, ui_text(Some(tr), ENGLISH_LABELS[label as usize]));
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;
    use gd_gettext::Lookup;

    #[test]
    fn labels_are_listed_in_order() {
        for (i, label) in ALL_LABELS.into_iter().enumerate() {
            assert_eq!(label as usize, i);
        }
    }

    // Every built-in catalog parses and translates every msgid of the template.
    #[test]
    fn embedded_catalogs_are_complete() {
        let template = parse_po(UI_TEMPLATE, "guidedog.pot").expect("the template parses");
        for english in ENGLISH_LABELS {
            assert!(
                template.messages.iter().any(|m| m.id == english),
                "the template lacks the label {english:?}"
            );
        }
        for (language, text) in EMBEDDED {
            let cat = parse_po(text, language).unwrap_or_else(|e| panic!("{language}: {e}"));
            for m in &template.messages {
                let s = cat.translation(&m.id, None);
                assert!(s.is_some_and(|s| !s.is_empty()), "{language} lacks {:?}", m.id);
                let s = s.unwrap_or_default();
                assert_eq!(s.matches("%s").count(), m.id.matches("%s").count(), "{language}");
            }
        }
    }

    #[test]
    fn ui_translator_prefers_the_project() {
        let dir = std::env::temp_dir().join(format!("gd-i18n-ui-{}", std::process::id()));
        let messages = dir.join("de").join("LC_MESSAGES");
        std::fs::create_dir_all(&messages).expect("make the locale directory");
        let write = |name: &str, text: &str| {
            std::fs::write(messages.join(name), text).expect("write a catalog");
        };
        write("guidedog.po", "msgid \"Next\"\nmsgstr \"Vorwärts\"\n");
        write(
            "sphinx.po",
            "msgid \"Next\"\nmsgstr \"Nächste\"\nmsgid \"Warning\"\nmsgstr \"Obacht\"\n",
        );
        let (tr, problems) = ui_translator("de", &[dir.as_path()], false);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(ui_text(Some(&tr), "Next"), "Vorwärts"); // guidedog.po first,
        assert_eq!(ui_text(Some(&tr), "Warning"), "Obacht"); // then sphinx.po,
        assert_eq!(ui_text(Some(&tr), "Previous"), "Zurück"); // then the built-in one.
        assert_eq!(ui_text(Some(&tr), "Unknown words"), "Unknown words");
        let labels = fill_labels(&tr);
        assert_eq!(labels.get(Label::Note), "Anmerkung");
        assert_eq!(labels.get(Label::VersionAdded), "Hinzugefügt in Version %s");
        let (pt, _) = ui_translator("pt-BR", &[], false);
        assert_eq!(ui_text(Some(&pt), "Search"), "Pesquisar");
        let (xx, _) = ui_translator("xx", &[], false);
        assert_eq!(ui_text(Some(&xx), "Search"), "Search");
        assert_eq!(ui_text(None, "Search"), "Search");
    }

    #[test]
    fn broken_project_catalogs_are_problems() {
        let dir = std::env::temp_dir().join(format!("gd-i18n-bad-{}", std::process::id()));
        let messages = dir.join("fr").join("LC_MESSAGES");
        std::fs::create_dir_all(&messages).expect("make the locale directory");
        std::fs::write(messages.join("guidedog.po"), "msgid \"Next\nmsgstr \"x\"\n")
            .expect("write a catalog");
        let (tr, problems) = ui_translator("fr", &[dir.as_path()], false);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].path.ends_with("guidedog.po"));
        assert_eq!(ui_text(Some(&tr), "Warning"), "Avertissement");
    }
}
