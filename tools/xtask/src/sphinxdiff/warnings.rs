//! Where each builder warned. Warnings are compared per document: how many warnings and
//! how many errors each builder reports for each source file (`index.rst warning (2)`),
//! or for none (`(no location)`). The messages differ by design (Guidedog's say what to
//! do), and so can the line of a problem spanning several (Docutils names a directive's
//! first line, Guidedog the line at fault), so the lines are listed for reading
//! (`index.rst:12 warning`) but not counted. Guidedog's notes about the configuration
//! (`config.*`) are left out: the configuration is the conversion of conf.py by
//! `guidedog migrate`, which Sphinx has no word about.

use std::path::{Component, Path, PathBuf};

use super::html::counted;

/// One warning or error: the source document it names, relative to the project, its
/// severity (`warning` or `error`) and its line (0 for none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Warning {
    pub document: String,
    pub severity: &'static str,
    pub line: usize,
}

/// The levels sphinx-build writes, in the order a line is tried with them.
const SPHINX_LEVELS: [&str; 4] = ["WARNING", "ERROR", "CRITICAL", "SEVERE"];

/// The warnings and errors of a sphinx-build log, one per line of the form
/// `PATH:LINE: WARNING: message` (or `WARNING: message` without a location).
pub(crate) fn sphinx_warnings(log: &str, source_dir: &Path) -> Vec<Warning> {
    log.lines().filter_map(|line| sphinx_warning(line, source_dir)).collect()
}

fn sphinx_warning(line: &str, source_dir: &Path) -> Option<Warning> {
    for level in SPHINX_LEVELS {
        let severity = if level == "WARNING" { "warning" } else { "error" };
        match line.find(&format!(": {level}: ")) {
            Some(at) => {
                let (document, line) = split_location(&line[..at], source_dir);
                return Some(Warning { document, severity, line });
            }
            None if line.starts_with(&format!("{level}: ")) => {
                return Some(Warning { document: "(no location)".into(), severity, line: 0 });
            }
            None => {}
        }
    }
    None
}

/// Splits `SOURCE_DIR/doc.rst:12` into `("doc.rst", 12)`; a location without a line
/// number (`a.py:docstring of f`) keeps its whole text and line 0.
pub(crate) fn split_location(location: &str, source_dir: &Path) -> (String, usize) {
    let location = relative_to(location, source_dir);
    let Some((path, number)) = location.rsplit_once(':') else { return (location, 0) };
    if !number.bytes().all(|b| b.is_ascii_digit()) {
        return (location, 0);
    }
    (path.to_string(), number.parse().unwrap_or(0))
}

/// `path` relative to the project's source folder `dir`, `(project)` for the folder
/// itself, or `path` as written when it lies outside. Guidedog shortens paths against
/// the working folder and Sphinx writes them absolute, so both are made absolute against
/// the working folder first, lexically: the file named may not exist, and a location
/// may carry a suffix such as `:12`.
pub(crate) fn relative_to(path: &str, dir: &Path) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let base = slashed(&lexical_absolute(dir, &cwd));
    let base = base.trim_end_matches('/');
    let full = slashed(&lexical_absolute(Path::new(path), &cwd));
    if full == base {
        return String::from("(project)");
    }
    match full.strip_prefix(base).and_then(|rest| rest.strip_prefix('/')) {
        Some(relative) => relative.to_string(),
        None => path.to_string(),
    }
}

/// `path` joined to `cwd` when relative, with `.` and `..` resolved by name.
fn lexical_absolute(path: &Path, cwd: &Path) -> PathBuf {
    let joined = if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The warnings and errors of a Guidedog log written with `--diagnostics=json`, one JSON
/// object per line, leaving out notes, configuration notes (`config.*`) and the build's
/// summary of documents that failed (`build.read`). Lines that are not JSON are skipped.
pub(crate) fn guidedog_warnings(log: &str, source_dir: &Path) -> Vec<Warning> {
    let mut warnings = Vec::new();
    for line in log.lines().filter(|line| line.starts_with('{')) {
        let Ok(value) = gd_json::parse(line) else { continue };
        let text = |name: &str| value.get(name).and_then(|v| v.as_str()).unwrap_or_default();
        let severity = match text("severity") {
            "warning" => "warning",
            "error" => "error",
            _ => continue,
        };
        let code = text("code");
        if code == "build.read" || code.starts_with("config.") {
            continue;
        }
        let path = text("path");
        let document =
            if path.is_empty() { "(no location)".into() } else { relative_to(path, source_dir) };
        let line = value.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
        warnings.push(Warning { document, severity, line: usize::try_from(line).unwrap_or(0) });
    }
    warnings
}

/// Counts warnings per document and severity: `doc.rst warning (2)`.
pub(crate) fn per_document(warnings: &[Warning]) -> Vec<String> {
    counted(warnings.iter().map(|w| format!("{} {}", w.document, w.severity)).collect())
}

/// Lists warnings with their lines, for reading: `doc.rst:12 warning`.
pub(crate) fn located(warnings: &[Warning]) -> Vec<String> {
    let lines = warnings.iter().map(|w| match w.line {
        0 => format!("{} {}", w.document, w.severity),
        line => format!("{}:{line} {}", w.document, w.severity),
    });
    counted(lines.collect())
}
