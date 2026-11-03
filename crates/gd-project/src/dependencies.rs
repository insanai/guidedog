//! The dependency manifest, `gd-dependencies.json` in the output folder: for each page,
//! book or other output unit, the files it was made from, so that an editor rebuilds
//! only the outputs a changed file affects (GDS 0017).
//!
//! ```json
//! {"schema": 1, "builder": "pdf", "source_dir": "/work/docs",
//!  "shared": ["conf.toml"],
//!  "outputs": [{"unit": "book:guide.pdf", "files": ["guide.pdf", "..."],
//!               "documents": ["index.rst", "intro.rst"],
//!               "inputs": ["_templates/book.typ", "images/a.png", "index.rst", "..."]}]}
//! ```
//!
//! Paths are relative to the source folder with `/` separators, or absolute when they
//! lie outside it. `shared` lists what every output depends on. A page's unit is its
//! docname, a book's `book:<file>`, and the files every page shares are the unit `""`,
//! made from every document. `inputs` holds a unit's
//! documents, the files they include, quote or show, and for a book every file its Typst
//! reads; `documents` repeats the documents among them.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::{BuilderKind, DocInfo, Environment, LookupKind};
use gd_host::Problem;
use gd_outputs::{Generation, OutputRecord, UnitRecord};

/// The manifest's file name in the output folder.
pub const DEPENDENCIES_FILE: &str = "gd-dependencies.json";

/// The version of the manifest's form; it changes when a field changes meaning.
pub const DEPENDENCIES_SCHEMA: u32 = 1;

/// The unit that holds the manifest.
const DEPENDENCIES_UNIT: &str = "dependencies";

/// Tells whether `kind` writes a dependency manifest: every builder whose output a
/// person reads.
pub fn writes_dependencies(kind: BuilderKind) -> bool {
    matches!(
        kind,
        BuilderKind::Html
            | BuilderKind::Dirhtml
            | BuilderKind::Singlehtml
            | BuilderKind::Pdf
            | BuilderKind::Text
    )
}

/// Writes the manifest of the units `generation` holds so far, as their own unit.
/// Fails only when the file cannot be staged.
pub(crate) fn write_dependencies(
    config: &Config,
    kind: BuilderKind,
    env: &Environment,
    generation: &mut Generation,
) -> Result<(), Problem> {
    if !writes_dependencies(kind) {
        return Ok(());
    }
    let json = dependencies_json(config, kind, env, generation);
    let mut unit = generation.unit(DEPENDENCIES_UNIT);
    unit.emit_text(DEPENDENCIES_FILE, &json)?;
    unit.finish(UnitRecord::default());
    Ok(())
}

fn dependencies_json(
    config: &Config,
    kind: BuilderKind,
    env: &Environment,
    generation: &Generation,
) -> String {
    let source = &config.source_dir;
    let docs: HashMap<&str, &DocInfo> = env.docs.iter().map(|d| (d.docname.as_str(), d)).collect();
    let document_paths: HashMap<PathBuf, &DocInfo> =
        env.docs.iter().map(|d| (source.join(&d.relative), d)).collect();
    let mut out = format!(
        "{{\"schema\":{DEPENDENCIES_SCHEMA},\"builder\":\"{}\",\"source_dir\":",
        kind.name()
    );
    gd_report::escape_json(&mut out, &source.to_string_lossy());
    out.push_str(",\"shared\":");
    let shared = [config.config_dir.join("conf.toml")];
    texts(&mut out, shared.iter().map(|p| shown(source, p)));
    out.push_str(",\"outputs\":[");
    let mut first = true;
    for (key, record) in generation.units() {
        if key == DEPENDENCIES_UNIT || record.files.is_empty() {
            continue;
        }
        let mut inputs = unit_inputs(config, docs.get(key).copied(), record);
        if key == gd_outputs::SITE_UNIT {
            // The shared files (search index, general index, copied sources) are made
            // from every document.
            inputs.extend(document_paths.keys().cloned());
        }
        let documents: Vec<String> = inputs
            .iter()
            .filter_map(|p| document_paths.get(p).map(|d| d.relative.replace('\\', "/")))
            .collect();
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str("{\"unit\":");
        gd_report::escape_json(&mut out, key);
        out.push_str(",\"files\":");
        texts(&mut out, record.files.iter().cloned());
        out.push_str(",\"documents\":");
        texts(&mut out, documents.into_iter());
        out.push_str(",\"inputs\":");
        texts(&mut out, inputs.iter().map(|p| shown(source, p)));
        out.push('}');
    }
    out.push_str("]}\n");
    out
}

/// The files unit `record` was made from: its document (`doc`, for a page) with the
/// files it depends on, the images and downloads it copies, the project templates it
/// loaded, and the files it recorded reading.
fn unit_inputs(
    config: &Config,
    doc: Option<&DocInfo>,
    record: &OutputRecord,
) -> BTreeSet<PathBuf> {
    let mut inputs: BTreeSet<PathBuf> = record.files_read.iter().cloned().collect();
    if let Some(doc) = doc {
        inputs.insert(config.source_dir.join(&doc.relative));
        inputs.extend(doc.deps.iter().map(|d| d.path.clone()));
    }
    inputs.extend(record.assets.iter().map(|a| a.source.clone()));
    for (lookup, _) in record.lookups.iter().filter(|(l, _)| l.kind == LookupKind::Template) {
        for folder in &config.templates_path {
            let path = config.source_dir.join(folder).join(&lookup.key);
            if path.is_file() {
                inputs.insert(path);
                break;
            }
        }
    }
    inputs
}

/// `path` relative to the source folder with `/` separators, or absolute outside it.
fn shown(source: &Path, path: &Path) -> String {
    let shown = path.strip_prefix(source).unwrap_or(path);
    shown.to_string_lossy().replace('\\', "/")
}

fn texts(out: &mut String, items: impl Iterator<Item = String>) {
    out.push('[');
    for (i, item) in items.enumerate() {
        if i > 0 {
            out.push(',');
        }
        gd_report::escape_json(out, &item);
    }
    out.push(']');
}
