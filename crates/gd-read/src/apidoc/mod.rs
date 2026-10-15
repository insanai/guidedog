//! sphinx.ext.apidoc. Each `apidoc_modules` entry names a package folder and a document
//! folder; the package is walked as `sphinx-apidoc` walks it and its pages (a page per
//! package, per module with `separate_modules`, and `modules` as the table of
//! contents) become generated documents with the `automodule` directives
//! `sphinx-apidoc` writes. Nothing is written to the source folder.

mod walk;

pub(crate) use walk::fnmatch;

use std::path::{Path, PathBuf};

use gd_conf::{ApidocModule, Config};
use gd_env::SourceFile;
use gd_jinja::{Environment, Loader, Options, Value};
use gd_report::Report;

use crate::api::generated_source;

/// The most modules one entry's walk visits.
const MAX_MODULES: usize = 10_000;
/// How deeply a walk descends into folders.
const MAX_FOLDER_DEPTH: usize = 64;

/// Sphinx's apidoc templates.
const TEMPLATES: &[(&str, &str)] = &[
    ("module.rst.jinja", include_str!("templates/module.rst.jinja")),
    ("package.rst.jinja", include_str!("templates/package.rst.jinja")),
    ("toc.rst.jinja", include_str!("templates/toc.rst.jinja")),
];

/// Whether sphinx.ext.apidoc is on.
fn is_on(c: &Config) -> bool {
    c.has_extension("sphinx.ext.apidoc")
}

/// One entry's settings, its own where it sets them, the general ones otherwise.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    /// The package folder, absolute and cleaned.
    pub(crate) path: PathBuf,
    /// The document folder, relative to the source folder.
    pub(crate) destination: String,
    /// The exclude patterns, absolute (relative to conf.toml's folder).
    pub(crate) excludes: Vec<String>,
    pub(crate) max_depth: i64,
    pub(crate) follow_links: bool,
    pub(crate) separate_modules: bool,
    pub(crate) include_private: bool,
    pub(crate) no_headings: bool,
    pub(crate) module_first: bool,
    pub(crate) implicit_namespaces: bool,
    pub(crate) automodule_options: Vec<String>,
}

impl Settings {
    fn of(c: &Config, m: &ApidocModule) -> Settings {
        let a = &c.ext.apidoc;
        let in_conf = |p: &str| gd_conf::clean_path(&c.config_dir.join(p));
        let excludes = m.exclude_patterns.as_ref().unwrap_or(&a.exclude_patterns);
        Settings {
            path: in_conf(&m.path),
            destination: m.destination.trim_matches('/').to_owned(),
            excludes: excludes.iter().map(|p| in_conf(p).to_string_lossy().into_owned()).collect(),
            max_depth: m.max_depth.unwrap_or(a.max_depth),
            follow_links: m.follow_links.unwrap_or(a.follow_links),
            separate_modules: m.separate_modules.unwrap_or(a.separate_modules),
            include_private: m.include_private.unwrap_or(a.include_private),
            no_headings: m.no_headings.unwrap_or(a.no_headings),
            module_first: m.module_first.unwrap_or(a.module_first),
            implicit_namespaces: m.implicit_namespaces.unwrap_or(a.implicit_namespaces),
            automodule_options: m
                .automodule_options
                .clone()
                .unwrap_or_else(|| a.automodule_options.clone()),
        }
    }
}

/// Whether a folder is a regular package: it holds an `__init__` module.
pub(crate) fn is_package_dir(dir: &Path) -> bool {
    walk::INIT_FILES.iter().any(|f| dir.join(f).is_file())
}

