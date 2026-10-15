//! Tests ported from intl_test.odin, with the catalog loading, the interface words and
//! the gettext builder besides.

use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use gd_conf::{Config, FlagOrName};
use gd_env::{BuilderKind, DocInfo, EnglishUi, Environment, ObjectStore, UiText};
use gd_gettext::{LoadOptions, Stats, load_po};
use gd_host::Budget;
use gd_i18n::{Compact, Progress};

use crate::gettext::relative_path;
use crate::*;

/// A fresh folder for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-intl-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn config_in(dir: &Path) -> Config {
    Config::new(dir.join("src"), dir.join("src").join("_build"))
}

#[test]
fn intl_settings_convert_as_sphinx_reads_them() {
    let docs = FlagOrName::Name("docs".into());
    assert_eq!(compact_mode(&docs), Compact::Single("docs".into()));
    assert_eq!(compact_mode(&FlagOrName::Flag(false)), Compact::PerDocument);
    assert_eq!(compact_mode(&FlagOrName::Flag(true)), Compact::PerDirectory);
    let untranslated = FlagOrName::Name("untranslated".into());
    assert_eq!(
        progress_classes(&untranslated),
        Progress { translated: false, untranslated: true }
    );
    assert_eq!(progress_classes(&FlagOrName::Flag(true)), Progress::ALL);
    assert_eq!(progress_classes(&FlagOrName::Flag(false)), Progress::default());
    let mut c = Config::new("/src", "/b");
    c.intl.gettext_additional_targets = vec!["index".into(), "image".into()];
    let targets = additional_targets(&c);
    assert!(targets.index && targets.image && !targets.raw);
    let e = Config::new("/src", "/b");
    assert_eq!(e.intl.locale_dirs, ["locales"]);
    assert_eq!(e.intl.figure_language_filename, "{root}.{language}{ext}");
}

#[test]
fn python_format_fills_values() {
    assert_eq!(python_format("%s %s documentation", &["Demo", "1.0"]), "Demo 1.0 documentation");
    assert_eq!(python_format("Documentation %s", &["Demo"]), "Documentation Demo");
    assert_eq!(python_format("no values", &[]), "no values");
    assert_eq!(python_format("%s and %s", &["one"]), "one and %s");
}

// A project's catalogs are created, merged, and counted as sphinx-intl does.
#[test]
fn intl_update_creates_and_merges() {
    let dir = TempDir::new("update");
    let config = config_in(&dir.0);
    let pot = dir.write("src/_build/gettext/index.pot", "msgid \"Hello\"\nmsgstr \"\"\n");
    let mut files = Vec::new();
    let missing = intl_update(&config, &IntlOptions::default(), &mut files);
    assert_eq!(missing.unwrap_err().exit, 2); // no language: en is the source language.
    let fr = IntlOptions { languages: vec!["fr".into()], ..IntlOptions::default() };
    intl_update(&config, &fr, &mut files).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].action, IntlAction::Create);
    let po = dir.0.join("src/locales/fr/LC_MESSAGES/index.po");
    assert_eq!(files[0].path, po);
    let cat = load_po(&po, &LoadOptions::default()).unwrap();
    assert_eq!(cat.header_field("Plural-Forms"), Some("nplurals=2; plural=(n > 1);"));
    assert_eq!(cat.header_field("Language"), Some("fr"));
    std::fs::write(
        &po,
        "msgid \"\"\nmsgstr \"Language: fr\\n\"\n\nmsgid \"Hello\"\nmsgstr \"Bonjour\"\n",
    )
    .unwrap();
    std::fs::write(&pot, "msgid \"Hello!\"\nmsgstr \"\"\n").unwrap();
    files.clear();
    intl_update(&config, &fr, &mut files).unwrap();
    assert_eq!(files[0].action, IntlAction::Update);
    assert_eq!(files[0].stats, Stats { fuzzy: 1, ..Stats::default() });
    files.clear();
    intl_update(&config, &fr, &mut files).unwrap();
    assert_eq!(files[0].action, IntlAction::NotChanged);
    files.clear();
    intl_stat(&config, &fr, &mut files).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].stats.fuzzy, 1);
}

#[test]
fn intl_update_needs_templates() {
    let dir = TempDir::new("no-templates");
    let config = config_in(&dir.0);
    let de = IntlOptions { languages: vec!["de".into()], ..IntlOptions::default() };
    let problem = intl_update(&config, &de, &mut Vec::new()).unwrap_err();
    assert_eq!(problem.code, "intl.templates");
    let mut files = Vec::new();
    intl_stat(&config, &de, &mut files).unwrap();
    assert!(files.is_empty());
}

#[test]
fn intl_update_reports_broken_catalogs() {
    let dir = TempDir::new("broken");
    let config = config_in(&dir.0);
    dir.write("src/_build/gettext/index.pot", "msgid \"Hello\nmsgstr \"\"\n");
    let de = IntlOptions { languages: vec!["de".into()], ..IntlOptions::default() };
    let problem = intl_update(&config, &de, &mut Vec::new()).unwrap_err();
    assert_eq!(problem.code, "i18n.catalog");
    assert!(problem.path.ends_with("index.pot:1"), "{}", problem.path);
}

