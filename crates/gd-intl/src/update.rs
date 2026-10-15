//! `guidedog intl update` and `intl stat`, sphinx-intl's commands: update creates each
//! language's catalogs from the gettext builder's templates, or merges new templates
//! into them; stat counts what is translated.

use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_gettext::{
    Catalog, Error, LoadOptions, MergeOptions, Stats, WriteOptions, load_po, merge,
    plural_forms_for, stats, write_po,
};
use gd_host::Problem;

use crate::intl::files_with;

/// sphinx-intl's options for update and stat. Fill with struct-update syntax:
/// `IntlOptions { languages: vec!["de".into()], ..IntlOptions::default() }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntlOptions {
    /// `-p`: the templates; `None`: the gettext builder's output, `<build_dir>/gettext`.
    pub pot_dir: Option<PathBuf>,
    /// `-d`: the catalogs; `None`: the first of `locale_dirs` (`locales`).
    pub locale_dir: Option<PathBuf>,
    /// `-l`, repeatable; empty: the project's language, unless it is English.
    pub languages: Vec<String>,
    /// `-w`: the line width; 0 means 76.
    pub width: usize,
    /// Keep obsolete messages (sphinx-intl's default; `--no-obsolete` clears it).
    pub obsolete: bool,
}

impl Default for IntlOptions {
    fn default() -> IntlOptions {
        IntlOptions {
            pot_dir: None,
            locale_dir: None,
            languages: Vec::new(),
            width: 0,
            obsolete: true,
        }
    }
}

/// What [`intl_update`] did to one catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntlAction {
    /// The catalog was made from its template.
    Create,
    /// The template was merged into the catalog.
    Update,
    /// Merging changed nothing; the file was left alone (also every [`intl_stat`] file).
    NotChanged,
}

/// One catalog [`intl_update`] or [`intl_stat`] looked at, with its statistics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntlFile {
    /// What was done to it.
    pub action: IntlAction,
    /// The catalog.
    pub path: PathBuf,
    /// Its translated, fuzzy, untranslated and obsolete messages.
    pub stats: Stats,
}

/// The folders and languages of update and stat, defaults filled from the project.
struct Locations {
    pot_dir: PathBuf,
    locale_dir: PathBuf,
    languages: Vec<String>,
    width: usize,
}

/// Fills the defaults of `options` from `config`. Fails when no language is given and
/// the project's is English or unset (`intl.language`, exit 2).
fn locations(config: &Config, options: &IntlOptions) -> Result<Locations, Problem> {
    let pot_dir = options.pot_dir.clone().unwrap_or_else(|| config.build_dir.join("gettext"));
    let locale_dir = options.locale_dir.clone().unwrap_or_else(|| {
        let first = config.intl.locale_dirs.first().map_or("locales", String::as_str);
        config.source_dir.join(first)
    });
    let mut languages = options.languages.clone();
    if languages.is_empty() && !config.language.is_empty() && config.language != "en" {
        languages.push(config.language.clone());
    }
    if languages.is_empty() {
        return Err(Problem::new(
            "intl.language",
            "NO LANGUAGE GIVEN",
            "",
            "Name the languages to translate into.",
            "Write guidedog intl update -l de, or set language in conf.toml.",
            2,
        ));
    }
    let width = if options.width == 0 { 76 } else { options.width };
    Ok(Locations { pot_dir, locale_dir, languages, width })
}

