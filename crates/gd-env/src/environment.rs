//! The environment: what reading learned about each document, kept between builds.
//!
//! Reading fills a `DocInfo` per document: the digest of its source, the files it
//! depended on, where its parsed tree is cached (an object, see `ObjectStore`), and the
//! symbols other documents may refer to. Resolving joins them into project tables;
//! writing loads objects back instead of parsing sources again. The environment is
//! saved as JSON (`.doctrees/environment.json`) and reused when its version and
//! settings digest match (docs/manual/internals/incremental.rst).

use std::path::{Path, PathBuf};

use gd_conf::{Config, Digest};
use gd_host::{Budget, Ledger, Problem};
use gd_report::Report;

use crate::discover::SourceKind;
use crate::records;

/// The environment's format version; an environment of another version is not read.
/// 26 is the Rust port's first (the Odin build wrote 25).
pub const ENVIRONMENT_VERSION: u32 = 26;

/// A document's index in the environment, which the project tables are indexed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DocId(pub u32);

impl DocId {
    /// The index as a `usize`, for indexing the environment's documents.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A section of a document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionInfo {
    /// Its level, 1 for the document title.
    pub level: u32,
    /// Its title, as text.
    pub title: String,
    /// Its id on the page.
    pub anchor: String,
    /// The index of the enclosing section; `None` at the top.
    pub parent: Option<u32>,
    /// A Markdown heading anchor, for links such as `page.md#slug`; may be empty.
    pub slug: String,
    /// Every id it was read with, its anchor first.
    pub ids: Vec<String>,
    /// In an object's description: no table of contents entry or number.
    pub in_object: bool,
    /// reStructuredText: the title as written, autosectionlabel's name for it.
    pub raw_title: String,
    /// The `only` expressions it stands in, joined with `and`; empty for none.
    pub only: String,
}

/// One entry of a toctree, as written.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TocEntry {
    /// A docname, a glob, a URL, or `self`.
    pub target: String,
    /// The explicit title; empty for none.
    pub title: String,
}

/// A toctree directive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToctreeInfo {
    /// The enclosing section; `None` at the top.
    pub section: Option<u32>,
    /// How many of the document's sections come before it.
    pub position: u32,
    /// Its line in the document; 0 when an included file holds it.
    pub line: u32,
    /// `:maxdepth:`; 0: unlimited.
    pub maxdepth: u32,
    /// `:caption:`.
    pub caption: String,
    /// `:hidden:`.
    pub hidden: bool,
    /// `:includehidden:`.
    pub includehidden: bool,
    /// `:titlesonly:`.
    pub titlesonly: bool,
    /// `:glob:`.
    pub glob: bool,
    /// `:reversed:`.
    pub reversed: bool,
    /// `:numbered:` depth; 0: not numbered.
    pub numbered: u32,
    /// The entries, as written.
    pub entries: Vec<TocEntry>,
    /// As [`SectionInfo::only`].
    pub only: String,
}

/// An object description's entry in its document's table of contents
/// (`toc_object_entries`), nested as Sphinx's TocTreeCollector places it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TocObjectInfo {
    /// The domain's name for the object, shown as code.
    pub title: String,
    /// Its id; empty when it is the document's first entry.
    pub anchor: String,
    /// The enclosing section, used when `parent` is `None`.
    pub section: Option<u32>,
    /// The entry of the object it is described in.
    pub parent: Option<u32>,
    /// How many of the document's sections come before it.
    pub position: u32,
    /// How many of the document's toctrees come before it.
    pub toctrees: u32,
    /// As [`SectionInfo::only`].
    pub only: String,
}

/// What a label names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LabelKind {
    /// Anything else (a paragraph, an anonymous target).
    #[default]
    Other,
    /// A section.
    Section,
    /// A figure.
    Figure,
    /// A table.
    Table,
    /// A code block.
    Code,
    /// A numbered equation.
    Equation,
}

impl LabelKind {
    /// Every kind, in a fixed order (numbering keeps a counter per kind).
    pub const ALL: [LabelKind; 6] = [
        LabelKind::Other,
        LabelKind::Section,
        LabelKind::Figure,
        LabelKind::Table,
        LabelKind::Code,
        LabelKind::Equation,
    ];

    /// The name saved in the environment.
    pub fn name(self) -> &'static str {
        match self {
            LabelKind::Other => "other",
            LabelKind::Section => "section",
            LabelKind::Figure => "figure",
            LabelKind::Table => "table",
            LabelKind::Code => "code",
            LabelKind::Equation => "equation",
        }
    }

    /// The kind [`LabelKind::name`] names.
    pub fn from_name(name: &str) -> Option<LabelKind> {
        LabelKind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// A label (`.. _name:`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LabelInfo {
    /// The normalized name.
    pub name: String,
    /// The id it reaches.
    pub anchor: String,
    /// The section title or caption; empty for an anonymous label.
    pub title: String,
    /// What it names.
    pub kind: LabelKind,
}