#[test]
fn load_intl_reads_the_language_catalogs() {
    let dir = TempDir::new("load");
    let mut config = config_in(&dir.0);
    config.language = "de".into();
    config.intl.locale_dirs = vec!["locales".into(), "more".into()];
    config.intl.gettext_auto_build = false;
    dir.write("src/locales/de/LC_MESSAGES/index.po", "msgid \"Hello\"\nmsgstr \"Hallo\"\n");
    dir.write(
        "src/more/de/LC_MESSAGES/index.po",
        "msgid \"Hello\"\nmsgstr \"Servus\"\nmsgid \"Bye\"\nmsgstr \"Tschüss\"\n",
    );
    dir.write("src/locales/de/LC_MESSAGES/guidedog.po", "msgid \"Next\"\nmsgstr \"Vorwärts\"\n");
    dir.write("src/locales/de/LC_MESSAGES/broken.po", "msgid \"x\nmsgstr \"\"\n");
    let budget = Budget::new(gd_env::BUILD_BUDGET);
    let mut reports = Vec::new();
    let intl = load_intl(&config, BuilderKind::Html, &budget, &mut reports);
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert_eq!(reports[0].code, "i18n.catalog");
    assert!(intl.translate);
    assert_eq!(intl.domains.keys().collect::<Vec<_>>(), ["index"]);
    let catalog = intl.catalog_of("index").unwrap();
    use gd_gettext::Lookup;
    assert_eq!(catalog.text("Hello"), "Hallo"); // the first folder wins,
    assert_eq!(catalog.text("Bye"), "Tschüss"); // the others add to it.
    assert_eq!(intl.files_of("index").len(), 2);
    assert!(intl.files_of("guide/usage").is_empty());
    assert_eq!(intl.source_name("index"), "<translations>/index.po");
    assert_eq!(intl.ui.gettext("Next"), "Vorwärts");
    assert_eq!(intl.ui.gettext("Previous"), "Zurück");

    let extracting = load_intl(&config, BuilderKind::Gettext, &budget, &mut Vec::new());
    assert!(!extracting.translate && extracting.domains.is_empty());
    assert_ne!(extracting.digest, intl.digest);
    dir.write("src/locales/de/LC_MESSAGES/guide.po", "msgid \"A\"\nmsgstr \"B\"\n");
    let more = load_intl(&config, BuilderKind::Html, &budget, &mut Vec::new());
    assert_ne!(more.digest, intl.digest); // a new catalog file changes the digest.
}

#[test]
fn auto_build_compiles_mo_files() {
    let dir = TempDir::new("auto-build");
    let mut config = config_in(&dir.0);
    config.language = "de".into();
    dir.write("src/locales/de/LC_MESSAGES/index.po", "msgid \"Hello\"\nmsgstr \"Hallo\"\n");
    let budget = Budget::new(gd_env::BUILD_BUDGET);
    let mut reports = Vec::new();
    let intl = load_intl(&config, BuilderKind::Text, &budget, &mut reports);
    assert!(reports.is_empty(), "{reports:?}");
    let mo = dir.0.join("src/locales/de/LC_MESSAGES/index.mo");
    assert!(mo.is_file());
    // The .mo beside a .po is not a second catalog.
    let again = load_intl(&config, BuilderKind::Text, &budget, &mut Vec::new());
    assert_eq!(again.files_of("index").len(), 1);
    assert_eq!(again.digest, intl.digest);
}

#[test]
fn english_projects_are_not_translated() {
    let dir = TempDir::new("english");
    let config = config_in(&dir.0);
    let budget = Budget::new(gd_env::BUILD_BUDGET);
    let intl = load_intl(&config, BuilderKind::Html, &budget, &mut Vec::new());
    assert!(!intl.translate);
    assert_eq!(intl.ui.gettext("Next"), "Next");
    assert_eq!(intl.ui.ngettext("one", "many", 2), "many");
}

#[test]
fn localize_config_translates_english_defaults() {
    let mut config = Config::new("/src", "/b");
    config.language = "de".into();
    config.project = "Demo".into();
    config.release = "1.0".into();
    config.html.title_derived = true;
    config.html.short_title_derived = true;
    config.numfig_format.insert("table".into(), "Tab. %s".into());
    let (ui, _) = UiTranslator::load("de", &[], false);
    localize_config(&mut config, &ui);
    assert_eq!(config.numfig_format["figure"], ui.gettext("Fig. %s"));
    assert_ne!(config.numfig_format["figure"], "Fig. %s");
    assert_eq!(config.numfig_format["table"], "Tab. %s"); // the project's own stays.
    assert!(config.html.title.starts_with("Demo 1.0"), "{}", config.html.title);
    assert_eq!(config.html.short_title, config.html.title);
    let mut english = Config::new("/src", "/b");
    english.project = "Demo".into();
    english.html.title_derived = true;
    localize_config(&mut english, &EnglishUi);
    assert_eq!(english.html.title, "Demo documentation");
}

