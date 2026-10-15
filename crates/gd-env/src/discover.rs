//! Discovering a project's documents: `discover` lists the source files, `glob_match`
//! implements Sphinx's patterns, and `contents_root` keeps pre-2.0 projects building.

use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_host::Problem;
use gd_report::{Report, Severity};

use crate::environment::Dependency;

/// The most documents one project may have.
pub const MAX_DOCUMENTS: usize = 100_000;

/// The markup of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SourceKind {
    /// reStructuredText.
    #[default]
    Rst,
    /// CommonMark and MyST Markdown.
    Markdown,
}

impl SourceKind {
    /// The kind a source suffix reads as: `.md` and `.markdown` are Markdown, any other
    /// suffix reStructuredText.
    pub fn of_suffix(suffix: &str) -> SourceKind {
        if suffix.eq_ignore_ascii_case(".md") || suffix.eq_ignore_ascii_case(".markdown") {
            SourceKind::Markdown
        } else {
            SourceKind::Rst
        }
    }

    /// The name saved in the environment and given to plugins: `"rst"` or `"markdown"`.
    pub fn name(self) -> &'static str {
        match self {
            SourceKind::Rst => "rst",
            SourceKind::Markdown => "markdown",
        }
    }

    /// The kind `name` names, as [`SourceKind::name`] writes it.
    pub fn from_name(name: &str) -> Option<SourceKind> {
        match name {
            "rst" => Some(SourceKind::Rst),
            "markdown" => Some(SourceKind::Markdown),
            _ => None,
        }
    }
}

/// A document the build makes rather than reads (a generated API page): its text, and
/// the files it was made from, whose digests decide when it is read again.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Generated {
    /// The document's text.
    pub text: String,
    /// The files the text was made from.
    pub deps: Vec<Dependency>,
}

/// One document of the project.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFile {
    /// Its name: the path below the source folder without the suffix, with `/`
    /// separators, as in Sphinx.
    pub docname: String,
    /// The file (absolute); for a generated page, where it would be.
    pub path: PathBuf,
    /// The path below the source folder with its suffix and `/` separators.
    pub relative: String,
    /// Its markup.
    pub kind: SourceKind,
    /// The page's text, when the build generates it instead of reading a file.
    pub generated: Option<Generated>,
}

/// Lists the project's documents, sorted by docname: the files below the source folder
/// with a `source_suffix`, not excluded (`exclude_patterns`, the build folder, template
/// and static folders, hidden names) and included (`include_patterns`). A document that
/// is a link must lead into the readable roots, as an include must.
///
/// Fails with `project.duplicate` (two files for one docname), `project.link`,
/// `project.limit` (more than [`MAX_DOCUMENTS`]) or `host.io`.
pub fn discover(config: &Config) -> Result<Vec<SourceFile>, Problem> {
    let roots = gd_host::canonical_roots(&gd_conf::readable_roots(config));
    let mut found = Vec::new();
    walk_sources(config, &roots, &config.source_dir, &mut found)?;
    found.sort_by(|a, b| a.docname.cmp(&b.docname));
    if let Some(pair) = found.windows(2).find(|w| w[0].docname == w[1].docname) {
        return Err(Problem::new(
            "project.duplicate",
            "TWO SOURCES FOR ONE DOCUMENT",
            &pair[1].relative,
            format!(
                "{} and {} both define the document {}.",
                pair[0].relative, pair[1].relative, pair[1].docname
            ),
            "Rename one of them, or add one to exclude_patterns.",
            1,
        ));
    }
    Ok(found)
}

fn walk_sources(
    config: &Config,
    roots: &[PathBuf],
    dir: &Path,
    found: &mut Vec<SourceFile>,
) -> Result<(), Problem> {
    let entries = std::fs::read_dir(dir).map_err(|e| gd_host::io_problem(dir, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| gd_host::io_problem(dir, &e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let relative = relative_to(&config.source_dir, &path);
        let file_type = entry.file_type().map_err(|e| gd_host::io_problem(&path, &e))?;
        if file_type.is_dir() {
            if !skipped_directory(config, &path, &relative) {
                walk_sources(config, roots, &path, found)?;
            }
            continue;
        }
        if !file_type.is_file() && !file_type.is_symlink() {
            continue;
        }
        let Some(suffix) = source_suffix(config, &name) else { continue };
        if is_excluded(config, &relative) || !is_included(config, &relative) {
            continue;
        }
        add_source(roots, dir, path, relative, suffix, file_type.is_symlink(), found)?;
    }
    Ok(())
}

fn add_source(
    roots: &[PathBuf],
    dir: &Path,
    path: PathBuf,
    relative: String,
    suffix: &str,
    link: bool,
    found: &mut Vec<SourceFile>,
) -> Result<(), Problem> {
    if link && !gd_host::confined(roots, &path) {
        return Err(Problem::new(
            "project.link",
            "DOCUMENT OUTSIDE THE PROJECT",
            &relative,
            "This document is a link to a file outside the source folder and the include \
             roots, which a build does not read.",
            "Add the folder it leads to to include_roots, or copy the file here.",
            1,
        ));
    }
    if found.len() >= MAX_DOCUMENTS {
        return Err(Problem::new(
            "project.limit",
            "TOO MANY DOCUMENTS",
            &dir.display().to_string(),
            format!("The project has more than {MAX_DOCUMENTS} documents."),
            "Exclude generated directories with exclude_patterns.",
            3,
        ));
    }
    let docname = relative[..relative.len() - suffix.len()].to_string();
    let kind = SourceKind::of_suffix(suffix);
    found.push(SourceFile { docname, path, relative, kind, generated: None });
    Ok(())
}

fn skipped_directory(config: &Config, path: &Path, relative: &str) -> bool {
    if path == config.build_dir {
        return true;
    }
    // As in Sphinx, template and asset folders never hold documents.
    let lists = [&config.templates_path, &config.html.static_path, &config.html.extra_path];
    if lists.iter().any(|l| l.iter().any(|t| relative == t.trim_end_matches('/'))) {
        return true;
    }
    is_excluded(config, relative)
}

/// The configured suffix `name` ends with, when it is longer than the suffix.
fn source_suffix<'c>(config: &'c Config, name: &str) -> Option<&'c str> {
    config
        .source_suffix
        .iter()
        .find(|s| name.len() > s.len() && name.ends_with(s.as_str()))
        .map(String::as_str)
}

