//! A build's translation state, prepared once before reading: the catalogs
//! documents are translated with, Guidedog's own words, the settings of the translation
//! pass, and the digest of all of it.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::{BuilderKind, Dependency, Digest, Digester, UiText};
use gd_gettext::{Catalog, language_candidates, parse_mo, parse_po_bytes};
use gd_host::{Budget, Ledger, MAX_SOURCE_BYTES};
use gd_i18n::{Compact, Progress, Targets, UI_DOMAIN, domain_for};
use gd_report::{Report, Severity};
use guidedoc::{ENGLISH_LABELS, Labels};

use crate::localize::{additional_targets, catalog_report, compact_mode, progress_classes};
use crate::ui::UiTranslator;

/// A build's translations:
///
/// - documents are translated as they are read, from the catalog of their text domain
///   in `locale_dirs/<language>/LC_MESSAGES` (gd-read runs gd-i18n's pass with
///   [`Intl::catalog_of`] and [`Intl::files_of`]);
/// - Guidedog's own words come from the `guidedog` domain ([`Intl::ui`]), for renderers
///   through `labels` and for templates through `_()` and `gettext()`.
///
/// Plain data: make one with [`load_intl`]; `Default` translates nothing.
#[derive(Debug, Default)]
pub struct Intl {
    /// Run the translation pass while reading.
    pub translate: bool,
    /// Text domain to its catalogs, merged (the first locale folder's first).
    pub domains: BTreeMap<String, Catalog>,
    /// Text domain to its catalog files and their digests: a document depends on them.
    pub files: BTreeMap<String, Vec<Dependency>>,
    /// Guidedog's words in the build's language.
    pub ui: UiTranslator,
    /// The renderer labels, translated.
    pub labels: Labels,
    /// `gettext_additional_targets`.
    pub targets: Targets,
    /// `translation_progress_classes`.
    pub progress: Progress,
    /// `gettext_compact`.
    pub compact: Compact,
    /// The digest of everything above (incremental.rst, "Translation digest"), for
    /// `gd_env::build_settings` and the page digests.
    pub digest: Digest,
}

impl Intl {
    /// The text domain of a document's messages (`gettext_compact`), borrowed.
    pub fn domain_of<'a>(&'a self, docname: &'a str) -> &'a str {
        domain_for(docname, &self.compact)
    }

    /// The catalog a document is translated with, if its domain has one.
    pub fn catalog_of(&self, docname: &str) -> Option<&Catalog> {
        self.domains.get(self.domain_of(docname))
    }

    /// The catalog files a document's translation depends on; empty without any.
    pub fn files_of(&self, docname: &str) -> &[Dependency] {
        self.files.get(self.domain_of(docname)).map_or(&[], Vec::as_slice)
    }

    /// The name translations of a document are parsed under, e.g. `<translations>/guide.po`.
    pub fn source_name(&self, docname: &str) -> String {
        format!("<translations>/{}.po", self.domain_of(docname))
    }
}

/// Loads the translations of `config`'s language for a build by `kind`: Guidedog's
/// words always; the documents' catalogs unless the builder is `gettext` (which
/// extracts the English messages) or no language is set. Catalog files are read within
/// `budget`. A catalog that cannot be read or parsed is left out with a warning in
/// `reports`; nothing fails.
pub fn load_intl(
    config: &Config,
    kind: BuilderKind,
    budget: &Budget,
    reports: &mut Vec<Report>,
) -> Intl {
    let c = &config.intl;
    let dirs = locale_paths(config);
    let dir_refs: Vec<&Path> = dirs.iter().map(PathBuf::as_path).collect();
    let (ui, problems) =
        UiTranslator::load(&config.language, &dir_refs, c.gettext_allow_fuzzy_translations);
    reports.extend(problems.iter().map(|p| catalog_report(&p.error, Severity::Warning)));
    let mut intl = Intl {
        labels: ui.labels(),
        ui,
        targets: additional_targets(config),
        progress: progress_classes(&c.translation_progress_classes),
        compact: compact_mode(&c.gettext_compact),
        ..Intl::default()
    };
    if kind != BuilderKind::Gettext && !config.language.is_empty() {
        let mut loader = DomainLoader { config, budget, reports, intl: &mut intl };
        for dir in &dirs {
            loader.load_folder(dir);
        }
        intl.translate = !intl.domains.is_empty() || config.language != "en";
    }
    intl.digest = intl_digest(&intl, config, &dirs);
    intl
}

/// `locale_dirs` made absolute against the source folder.
pub fn locale_paths(config: &Config) -> Vec<PathBuf> {
    config.intl.locale_dirs.iter().map(|d| config.source_dir.join(d)).collect()
}

/// The translation digest: which catalogs exist, not what they hold, since a document
/// depends on its own catalog through its dependencies.
fn intl_digest(intl: &Intl, config: &Config, dirs: &[PathBuf]) -> Digest {
    let c = &config.intl;
    let mut d = Digester::new();
    d.flag(intl.translate).text(&config.language);
    for dir in dirs {
        d.text(&dir.display().to_string());
    }
    d.text(&format!("{:?}", intl.targets)).text(&format!("{:?}", intl.progress));
    d.flag(c.gettext_allow_fuzzy_translations).text(&c.figure_language_filename);
    d.text(&format!("{:?}", intl.compact));
    for english in ENGLISH_LABELS {
        d.text(&intl.ui.gettext(english));
    }
    for (domain, files) in &intl.files {
        for f in files {
            d.text(domain).text(&f.path.display().to_string());
        }
    }
    d.finish()
}

