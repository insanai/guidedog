//! Migrating the guidedog.toml of projects made by the earlier `guidedog init`.

use std::path::{Path, PathBuf};

use gd_host::Problem;
use gd_toml::{Table, Value as Toml};

use crate::convert::{finish, write_setting};
use crate::migrate::{Conversion, Migration};
use crate::pyconf::Value;

/// Converts the text of a guidedog.toml at `path` (which names it in notes). Returns
/// the conversion and the folder conf.toml belongs in: the first source root, relative
/// to `path`'s folder.
///
/// Fails with `config.invalid` (exit 2) when the text is not TOML.
pub fn convert_guidedog_toml(text: &str, path: &Path) -> Result<(Conversion, PathBuf), Problem> {
    convert_guidedog_toml_in(text, path)
}

pub(crate) fn convert_guidedog_toml_in(
    text: &str,
    path: &Path,
) -> Result<(Conversion, PathBuf), Problem> {
    let shown = path.display().to_string();
    let root = gd_toml::parse(text).map_err(|e| {
        let mut p = Problem::new(
            "config.invalid",
            "INVALID CONFIGURATION",
            &shown,
            e.message.clone(),
            "Fix the line, then migrate again.",
            2,
        );
        p.path = format!("{shown}:{}", e.line);
        p
    })?;
    let mut m = Migration::new(text, path);
    let first = field(&root, "source", "roots")
        .and_then(Toml::as_array)
        .and_then(|roots| roots.first())
        .and_then(Toml::as_str)
        .unwrap_or(".");
    let dir = path.parent().unwrap_or(Path::new("")).join(first);
    for (name, key) in [
        ("project", "title"),
        ("author", "author"),
        ("language", "language"),
        ("version", "version"),
        ("release", "version"),
    ] {
        set_text(&mut m, name, field(&root, "project", key).and_then(Toml::as_str));
    }
    if let Some(entry) = field(&root, "source", "entry").and_then(Toml::as_str) {
        let doc = root_doc(first, entry);
        if doc != "index" {
            set_text(&mut m, "root_doc", Some(&doc));
        }
    }
    if let Some(exclude) = field(&root, "source", "exclude").and_then(Toml::as_array) {
        let items: Vec<Value> =
            exclude.iter().filter_map(Toml::as_str).map(|s| Value::Str(s.into())).collect();
        if !items.is_empty() {
            write_setting(&mut m, "exclude_patterns", &Value::List(items));
            m.converted += 1;
        }
    }
    if let Some(paper) = field(&root, "pdf", "paper").and_then(Toml::as_str) {
        let size = if paper == "us-letter" { "letter" } else { paper };
        set_text(&mut m, "pdf_paper_size", Some(size));
    }
    legacy_notes(&mut m, &root);
    Ok((finish(m), dir))
}

/// The document name of `entry` (a path from the project folder) inside the source
/// root `first`, without its suffix.
fn root_doc(first: &str, entry: &str) -> String {
    let entry_path = Path::new(entry);
    let relative = entry_path.strip_prefix(first).unwrap_or(entry_path);
    let relative = relative.with_extension("");
    relative.to_string_lossy().replace('\\', "/")
}

fn legacy_notes(m: &mut Migration<'_>, root: &Table) {
    if let Some(order) = field(root, "source", "order") {
        m.warn(
            order.line,
            "migrate.order",
            "READING ORDER NOW COMES FROM TOCTREES",
            "source.order listed the reading order; conf.toml projects take it from the root \
             document's toctree."
                .into(),
            "Add a toctree directive to the root document that lists the documents in this \
             order.",
        );
    }
    if let Some(strict) = field(root, "build", "strict").filter(|s| s.as_bool() == Some(true)) {
        m.tell(
            strict.line,
            "migrate.strict",
            "STRICT BUILDS ARE AN OPTION",
            "build.strict has no conf.toml setting.".into(),
            "Run guidedog build -W to treat warnings as errors.",
        );
    }
    for table in ["html", "pdf"] {
        if let Some(theme) = field(root, table, "theme") {
            m.tell(
                theme.line,
                "migrate.theme",
                "THEME REPLACED",
                format!("The {table} theme is replaced by the templates quickstart writes."),
                "Copy _templates and _static from a new guidedog quickstart project to \
                 customize the look.",
            );
        }
    }
}

fn field<'t>(root: &'t Table, table: &str, key: &str) -> Option<&'t Toml> {
    root.path(&[table, key])
}

fn set_text(m: &mut Migration<'_>, name: &str, value: Option<&str>) {
    let Some(text) = value.filter(|t| !t.is_empty()) else { return };
    write_setting(m, name, &Value::Str(text.into()));
    m.converted += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_doc_is_relative_to_the_first_root() {
        assert_eq!(root_doc("docs", "docs/start.rst"), "start");
        assert_eq!(root_doc(".", "guide/intro.md"), "guide/intro");
    }
}
