//! Declarative project mounts, language editions and shared publication assets.

use gd_host::Problem;
use gd_toml::{Data, Table, Value};

use crate::site::{invalid, strings, valid_output};

/// A project build that owns files under a public output prefix.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteMount {
    /// Unique mount name, also used by cross-mount links.
    pub name: String,
    /// Project directory relative to the site's configuration directory.
    pub source: String,
    /// Public output directory; empty mounts at the website root.
    pub output: String,
    /// html, dirhtml, pdf or discussion-pdf.
    pub builder: String,
    /// Project setting overrides, as -D assignments.
    pub overrides: Vec<String>,
    /// Output globs to omit during assembly.
    pub exclude: Vec<String>,
    /// Optional language editions; empty builds the project's configured language once.
    pub editions: Vec<SiteEdition>,
    /// Discussion catalog prefix for discussion-pdf; empty selects gds.
    pub prefix: String,
}

/// A language-specific mount instance; gettext is used unless source is overridden.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteEdition {
    /// Guidedog language name, such as en or zh_CN.
    pub language: String,
    /// Output directory beneath the mount's output directory; empty for its default edition.
    pub output: String,
    /// Optional separate translated project directory, relative to the site root.
    pub source: String,
    /// Overrides applied after the mount's settings.
    pub overrides: Vec<String>,
}

/// A shared file or directory, copied once into the aggregate website.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteAsset {
    /// File or directory relative to the site's configuration directory.
    pub source: String,
    /// Relative destination directory; empty means the website root.
    pub output: String,
}

pub(crate) fn mounts(items: &[Value], path: &str) -> Result<Vec<SiteMount>, Problem> {
    let mounts: Vec<_> = items.iter().map(|v| mount(v, path)).collect::<Result<_, _>>()?;
    let mut names = std::collections::BTreeSet::new();
    for mount in &mounts {
        if !names.insert(mount.name.to_ascii_lowercase()) {
            return Err(invalid(path, 0, "Mount names must be unique, ignoring ASCII case."));
        }
    }
    Ok(mounts)
}

fn mount(value: &Value, path: &str) -> Result<SiteMount, Problem> {
    let mut mount = SiteMount { builder: "html".into(), ..Default::default() };
    for (key, v) in table(value, path)?.iter() {
        match (key, &v.data) {
            ("name", Data::String(s)) => mount.name.clone_from(s),
            ("source", Data::String(s)) => mount.source.clone_from(s),
            ("output", Data::String(s)) => mount.output.clone_from(s),
            ("builder", Data::String(s)) => mount.builder.clone_from(s),
            ("prefix", Data::String(s)) => mount.prefix.clone_from(s),
            ("overrides", Data::Array(items)) => mount.overrides = strings(items, path)?,
            ("exclude", Data::Array(items)) => mount.exclude = strings(items, path)?,
            ("editions", Data::Array(items)) => {
                mount.editions =
                    items.iter().map(|v| edition(v, path)).collect::<Result<_, _>>()?;
            }
            _ => return Err(invalid(path, v.line, &format!("Invalid mount setting: {key}."))),
        }
    }
    if !valid_output(&mount.name) || mount.name.contains('/') || mount.source.is_empty() {
        return Err(invalid(path, value.line, "A mount requires a portable name and source."));
    }
    output(&mount.output, value, path)?;
    if mount
        .overrides
        .iter()
        .chain(mount.editions.iter().flat_map(|e| &e.overrides))
        .any(|s| s.split_once('=').is_some_and(|(key, _)| key.trim() == "language"))
    {
        return Err(invalid(
            path,
            value.line,
            "Use edition.language instead of a language override.",
        ));
    }
    if !matches!(mount.builder.as_str(), "html" | "dirhtml" | "pdf" | "discussion-pdf") {
        return Err(invalid(
            path,
            value.line,
            "Mount builder must be html, dirhtml, pdf or discussion-pdf.",
        ));
    }
    let mut languages = std::collections::BTreeSet::new();
    let mut outputs = std::collections::BTreeSet::new();
    for edition in &mount.editions {
        if !languages.insert(edition.language.replace('-', "_").to_ascii_lowercase())
            || !outputs.insert(edition.output.to_ascii_lowercase())
        {
            return Err(invalid(
                path,
                value.line,
                "Edition languages and output directories must be unique.",
            ));
        }
    }
    Ok(mount)
}

fn edition(value: &Value, path: &str) -> Result<SiteEdition, Problem> {
    let mut edition = SiteEdition::default();
    for (key, v) in table(value, path)?.iter() {
        match (key, &v.data) {
            ("language", Data::String(s)) => edition.language.clone_from(s),
            ("output", Data::String(s)) => edition.output.clone_from(s),
            ("source", Data::String(s)) => edition.source.clone_from(s),
            ("overrides", Data::Array(items)) => edition.overrides = strings(items, path)?,
            _ => return Err(invalid(path, v.line, &format!("Invalid edition setting: {key}."))),
        }
    }
    if edition.language.is_empty()
        || !edition.language.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(invalid(path, value.line, "Each edition requires a language name."));
    }
    output(&edition.output, value, path)?;
    Ok(edition)
}

pub(crate) fn assets(items: &[Value], path: &str) -> Result<Vec<SiteAsset>, Problem> {
    items
        .iter()
        .map(|v| {
            let mut asset = SiteAsset::default();
            for (key, value) in table(v, path)?.iter() {
                match (key, &value.data) {
                    ("source", Data::String(s)) => asset.source.clone_from(s),
                    ("output", Data::String(s)) => asset.output.clone_from(s),
                    _ => {
                        return Err(invalid(
                            path,
                            value.line,
                            &format!("Invalid asset setting: {key}."),
                        ));
                    }
                }
            }
            if asset.source.is_empty() {
                return Err(invalid(path, v.line, "An asset requires a source."));
            }
            output(&asset.output, v, path)?;
            Ok(asset)
        })
        .collect()
}

fn table<'a>(value: &'a Value, path: &str) -> Result<&'a Table, Problem> {
    match &value.data {
        Data::Table(table) => Ok(table),
        _ => Err(invalid(path, value.line, "Expected a mount, edition or asset table.")),
    }
}

fn output(text: &str, value: &Value, path: &str) -> Result<(), Problem> {
    if text.is_empty() || valid_output(text) {
        Ok(())
    } else {
        Err(invalid(path, value.line, "Output directories must be portable relative paths."))
    }
}
