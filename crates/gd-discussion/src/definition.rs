//! Finding a catalog's configuration: `discussion.toml`, or the `.discussion.json` of a
//! catalog made by version 0.2.1.

use crate::config::{Definition, Process, is_valid_labels, is_valid_name, read_config};
use crate::problem::{io_problem, shown};
use crate::{CONFIG_FILE, DEFINITION_FILE, Prefix, Problem, Result, exists, read_text};
use std::path::{Path, PathBuf};

/// Reads the configuration of the catalog in `dir`: its [`CONFIG_FILE`], or else its
/// [`DEFINITION_FILE`]; `None` when it has neither. A malformed one is a problem.
pub fn definition(dir: &Path) -> Result<Option<Definition>> {
    let config = dir.join(CONFIG_FILE);
    if exists(&config) {
        let source = read_text("discussion", &config, 256 * 1024)?;
        return read_config(&shown(&config), &source).map(Some);
    }
    let path = dir.join(DEFINITION_FILE);
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_problem("discussion", &path, &e)),
    }
    let source = read_text("discussion", &path, 64 * 1024)?;
    legacy_definition(&source).map(Some).ok_or_else(|| {
        Problem::new("discussion.definition", "Invalid catalog definition", shown(&path),
            "A definition is JSON with schema 1, a prefix of 2 to 16 lowercase letters, a \
             one-line name, and optionally an author and a list of lowercase labels.",
            "Restore the file from version control; a live catalog's prefix never changes.")
    })
}

/// Reads a 0.2.1 definition; such a catalog follows the default process.
fn legacy_definition(source: &str) -> Option<Definition> {
    let value = gd_json::parse(source).ok()?;
    let text = |key: &str| value.get(key).and_then(gd_json::Value::as_str);
    if value.get("schema").and_then(gd_json::Value::as_i64) != Some(1) {
        return None;
    }
    let labels = match value.get("labels") {
        None | Some(gd_json::Value::Null) => Vec::new(),
        Some(v) => v.as_array()?.iter().map(|l| l.as_str().map(str::to_string)).collect::<
            Option<Vec<_>>,
        >()?,
    };
    let d = Definition {
        prefix: Prefix::new(text("prefix")?).ok()?,
        name: text("name")?.to_string(),
        author: text("author").unwrap_or("").to_string(),
        labels,
        process: Process::default(),
    };
    let valid = is_valid_name(&d.name)
        && (d.author.is_empty() || is_valid_name(&d.author))
        && is_valid_labels(&d.labels);
    valid.then_some(d)
}

/// Finds the catalog that holds the folder `from`: the nearest folder at or above it
/// with a configuration, and that configuration; `None` when there is none.
pub fn discover(from: &Path) -> Result<Option<(PathBuf, Definition)>> {
    let start = std::path::absolute(from).map_err(|e| io_problem("discussion", from, &e))?;
    let mut current = start.as_path();
    loop {
        if let Some(d) = definition(current)? {
            return Ok(Some((current.to_path_buf(), d)));
        }
        match current.parent() {
            Some(parent) if parent != current && !parent.as_os_str().is_empty() => {
                current = parent;
            }
            _ => return Ok(None),
        }
    }
}

/// The problem of a configuration whose prefix is not the one asked for.
pub(crate) fn definition_mismatch(dir: &Path, defined: &Prefix, given: &Prefix) -> Problem {
    Problem::new("discussion.prefix", "The prefix differs from the catalog's", shown(dir),
        format!("The catalog's configuration names prefix {defined}, not {given}."),
        format!("Pass --prefix={defined}, or leave --prefix out in this catalog."))
    .exit(2)
}