/// A glossary term.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TermInfo {
    /// The lowercased term.
    pub name: String,
    /// Its id.
    pub anchor: String,
}

/// An object a domain described.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectInfo {
    /// The domain, such as `py`.
    pub domain: String,
    /// The object type, such as `function`.
    pub objtype: String,
    /// The full name.
    pub fullname: String,
    /// Its id.
    pub anchor: String,
    /// The canonical name of a Python object (`:canonical:`), not the object's own.
    pub aliased: bool,
    /// Its signature's line in the document; 0 when unknown.
    pub line: u32,
}

/// A C++ declaration: the path of its symbol, its type and directive, its newest id,
/// and the names of its template and function parameters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CppDeclInfo {
    /// The symbol's encoded path.
    pub path: String,
    /// The object type.
    pub objtype: String,
    /// The directive that declared it.
    pub directive: String,
    /// Its newest id.
    pub id: String,
    /// Template parameter names.
    pub tparams: Vec<String>,
    /// Function parameter names.
    pub fparams: Vec<String>,
    /// Signatures of one directive share a group.
    pub group: u32,
    /// Its signature's line in the document; 0 when unknown.
    pub line: u32,
}

/// A module a domain declared, for the module index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleInfo {
    /// The module's name.
    pub name: String,
    /// Its domain.
    pub domain: String,
    /// Its id.
    pub anchor: String,
    /// `:synopsis:`.
    pub synopsis: String,
    /// `:platform:`.
    pub platform: String,
    /// `:deprecated:`.
    pub deprecated: bool,
}

/// An index directive or role.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexInfo {
    /// One entry per line, as written.
    pub entries: String,
    /// The index node's own id, the target of entries without one.
    pub anchor: String,
    /// Per entry line, its target id (an object's or a term's), or empty.
    pub targets: String,
    /// The category key a glossary term gives its entries.
    pub key: String,
}

/// A numbered item: a figure, table, code block, or equation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NumberedInfo {
    /// What it is.
    pub kind: LabelKind,
    /// Its id.
    pub anchor: String,
    /// Every id it was read with, its anchor first: a label names any.
    pub ids: Vec<String>,
    /// The enclosing section.
    pub section: Option<u32>,
    /// How many of the document's toctrees come before it.
    pub toctrees: u32,
}

/// A reference a document makes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefInfo {
    /// The role, such as `ref` or `py:func`.
    pub reftype: String,
    /// The target as written.
    pub target: String,
}

/// A file a document was made from besides its source.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dependency {
    /// The file.
    pub path: PathBuf,
    /// The digest of its bytes when it was read.
    pub digest: Digest,
    /// Read by include as part of the document, not quoted.
    pub included: bool,
}

/// One todo, which a todolist copies from the document's object.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TodoInfo {
    /// The todo's node in the object.
    pub node: u32,
    /// Its id on the page; empty to link the page itself.
    pub anchor: String,
    /// Its line in the source.
    pub line: u32,
}

/// A file a build keeps below the doctrees folder, named by the digest of its bytes
/// (an object, a document's search words).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CachedFile {
    /// The path below the doctrees folder, with `/` separators.
    pub path: String,
    /// The digest of its bytes, checked when it is read back.
    pub digest: Digest,
}

/// What reading learned about one document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocInfo {
    /// Its name.
    pub docname: String,
    /// Its path below the source folder, with its suffix.
    pub relative: String,
    /// Its markup.
    pub kind: SourceKind,
    /// The digest of its source text.
    pub digest: Digest,
    /// The files it was made from besides its source.
    pub deps: Vec<Dependency>,
    /// Its parsed tree; `None` when it failed.
    pub object: Option<CachedFile>,
    /// Its title, as text.
    pub title: String,
    /// Its sections, in order.
    pub sections: Vec<SectionInfo>,
    /// Its toctrees, in order.
    pub toctrees: Vec<ToctreeInfo>,
    /// Its object descriptions' table of contents entries.
    pub toc_objects: Vec<TocObjectInfo>,
    /// Its labels.
    pub labels: Vec<LabelInfo>,
    /// Its glossary terms.
    pub terms: Vec<TermInfo>,
    /// The objects it describes.
    pub objects: Vec<ObjectInfo>,
    /// Its index entries.
    pub index: Vec<IndexInfo>,
    /// Its numbered items, in order.
    pub numbered: Vec<NumberedInfo>,
    /// The images it shows, as written.
    pub images: Vec<String>,
    /// The files it offers for download, as written.
    pub downloads: Vec<String>,
    /// The references it makes.
    pub refs: Vec<RefInfo>,
    /// Its todos.
    pub todos: Vec<TodoInfo>,
    /// The page lists every todo (a todolist), so any todo changes it.
    pub todolist: bool,
    /// meta directives: the field as written, then the content.
    pub meta: Vec<(String, String)>,
    /// The modules it declares.
    pub modules: Vec<ModuleInfo>,
    /// Its C++ declarations, for the project's symbol tree.
    pub cpp: Vec<CppDeclInfo>,
    /// Its words for the search index; `None` when it has none.
    pub search: Option<CachedFile>,
    /// `:orphan:`: no toctree need reach it.
    pub orphan: bool,
    /// `:tocdepth:`; 0 for none.
    pub tocdepth: u32,
    /// Reading reported errors, or it could not be read: it has no object.
    pub failed: bool,
    /// A generated page: its text is made, not read.
    pub generated: bool,
    /// What reading it reported, given again when it is reused.
    pub reports: Vec<Report>,
    /// This build wrote its object and words: the commit flushes them. Not saved.
    pub written: bool,
}