/// Reads a build's document catalogs into an [`Intl`].
struct DomainLoader<'a> {
    config: &'a Config,
    budget: &'a Budget,
    reports: &'a mut Vec<Report>,
    intl: &'a mut Intl,
}

impl DomainLoader<'_> {
    /// Loads every catalog for the language below one locale folder:
    /// `<dir>/<language>/LC_MESSAGES/<domain>.po`, or `.mo` without a `.po`. Guidedog's
    /// and Sphinx's own domains are for the interface, not documents.
    fn load_folder(&mut self, dir: &Path) {
        for language in language_candidates(&self.config.language) {
            let base = dir.join(language).join("LC_MESSAGES");
            if !base.is_dir() {
                continue;
            }
            let mut files = Vec::new();
            catalog_files(&base, &mut files);
            files.sort();
            for path in files {
                let domain = domain_name(&base, &path);
                if domain == UI_DOMAIN || domain == "sphinx" {
                    continue;
                }
                if path.extension().is_some_and(|e| e == "mo")
                    && path.with_extension("po").is_file()
                {
                    continue;
                }
                self.add_catalog(domain, &path);
            }
        }
    }

    /// Reads, parses and keeps one catalog; one of a known domain merges into it.
    fn add_catalog(&mut self, domain: String, path: &Path) {
        let allow_fuzzy = self.config.intl.gettext_allow_fuzzy_translations;
        // The file is held only while it is parsed.
        let ledger = Ledger::new(self.budget);
        let data = match gd_host::read_file(path, Some(&ledger), MAX_SOURCE_BYTES) {
            Ok(data) => data,
            Err(problem) => return self.reports.push(problem.report()),
        };
        let name = path.display().to_string();
        let is_mo = path.extension().is_some_and(|e| e == "mo");
        let parsed = if is_mo { parse_mo(&data, &name) } else { parse_po_bytes(&data, &name) };
        let mut catalog = match parsed {
            Ok(catalog) => catalog,
            Err(e) => return self.reports.push(catalog_report(&e, Severity::Warning)),
        };
        catalog.allow_fuzzy = allow_fuzzy;
        if self.config.intl.gettext_auto_build && !is_mo {
            compile_mo(&catalog, path, self.reports);
        }
        let file = Dependency {
            path: path.to_path_buf(),
            digest: Digest::of_parts(&[&data]),
            included: false,
        };
        self.intl.files.entry(domain.clone()).or_default().push(file);
        let stored = match self.intl.domains.entry(domain) {
            Entry::Occupied(first) => {
                let first = first.into_mut();
                merge_catalog(first, catalog);
                first
            }
            Entry::Vacant(slot) => slot.insert(catalog),
        };
        stored.build_index(); // threads only read it from now on.
    }
}

/// The text domain of a catalog file: its path below `base` without the extension.
fn domain_name(base: &Path, path: &Path) -> String {
    let relative = gd_env::relative_to(base, path);
    let stem = relative.strip_suffix(".po").or_else(|| relative.strip_suffix(".mo"));
    stem.unwrap_or(&relative).to_string()
}

/// Collects the `.po` and `.mo` files below `dir`; an unreadable folder adds nothing.
pub(crate) fn catalog_files(dir: &Path, out: &mut Vec<PathBuf>) {
    files_with(dir, &["po", "mo"], out);
}

/// Collects the files below `dir` whose extension is one of `extensions`.
pub(crate) fn files_with(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            files_with(&path, extensions, out);
        } else if path.extension().is_some_and(|e| extensions.iter().any(|x| e == *x)) {
            out.push(path);
        }
    }
}

/// Adds the messages of `more` that `into` lacks (by context and msgid); obsolete ones
/// are left out.
fn merge_catalog(into: &mut Catalog, more: Catalog) {
    let known: std::collections::HashSet<(Option<String>, String)> =
        into.messages.iter().map(|m| (m.ctx.clone(), m.id.clone())).collect();
    for m in more.messages {
        if !m.obsolete && !known.contains(&(m.ctx.clone(), m.id.clone())) {
            into.add_message(m);
        }
    }
}

/// Writes the `.mo` beside a `.po` when it is missing or older, as Sphinx does with
/// `gettext_auto_build`, for tools that read compiled catalogs. A failure is a warning.
fn compile_mo(catalog: &Catalog, po: &Path, reports: &mut Vec<Report>) {
    let mo = po.with_extension("mo");
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if let (Some(po_time), Some(mo_time)) = (modified(po), modified(&mo))
        && mo_time >= po_time
    {
        return;
    }
    let written = match gd_gettext::write_mo(catalog, catalog.allow_fuzzy) {
        Ok(bytes) => gd_host::publish(&mo, &bytes, true).map_err(|p| p.report()),
        Err(e) => Err(catalog_report(&e, Severity::Warning)),
    };
    if let Err(mut report) = written {
        report.severity = Severity::Warning;
        reports.push(report);
    }
}