/// `path` relative to `base`, with `/` separators; `path` itself (as text) when it is
/// not below `base`.
pub fn relative_to(base: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    let parts: Vec<String> =
        rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts.join("/")
}

/// Whether `exclude_patterns` excludes `relative`, or one of its parent folders.
pub fn is_excluded(config: &Config, relative: &str) -> bool {
    config.exclude_patterns.iter().any(|pattern| {
        glob_match(pattern, relative)
            || relative.match_indices('/').any(|(i, _)| glob_match(pattern, &relative[..i]))
    })
}

/// Whether `include_patterns` lets `relative` be a document (an empty list: all).
pub fn is_included(config: &Config, relative: &str) -> bool {
    config.include_patterns.is_empty()
        || config.include_patterns.iter().any(|p| glob_match(p, relative))
}

/// Sphinx's patterns: `*` matches within one path segment, `**` across segments, `?`
/// one character but `/`, and `[...]` a character class (`[!...]` or `[^...]` negated).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let (p, n) = (pattern.as_bytes(), name.as_bytes());
    let (mut pi, mut ni) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    let mut deep = false;
    while ni < n.len() {
        if pi < p.len() {
            match p[pi] {
                b'*' => {
                    deep = p.get(pi + 1) == Some(&b'*');
                    pi += if deep { 2 } else { 1 };
                    if deep && p.get(pi) == Some(&b'/') {
                        pi += 1;
                    }
                    star = Some((pi, ni));
                    continue;
                }
                b'?' if n[ni] != b'/' => {
                    pi += 1;
                    ni += 1;
                    continue;
                }
                b'[' => {
                    if let Some(next) = match_class(p, pi, n[ni]) {
                        pi = next;
                        ni += 1;
                        continue;
                    }
                }
                c if c == n[ni] && c != b'?' => {
                    pi += 1;
                    ni += 1;
                    continue;
                }
                _ => {}
            }
        }
        let Some((star_p, star_n)) = star else { return false };
        if !deep && n[star_n] == b'/' {
            return false;
        }
        star = Some((star_p, star_n + 1));
        (pi, ni) = (star_p, star_n + 1);
    }
    p[pi..].iter().all(|&c| c == b'*')
}

/// Matches `c` against the class starting at `start`; the index after the class when
/// it matches, `None` when it does not or the class is not closed.
fn match_class(p: &[u8], start: usize, c: u8) -> Option<usize> {
    let mut i = start + 1;
    let negate = matches!(p.get(i), Some(b'!' | b'^'));
    if negate {
        i += 1;
    }
    let mut matched = false;
    let mut first = true;
    while i < p.len() && (first || p[i] != b']') {
        first = false;
        let lo = p[i];
        let mut hi = lo;
        if i + 2 < p.len() && p[i + 1] == b'-' && p[i + 2] != b']' {
            hi = p[i + 2];
            i += 2;
        }
        matched |= lo <= c && c <= hi;
        i += 1;
    }
    if i >= p.len() || matched == negate {
        return None;
    }
    Some(i + 1)
}

/// Keeps a project from before Sphinx 2.0 building, as Sphinx's `check_root_doc` does:
/// when `root_doc` is `index`, the default, and there is no such document but there is
/// a `contents`, `contents` becomes the root (and the root of books that started from
/// `index`). Returns the warning that says so, or `None` when nothing changed.
pub fn contents_root(config: &mut Config, sources: &[SourceFile]) -> Option<Report> {
    if config.root_doc != "index" {
        return None;
    }
    let has = |name: &str| sources.iter().any(|s| s.docname == name);
    if has("index") || !has("contents") {
        return None;
    }
    config.root_doc = "contents".into();
    for book in config.pdf.documents.iter_mut().filter(|b| b.root == "index") {
        book.root = "contents".into();
    }
    Some(Report {
        code: "build.root.contents".into(),
        severity: Severity::Warning,
        title: "ROOT DOCUMENT IS CONTENTS".into(),
        message: "There is no index document, so contents is the root document, as it was \
                  before Sphinx 2.0."
            .into(),
        hints: vec!["Set root_doc = \"contents\" in conf.toml.".into()],
        ..Report::default()
    })
}

/// Adds generated pages to `sources`, keeping them sorted by docname.
pub fn with_generated(
    mut sources: Vec<SourceFile>,
    generated: Vec<SourceFile>,
) -> Vec<SourceFile> {
    if generated.is_empty() {
        return sources;
    }
    sources.extend(generated);
    sources.sort_by(|a, b| a.docname.cmp(&b.docname));
    sources
}