/// sphinx-intl update: for each template below the templates folder and each language,
/// creates `<locale_dir>/<language>/LC_MESSAGES/<domain>.po`, or merges the template
/// into the catalog there (`gd_gettext::merge`: translations kept, changed messages
/// fuzzy, removed ones obsolete, as msgmerge does). A catalog merging leaves unchanged is
/// not rewritten.
///
/// Appends what it did to each catalog to `files`, so a caller sees the catalogs
/// written before a failure. Fails without a language (`intl.language`), without a
/// templates folder (`intl.templates`), on a catalog that cannot be read
/// (`i18n.catalog`), or one that cannot be written (`host.io`).
pub fn intl_update(
    config: &Config,
    options: &IntlOptions,
    files: &mut Vec<IntlFile>,
) -> Result<(), Problem> {
    let o = locations(config, options)?;
    if !o.pot_dir.is_dir() {
        return Err(Problem::new(
            "intl.templates",
            "NO MESSAGE TEMPLATES",
            &o.pot_dir.display().to_string(),
            "There are no .pot templates to update the translations from.",
            "Run guidedog build gettext first, or pass -p with the templates' folder.",
            2,
        ));
    }
    let mut templates = Vec::new();
    files_with(&o.pot_dir, &["pot"], &mut templates);
    templates.sort();
    let write = WriteOptions {
        width: o.width,
        previous: true,
        obsolete: options.obsolete,
        ..WriteOptions::default()
    };
    for language in &o.languages {
        for pot in &templates {
            let relative = gd_env::relative_to(&o.pot_dir, pot);
            let domain = relative.strip_suffix(".pot").unwrap_or(&relative);
            let po = o.locale_dir.join(language).join("LC_MESSAGES").join(format!("{domain}.po"));
            files.push(update_catalog(pot, &po, language, &write)?);
        }
    }
    Ok(())
}

/// Creates or merges one catalog and writes it when it changed.
fn update_catalog(
    pot: &Path,
    po: &Path,
    language: &str,
    write: &WriteOptions,
) -> Result<IntlFile, Problem> {
    let reference = load_po(pot, &LoadOptions::default()).map_err(|e| catalog_problem(&e))?;
    let (action, result, old_text) = if po.is_file() {
        let existing = load_po(po, &LoadOptions::default()).map_err(|e| catalog_problem(&e))?;
        let options = MergeOptions { fuzzy_matching: true, keep_obsolete: write.obsolete };
        let merged = merge(&existing, &reference, &options);
        (IntlAction::Update, merged, write_po(&existing, write))
    } else {
        (IntlAction::Create, new_catalog(&reference, language), String::new())
    };
    let text = write_po(&result, write);
    let mut file = IntlFile { action, path: po.to_path_buf(), stats: stats(&result) };
    if action == IntlAction::Update && text == old_text {
        file.action = IntlAction::NotChanged;
        return Ok(file);
    }
    gd_host::publish(po, text.as_bytes(), true)?;
    Ok(file)
}

/// Starts a translation from a template: its live messages, and a header (not fuzzy)
/// that names the language and its plural rule.
fn new_catalog(reference: &Catalog, language: &str) -> Catalog {
    let mut out = Catalog::new(&reference.name);
    out.header = reference.header.clone();
    if let Some(header) = &mut out.header {
        header.fuzzy = false;
    }
    for m in reference.messages.iter().filter(|m| !m.obsolete) {
        out.add_message(m.clone());
    }
    out.set_header_field("Language", language);
    out.set_header_field("Plural-Forms", plural_forms_for(language));
    out
}

/// A catalog error as a command's problem (`i18n.catalog`, exit 1).
fn catalog_problem(error: &Error) -> Problem {
    let place = if error.line == 0 {
        error.file.clone()
    } else {
        format!("{}:{}", error.file, error.line)
    };
    Problem::new(
        "i18n.catalog",
        error.title(),
        &place,
        error.message.clone(),
        error.hint.clone(),
        1,
    )
}

/// sphinx-intl stat: the translated, fuzzy and untranslated messages of every `.po`
/// catalog of the languages, in path order. Writes nothing. Appends each catalog to
/// `files`; fails without a language (`intl.language`) or on a catalog that cannot be
/// read (`i18n.catalog`).
pub fn intl_stat(
    config: &Config,
    options: &IntlOptions,
    files: &mut Vec<IntlFile>,
) -> Result<(), Problem> {
    let o = locations(config, options)?;
    for language in &o.languages {
        let mut found = Vec::new();
        files_with(&o.locale_dir.join(language).join("LC_MESSAGES"), &["po"], &mut found);
        found.sort();
        for path in found {
            let catalog =
                load_po(&path, &LoadOptions::default()).map_err(|e| catalog_problem(&e))?;
            files.push(IntlFile { action: IntlAction::NotChanged, stats: stats(&catalog), path });
        }
    }
    Ok(())
}