/// The folders autodoc must read for the packages of `apidoc_modules`: the folder
/// above a package (so `import package` finds it), or a folder of top-level modules.
pub(crate) fn package_roots(c: &Config) -> Vec<PathBuf> {
    if !is_on(c) {
        return Vec::new();
    }
    let mut roots = Vec::new();
    for m in &c.ext.apidoc.modules {
        let s = Settings::of(c, m);
        let root = if is_package_dir(&s.path) || s.implicit_namespaces {
            s.path.parent().map_or_else(|| s.path.clone(), Path::to_path_buf)
        } else {
            s.path.clone()
        };
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

/// The pages of `apidoc_modules`, as generated documents. A page whose name a document
/// of `taken` already has is left out (`apidoc.exists`); a missing package folder is
/// `apidoc.path`, a walk cut short `apidoc.limit`.
pub(crate) fn apidoc_sources(
    c: &Config,
    taken: &[SourceFile],
    reports: &mut Vec<Report>,
) -> Vec<SourceFile> {
    if !is_on(c) {
        return Vec::new();
    }
    let env = templates();
    let mut out: Vec<SourceFile> = Vec::new();
    for m in &c.ext.apidoc.modules {
        let s = Settings::of(c, m);
        if !s.path.is_dir() {
            reports.push(missing_path(&s.path));
            continue;
        }
        let pages = walk::package_pages(&s, &env, reports);
        for (name, text) in pages {
            let docname =
                if s.destination.is_empty() { name } else { format!("{}/{name}", s.destination) };
            if taken.iter().any(|t| t.docname == docname) {
                reports.push(exists(&docname));
                continue;
            }
            if out.iter().any(|o| o.docname == docname) {
                continue;
            }
            out.push(generated_source(c, &docname, text, Vec::new()));
        }
    }
    out
}

/// The apidoc templates, with Sphinx's `e` and `heading` filters.
fn templates() -> Environment {
    let loader = Loader::custom(|name| {
        TEMPLATES.iter().find(|(n, _)| *n == name).map(|(_, text)| (*text).to_owned())
    });
    let mut env = Environment::new(Options { loader: Some(loader), ..Options::default() });
    let escape = |call: &mut gd_jinja::Call<'_, '_>| {
        Ok(Value::from(rst_escape(&gd_jinja::to_str(&call.args[0]))))
    };
    env.add_filter("e", escape);
    env.add_filter("escape", escape);
    env.add_filter("heading", |call| {
        let text = gd_jinja::to_str(&call.args[0]);
        let level = call.arg_count(1, "level", 1)?;
        let mark = match level {
            1 => '=',
            2 => '-',
            _ => '~',
        };
        let rule: String = std::iter::repeat_n(mark, text.chars().count()).collect();
        Ok(Value::from(format!("{text}\n{rule}")))
    });
    env
}

/// Sphinx's `rst.escape`: ASCII punctuation but `.` escaped, and a leading `.`.
pub(crate) fn rst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for (i, c) in s.chars().enumerate() {
        if (c.is_ascii_punctuation() && c != '.') || (i == 0 && c == '.') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn missing_path(path: &Path) -> Report {
    let message = format!(
        "apidoc_modules names the package folder {}, which does not exist; it has no pages.",
        path.display()
    );
    let report = Report::warning("apidoc.path", "APIDOC PACKAGE FOLDER MISSING", &message).hint(
        "Give the path of the package folder relative to conf.toml, such as \"../src/mypackage\".",
    );
    Report { path: gd_conf::CONFIG_FILE.to_owned(), category: "apidoc.path".to_owned(), ..report }
}

fn exists(docname: &str) -> Report {
    let message = format!(
        "The source folder has a document named {docname}, the name of a page apidoc would \
         generate; the document is kept."
    );
    let report = Report::note("apidoc.exists", "APIDOC PAGE WRITTEN BY HAND", &message)
        .hint("Nothing to do, unless the page should be generated: then remove the file.");
    Report { category: "apidoc.exists".to_owned(), ..report }
}

/// `apidoc.limit`: a walk stopped at `what`.
pub(crate) fn limit(path: &Path, what: &str) -> Report {
    let message =
        format!("apidoc stopped walking {} at {what}; the rest has no pages.", path.display());
    let report = Report::warning("apidoc.limit", "APIDOC WALK CUT SHORT", &message)
        .hint("Exclude the parts that need no pages with exclude_patterns.");
    Report { category: "apidoc.limit".to_owned(), ..report }
}

#[cfg(test)]
mod tests;
