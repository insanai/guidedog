//! The project tables: every document's symbols joined, its toctrees resolved, its
//! sections and items numbered (resolve.odin, numbering.odin, cpp_symbols.odin).

use std::collections::HashMap;

use gd_conf::Config;
use gd_env::{DocId, Environment, LabelKind, UiText};
use gd_report::Report;

use crate::cpp::CppTree;

/// Where a name (label, term, object) points.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TargetRef {
    /// The document it is in.
    pub doc: DocId,
    /// Its id on the page; empty for the document itself.
    pub anchor: String,
    /// The section title or caption a reference shows; may be empty.
    pub title: String,
    /// What a label names.
    pub kind: LabelKind,
    /// An object's type; empty for labels and terms.
    pub objtype: String,
    /// An object's canonical name (`:canonical:`), not the object's own.
    pub aliased: bool,
}

/// A resolved toctree entry: a document of the project, a generated page, or a URL.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TreeEntry {
    /// The document; `None` for a URL or a generated page.
    pub doc: Option<DocId>,
    /// A URL, or a generated page's file name relative to the output root.
    pub url: String,
    /// The explicit title; empty for none.
    pub title: String,
    /// The general index, the module index, or the search page.
    pub generated: bool,
}

/// The project-wide tables. Per-document tables are indexed by `DocId::index`; names
/// are the only keyed lookups.
#[derive(Debug, Clone, Default)]
pub struct Tables {
    /// Document ids by docname.
    pub by_name: HashMap<String, DocId>,
    /// Reading (toctree) order from the root document.
    pub order: Vec<DocId>,
    /// Per document: the document whose toctree reached it first; `None` for the root
    /// and unreached ones.
    pub parent: Vec<Option<DocId>>,
    /// Per document, per toctree: its resolved entries.
    pub trees: Vec<Vec<Vec<TreeEntry>>>,
    /// Per document, per section: its number (`"1.2"`) or empty.
    pub secnumbers: Vec<Vec<String>>,
    /// Per document, per numbered item: its number (`"3"`, `"1.2"`) or empty.
    pub fignumbers: Vec<Vec<String>>,
    /// Labels by normalized name.
    pub labels: HashMap<String, TargetRef>,
    /// Glossary terms by lowercased name.
    pub terms: HashMap<String, TargetRef>,
    /// Objects by key (`object_key`): `domain:fullname`, `std:<type>:<name>`.
    pub objects: HashMap<String, TargetRef>,
    /// The C++ domain's symbols.
    pub cpp: CppTree,
    /// Per document: its anchor namespace in the book and singlehtml
    /// (`install-guide` for `install/guide`, numbered when two names make one key).
    pub keys: Vec<String>,
}

impl Tables {
    /// The id of the document named `docname`.
    pub fn doc_named(&self, docname: &str) -> Option<DocId> {
        self.by_name.get(docname).copied()
    }
}

/// Joins every document's symbols into the project tables: resolves each toctree
/// (relative names against the document's folder, `/name` against the source folder,
/// globs against every docname, `genindex`/`modindex`/`search` as generated pages
/// titled with `ui`), walks them from the root document (order, parents, cycles),
/// registers labels, terms and objects as Sphinx's domains do (duplicates warned about,
/// the later one winning), builds the C++ symbol tree, warns about documents no
/// toctree reaches, and numbers sections and figures. The API index joins the root's
/// first toctree when `odin_autoapi_add_toctree_entry` or `rust_autoapi_add_toctree_entry`
/// asks. Warnings go to `reports`; it never fails.
pub fn resolve(
    config: &Config,
    env: &Environment,
    ui: &dyn UiText,
    reports: &mut Vec<Report>,
) -> Tables {
    let _ = (config, env, ui, reports);
    todo!("wave 1: port resolve.odin, numbering.odin and cpp_symbols.odin")
}

/// Files an object for lookup. Standard objects other than environment variables and
/// configuration values are filed by type, as Sphinx keys them by (objtype, name):
/// `std:option:rm -r`, `std:token:python-grammar:expr`, declared types; every other
/// object is `domain:fullname`.
pub fn object_key(domain: &str, objtype: &str, fullname: &str) -> String {
    let _ = (domain, objtype, fullname);
    todo!("wave 1: port object_key")
}
