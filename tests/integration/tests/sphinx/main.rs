//! Sphinx semantics end to end: small projects built with the real `guidedog` (Sphinx's
//! test roots, tables of contents, object types declared in conf.toml, objects.inv,
//! translation, and what a build may read), and the anchor ids the engine gives
//! documents, compared with the ids Sphinx publishes.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod anchors;
mod declared;
mod intl;
mod inventory;
mod roots;
mod toc;
mod trust;

use std::path::Path;

use gd_json::Value;
use guidedog_tests::{Run, Scratch, guidedog};

/// A diagnostic a build printed with `--diagnostics=json`.
#[derive(Debug, Clone, Default)]
pub struct Diag {
    /// Its code, such as `build.numref`.
    pub code: String,
    /// Its message.
    pub message: String,
    /// Its first hint.
    pub hint: String,
    /// The source it points at, relative to the project.
    pub path: String,
    /// Its line, or 0.
    pub line: i64,
}

/// The diagnostics of `stderr`, one JSON object per line.
pub fn diagnostics(stderr: &str) -> Vec<Diag> {
    let field = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
    stderr
        .lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| gd_json::parse(l).ok())
        .map(|v| Diag {
            code: field(&v, "code"),
            message: field(&v, "message"),
            hint: field(&v, "hint"),
            path: field(&v, "path"),
            line: v.get("line").and_then(Value::as_i64).unwrap_or(0),
        })
        .collect()
}

/// The diagnostic whose message is `message`; panics, listing them all, when none is.
pub fn with_message<'a>(diags: &'a [Diag], message: &str) -> &'a Diag {
    diags
        .iter()
        .find(|d| d.message == message)
        .unwrap_or_else(|| panic!("no diagnostic says {message:?}: {diags:#?}"))
}

/// How many diagnostics have `code`.
pub fn code_count(diags: &[Diag], code: &str) -> usize {
    diags.iter().filter(|d| d.code == code).count()
}

/// Panics unless `text` holds each of `parts`, in order.
pub fn in_order(text: &str, parts: &[&str]) {
    let mut rest = text;
    for part in parts {
        let at = rest.find(part).unwrap_or_else(|| panic!("no {part:?} (in order) in:\n{text}"));
        rest = &rest[at + part.len()..];
    }
}

/// A scratch folder holding `conf.toml` with `conf` and the other `files`.
pub fn project(name: &str, conf: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Scratch::new(name);
    dir.write("conf.toml", conf);
    for (path, text) in files {
        dir.write(path, text);
    }
    dir
}

/// A project made by `guidedog quickstart` (theme templates and all), then `files`.
pub fn quickstart(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = Scratch::new(name);
    guidedog(&dir.path, 0, &["quickstart", "-q", "-p", "Demo", "-a", "Ada"]);
    for (path, text) in files {
        dir.write(path, text);
    }
    dir
}

/// Runs `guidedog build <args> --color=never` in `dir`, expecting `status`.
pub fn build(dir: &Path, status: i32, args: &[&str]) -> Run {
    let mut all = vec!["build"];
    all.extend_from_slice(args);
    all.push("--color=never");
    guidedog(dir, status, &all)
}

/// Builds HTML with JSON diagnostics, expecting success, and returns the diagnostics.
pub fn build_json(dir: &Path) -> Vec<Diag> {
    diagnostics(&build(dir, 0, &["html", "--diagnostics=json"]).err)
}
