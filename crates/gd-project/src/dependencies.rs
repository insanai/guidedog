//! The dependency manifest, `gd-dependencies.json` in the output folder: for each page,
//! book or other output unit, the files it was made from, so that an editor rebuilds
//! only the outputs a changed file affects (GDS 0017).
//!
//! ```json
//! {"schema": 1, "builder": "pdf", "source_dir": "/work/docs",
//!  "shared": ["conf.toml"], "navigation": true,
//!  "outputs": [{"unit": "book:guide.pdf", "files": ["guide.pdf", "..."],
//!               "documents": ["index.rst", "intro.rst"],
//!               "inputs": ["_templates/book.typ", "images/a.png", "index.rst", "..."],
//!               "incomplete": false}]}
//! ```
//!
//! Paths are relative to the source folder with `/` separators, or absolute when they
//! lie outside it. `shared` lists what every output depends on. A page's unit is its
//! docname and a book's `book:<file>`; the other units, such as the files every page
//! shares (`""`), the search index and feeds, are made from every document. `inputs`
//! holds a unit's documents, the files they include, quote or show, the documents its
//! references found their titles and captions in, the project templates it loaded, the
//! files it copied, and for a book every file its Typst reads; `documents` repeats the
//! documents among them.
//!
//! `navigation` is true because every output also shows each document's title, its
//! section titles and its place in the toctree: a change to those changes every output.
//! `incomplete` marks a unit whose document asked for something that was not found,
//! such as a missing include or autodoc object; it may change when any file appears.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_env::{BuilderKind, DocInfo, Environment, LookupKind};
use gd_host::Problem;
use gd_outputs::{Generation, OutputRecord, UnitRecord};
use gd_resolve::Tables;

/// The manifest's file name in the output folder.
pub const DEPENDENCIES_FILE: &str = "gd-dependencies.json";

/// The version of the manifest's form; it changes when a field changes meaning.
pub const DEPENDENCIES_SCHEMA: u32 = 1;

/// The unit that holds the manifest; no docname can take this key.
const DEPENDENCIES_UNIT: &str = "<gd-dependencies>";

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
    project: (&Environment, &Tables),
    generation: &mut Generation,
) -> Result<(), Problem> {
    if !writes_dependencies(kind) {
        return Ok(());
    }
    let json = dependencies_json(&Sources::new(config, project.0, project.1), kind, generation);
    let mut unit = generation.unit(DEPENDENCIES_UNIT);
    unit.emit_text(DEPENDENCIES_FILE, &json)?;
    unit.finish(UnitRecord::default());
    Ok(())
}

/// What the manifest is made from: the project, its documents and its tables.
struct Sources<'a> {
    config: &'a Config,
    env: &'a Environment,
    tables: &'a Tables,
    docs: HashMap<&'a str, &'a DocInfo>,
    document_paths: HashMap<PathBuf, &'a DocInfo>,
    /// The canonical folders a build may read, which confine project templates.
    roots: Vec<PathBuf>,
}

impl<'a> Sources<'a> {
    fn new(config: &'a Config, env: &'a Environment, tables: &'a Tables) -> Sources<'a> {
        let source = &config.source_dir;
        Sources {
            config,
            env,
            tables,
            docs: env.docs.iter().map(|d| (d.docname.as_str(), d)).collect(),
            document_paths: env.docs.iter().map(|d| (source.join(&d.relative), d)).collect(),
            roots: gd_host::canonical_roots(&gd_conf::readable_roots(config)),
        }
    }
}

/// The files of one unit, and whether something it asked for was not found.
#[derive(Default)]
struct Inputs {
    paths: BTreeSet<PathBuf>,
    incomplete: bool,
}

fn dependencies_json(s: &Sources<'_>, kind: BuilderKind, generation: &Generation) -> String {
    let source = &s.config.source_dir;
    let mut out = format!(
        "{{\"schema\":{DEPENDENCIES_SCHEMA},\"builder\":\"{}\",\"source_dir\":",
        kind.name()
    );
    gd_report::escape_json(&mut out, &source.to_string_lossy());
    out.push_str(",\"shared\":");
    let shared = [s.config.config_dir.join("conf.toml")];
    texts(&mut out, shared.iter().map(|p| shown(source, p)));
    out.push_str(",\"navigation\":true,\"outputs\":[");
    let mut first = true;
    for (key, record) in generation.units() {
        if key == DEPENDENCIES_UNIT || record.files.is_empty() {
            continue;
        }
        let inputs = unit_inputs(s, key, record);
        let documents: Vec<String> = inputs
            .paths
            .iter()
            .filter_map(|p| s.document_paths.get(p).map(|d| d.relative.replace('\\', "/")))
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
        texts(&mut out, inputs.paths.iter().map(|p| shown(source, p)));
        out.push_str(if inputs.incomplete {
            ",\"incomplete\":true}"
        } else {
            ",\"incomplete\":false}"
        });
    }
    out.push_str("]}\n");
    out
}

/// The files unit `key` was made from: for a page its document with what the document
/// depends on, for a unit that is neither a page nor a book every document's, then the
/// documents its lookups found answers in, the images and downloads it copies, the
/// project templates it loaded, and the files it recorded reading or copied.
fn unit_inputs(s: &Sources<'_>, key: &str, record: &OutputRecord) -> Inputs {
    let mut inputs = Inputs::default();
    inputs.paths.extend(record.files_read.iter().cloned());
    match s.docs.get(key) {
        Some(doc) => add_document(s, doc, &mut inputs),
        None if !key.starts_with("book:") => {
            // The files every page shares, the search index and feeds are made from
            // every document.
            for doc in &s.env.docs {
                add_document(s, doc, &mut inputs);
            }
        }
        None => {}
    }
    for (lookup, _) in &record.lookups {
        for id in gd_resolve::lookup_documents(s.env, s.tables, lookup) {
            inputs.paths.insert(s.config.source_dir.join(&s.env.doc(id).relative));
        }
        if lookup.kind == LookupKind::Template
            && let Some(path) = template_path(s, &lookup.key)
        {
            inputs.paths.insert(path);
        }
    }
    inputs.paths.extend(record.assets.iter().map(|a| a.source.clone()));
    inputs
}

/// Adds `doc` and the files it depends on. A dependency that names no file marks
/// something that was not found, such as `<missing>`.
fn add_document(s: &Sources<'_>, doc: &DocInfo, inputs: &mut Inputs) {
    inputs.paths.insert(s.config.source_dir.join(&doc.relative));
    for dep in &doc.deps {
        if dep.path.is_absolute() {
            inputs.paths.insert(dep.path.clone());
        } else {
            inputs.incomplete = true;
        }
    }
}

/// The project template the HTML layout loaded for `name`, found as gd-site's loader
/// finds it: the first `templates_path` folder that holds it, within the folders a
/// build may read.
fn template_path(s: &Sources<'_>, name: &str) -> Option<PathBuf> {
    if name.contains("..") || name.starts_with('/') || name.is_empty() {
        return None;
    }
    s.config
        .templates_path
        .iter()
        .map(|dir| gd_conf::clean_path(&s.config.source_dir.join(dir)))
        .filter(|dir| gd_host::confined(&s.roots, dir))
        .map(|dir| dir.join(name))
        .find(|path| path.is_file() && gd_host::confined(&s.roots, path))
}

/// `path` relative to the source folder with `/` separators, or absolute outside it.
fn shown(source: &Path, path: &Path) -> String {
    let path = gd_conf::clean_path(path);
    let shown = path.strip_prefix(source).unwrap_or(&path);
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
