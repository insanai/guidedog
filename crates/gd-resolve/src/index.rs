//! The data of the general index and the module index (the models of genindex.odin
//! and modindex.odin), and objects.inv (inventory.odin). The pages are written by
//! gd-site; the PDF book's index by gd-book.

use gd_env::DocId;

use crate::view::BuildView;

/// A link of an index term to a page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexLink {
    /// A main entry (`!` in the index directive), shown in bold.
    pub main: bool,
    /// The URL from the output root (or the book's anchor key).
    pub uri: String,
}

/// A subterm of an index term.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexSub {
    /// Its text.
    pub name: String,
    /// Its links.
    pub links: Vec<IndexLink>,
}

/// A term of the general index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexTerm {
    /// Its text.
    pub name: String,
    /// The category key a glossary term gives; empty for none.
    pub key: String,
    /// Its links.
    pub links: Vec<IndexLink>,
    /// Its subterms, sorted.
    pub subs: Vec<IndexSub>,
}

/// A group of the general index: a letter, `_`, "Symbols" (translated), or a category
/// key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexGroup {
    /// The group's heading.
    pub name: String,
    /// Its terms, sorted as Sphinx sorts them.
    pub terms: Vec<IndexTerm>,
}

/// Collects the project's index entries into groups of sorted terms, as Sphinx's
/// `IndexEntries.create_index` does: entry types `single`, `pair`, `triple`, `see`,
/// `seealso` and the pair shorthands (`module:`, `keyword:`, ...); terms folded as
/// `f() (in module a)` under `f()`.
pub fn create_index(view: &BuildView<'_>) -> Vec<IndexGroup> {
    let _ = view;
    todo!("wave 1: port create_index")
}

/// One row of the module index: a module, or a package heading its submodules.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModindexEntry {
    /// The module's name.
    pub name: String,
    /// 0 a module, 1 a package heading its submodules, 2 a submodule.
    pub group: u8,
    /// The page that declares it; `None` for a package with no page of its own.
    pub doc: Option<DocId>,
    /// Its id on that page.
    pub anchor: String,
    /// `:platform:`.
    pub platform: String,
    /// `:deprecated:`.
    pub deprecated: bool,
    /// `:synopsis:`.
    pub synopsis: String,
}

/// The modules under one letter of the module index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModindexLetter {
    /// The letter.
    pub letter: String,
    /// Its rows, sorted by name, ignoring case.
    pub entries: Vec<ModindexEntry>,
}

/// The Python module index, by letter, `modindex_common_prefix` left out of names.
pub fn module_index(view: &BuildView<'_>) -> Vec<ModindexLetter> {
    let _ = view;
    todo!("wave 1: port module_index")
}

/// Whether the project has a module index: `html_domain_indices` is on and some
/// document declares a Python module.
pub fn has_module_index(view: &BuildView<'_>) -> bool {
    let _ = view;
    todo!("wave 1: port has_module_index")
}

/// Appends the project's `objects.inv` (Sphinx's version 2 format) to `out`: labels,
/// documents, terms and objects with their priorities (modules 0, other objects 1),
/// the C++ symbols with declarations, and the generated pages' labels.
pub fn write_inventory(view: &BuildView<'_>, out: &mut Vec<u8>) {
    let _ = (view, out);
    todo!("wave 1: port write_inventory with gd_inventory::write_inventory")
}