/// The environment: every document's `DocInfo`, sorted by docname, and the digest of
/// everything besides the sources that changes reading.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Environment {
    /// The reading settings it was made with (`reading_settings` and more).
    pub settings: Digest,
    /// The documents, sorted by docname.
    pub docs: Vec<DocInfo>,
}

impl Environment {
    /// An empty environment for `settings`.
    pub fn new(settings: Digest) -> Environment {
        Environment { settings, docs: Vec::new() }
    }

    /// The document named `docname`.
    pub fn find(&self, docname: &str) -> Option<&DocInfo> {
        self.docs.binary_search_by(|d| d.docname.as_str().cmp(docname)).ok().map(|i| &self.docs[i])
    }

    /// The id of the document named `docname`.
    pub fn id_of(&self, docname: &str) -> Option<DocId> {
        let i = self.docs.binary_search_by(|d| d.docname.as_str().cmp(docname)).ok()?;
        Some(DocId(i as u32))
    }

    /// The document `id` names. Panics when `id` is not of this environment, an
    /// internal invariant: ids come from it.
    pub fn doc(&self, id: DocId) -> &DocInfo {
        &self.docs[id.index()]
    }

    /// The documents with their ids.
    pub fn iter(&self) -> impl Iterator<Item = (DocId, &DocInfo)> {
        self.docs.iter().enumerate().map(|(i, d)| (DocId(i as u32), d))
    }

    /// The environment as the JSON the build saves.
    pub fn to_json(&self) -> String {
        records::environment_to_json(self)
    }

    /// Reads an environment the build saved; `None` for another version or anything
    /// that is not one.
    pub fn from_json(text: &str) -> Option<Environment> {
        records::environment_from_json(text)
    }
}

/// The folder of the build's cache: `<build_dir>/.doctrees`.
pub fn doctrees_dir(config: &Config) -> PathBuf {
    config.build_dir.join(".doctrees")
}

/// Where the environment is saved.
pub fn environment_path(config: &Config) -> PathBuf {
    doctrees_dir(config).join("environment.json")
}

/// A docname with the start of a digest and a suffix: `guide/intro.0123456789abcdef.gdo`.
pub fn digest_name(docname: &str, digest: &Digest, suffix: &str) -> String {
    format!("{docname}.{}{suffix}", digest.short())
}

/// Loads the saved environment at `path`, read within `budget`. Returns an empty one
/// for `settings` when there is none, it is from another version, its settings differ
/// (a fresh build), or `fresh` is set (`-E`). Fails only when the budget cannot hold
/// the file.
pub fn load_environment(
    path: &Path,
    settings: Digest,
    fresh: bool,
    budget: &Budget,
) -> Result<Environment, Problem> {
    let empty = Environment::new(settings);
    if fresh || !path.is_file() {
        return Ok(empty);
    }
    let ledger = Ledger::new(budget);
    let bytes = match gd_host::read_file(path, Some(&ledger), usize::MAX) {
        Ok(bytes) => bytes,
        Err(p) if p.code == "host.budget" => return Err(p),
        Err(_) => return Ok(empty),
    };
    let Ok(text) = std::str::from_utf8(&bytes) else { return Ok(empty) };
    match Environment::from_json(text) {
        Some(env) if env.settings == settings => Ok(env),
        _ => Ok(empty),
    }
}

/// Whether a file `doc` depended on changed: each is hashed as it is read in chunks
/// (`file_digest`), so none is held in memory. A file that cannot be read counts as
/// changed.
pub fn dependencies_changed(doc: &DocInfo) -> bool {
    doc.deps.iter().any(|dep| crate::file_digest(&dep.path) != Some(dep.digest))
}