#[test]
fn build_dates_follow_the_formats() {
    let mut config = Config::new("/src", "/b");
    let now = UNIX_EPOCH + Duration::from_secs(1_790_000_000); // 2026-09-21
    let dates = build_dates(&config, &EnglishUi, now);
    assert_eq!(dates.date, "2026-09-21");
    assert_eq!(dates.today, "Sep 21, 2026");
    assert_eq!(dates.last_updated, "");
    config.today = "Someday".into();
    config.html.last_updated_fmt = Some("%Y".into());
    let dates = build_dates(&config, &EnglishUi, now);
    assert_eq!((dates.today.as_str(), dates.last_updated.as_str()), ("Someday", "2026"));
}

#[test]
fn relative_paths_climb_out() {
    let p = relative_path(Path::new("/p/src/_build/gettext"), Path::new("/p/src/guide/a.rst"));
    assert_eq!(p, "../../guide/a.rst");
    assert_eq!(relative_path(Path::new("/p"), Path::new("/p/a.rst")), "a.rst");
}

/// Reads `text` as reStructuredText and saves its object for `docname`.
fn saved_document(store: &ObjectStore, docname: &str, text: &str) -> DocInfo {
    let mut ws = guidedoc::Workspace::new();
    let reader = guidedoc_rst::reader();
    let name = format!("{docname}.rst");
    let request = guidedoc::default_request(&name, text, "rst", "html");
    let doc = guidedoc::read_document(&mut ws, &reader, &request).unwrap();
    let object = store.save_object(docname, doc).unwrap();
    DocInfo { docname: docname.into(), relative: name, object: Some(object), ..DocInfo::default() }
}

/// Runs the gettext builder over `env` and publishes its output.
fn build_templates(config: &Config, env: &Environment, now: u64) -> Vec<gd_report::Report> {
    let budget = Budget::new(gd_env::BUILD_BUDGET);
    let paths = gd_outputs::output_paths(config, BuilderKind::Gettext, None).unwrap();
    let store = ObjectStore::new(&paths.doctrees);
    let mut reports = Vec::new();
    let options = gd_outputs::GenerationOptions { all: false, budget: &budget };
    let mut generation = gd_outputs::Generation::begin(paths, options, &mut reports).unwrap();
    let build = gd_env::BuildOptions {
        now: Some(UNIX_EPOCH + Duration::from_secs(now)),
        ..gd_env::BuildOptions::default()
    };
    let view = gd_resolve::BuildView {
        config,
        options: &build,
        kind: BuilderKind::Gettext,
        env,
        tables: &gd_resolve::Tables::default(),
        intersphinx: &gd_resolve::Intersphinx::default(),
        objects: &store,
        registry: &guidedoc::Registry::default(),
        policy: &guidedoc::Policy::default(),
        limits: &guidedoc::Limits::default(),
        ui: &EnglishUi,
        dates: &gd_resolve::Dates::default(),
        plugins: &gd_env::PluginRun::default(),
        budget: &budget,
        roots: &[],
        settings: gd_env::Digest::default(),
    };
    use gd_resolve::OutputBuilder;
    GettextBuilder::default().write(&view, &mut generation, &mut reports).unwrap();
    let records = gd_outputs::CommitRecords { environment: env, store: &store };
    let outcome = generation.commit(records);
    assert!(outcome.problem.is_none(), "{:?}", outcome.problem);
    reports
}

#[test]
fn gettext_builder_writes_one_template_per_domain() {
    let dir = TempDir::new("gettext");
    let mut config = config_in(&dir.0);
    config.project = "Demo".into();
    config.intl.gettext_exclude_patterns = vec!["draft*".into()];
    let doctrees = gd_env::doctrees_dir(&config);
    let store = ObjectStore::new(&doctrees);
    store.prepare().unwrap();
    let env = Environment {
        docs: vec![
            saved_document(&store, "draft", "Not yet.\n"),
            saved_document(&store, "guide/usage", "Usage text.\n"),
            saved_document(&store, "index", "Hello world.\n"),
        ],
        ..Environment::default()
    };
    let reports = build_templates(&config, &env, 1_790_000_000);
    assert!(reports.is_empty(), "{reports:?}");
    let out = config.build_dir.join("gettext");
    let index = std::fs::read_to_string(out.join("index.pot")).unwrap();
    assert!(index.contains("msgid \"Hello world.\""), "{index}");
    assert!(index.contains("Project-Id-Version: Demo"), "{index}");
    assert!(index.contains("#: ../../index.rst:1"), "{index}");
    let guide = std::fs::read_to_string(out.join("guide.pot")).unwrap();
    assert!(guide.contains("msgid \"Usage text.\""), "{guide}");
    assert!(!out.join("draft.pot").exists());
    // A later build that changes only the date keeps the templates as they are.
    build_templates(&config, &env, 1_790_100_000);
    assert_eq!(std::fs::read_to_string(out.join("index.pot")).unwrap(), index);
}

// Not ported: books_are_named_after_their_language tests language_file, which moved
// to gd-book with builders.odin.
