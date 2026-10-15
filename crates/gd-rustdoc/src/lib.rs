//! Rust API documentation for Guidedog, the Rust counterpart of odindoc and Python
//! autodoc. A crate is read from source by static analysis (tree-sitter; nothing is
//! compiled or run) and written as reStructuredText of Guidedog's `rust` domain: one page
//! per module and an index page.
//!
//! ```no_run
//! let pages = gd_rustdoc::document_crate(std::path::Path::new("crates/gd-json"))?;
//! for page in &pages {
//!     println!("{}: {} bytes", page.path, page.text.len());
//! }
//! # Ok::<(), gd_rustdoc::Error>(())
//! ```
//!
//! The staged path is [`load_crate`] (source to [`Crate`], with load warnings) then
//! [`document`] ([`Crate`] to [`Page`]s, each with its own warnings). The README holds
//! the domain vocabulary the Sphinx reader implements.

#![forbid(unsafe_code)]

mod attrs;
mod diag;
mod emit;
mod inline;
mod load;
mod markdown;
mod model;
mod parse;
mod plan;
mod resolve;
mod sig;

pub use diag::{Diagnostic, Error, Position, Severity};
pub use load::load_crate;
pub use model::{
    Crate, FileId, Item, ItemId, ItemKind, Module, ModuleId, Qualifiers, SourceFile, Visibility,
};

use std::path::Path;

/// In which order a module page lists the items of one group (structs, functions, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemberOrder {
    /// As written in the source.
    #[default]
    Source,
    /// By name, as rustdoc lists them.
    Alphabetical,
}

/// What to document and the bounds on the work. Fill it with struct-update syntax:
/// `Options { include_private: true, ..Options::default() }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Document private items and modules too (as `--document-private-items`).
    pub include_private: bool,
    /// Document `#[doc(hidden)]` items too.
    pub document_hidden: bool,
    /// The crate's name; `None` takes it from `Cargo.toml` or the folder or file name.
    pub crate_name: Option<String>,
    /// The most items (members included) read from the source; past it a
    /// `rust.autodoc.limit` warning is given and the rest is left out.
    pub max_items: usize,
    /// The most source files read.
    pub max_files: usize,
    /// The largest source file read, in bytes; larger ones are left out with a warning.
    pub max_file_bytes: usize,
    /// The deepest syntax tree accepted; deeper files are left out with a warning.
    pub max_depth: usize,
    /// The order of items within each group of a module page.
    pub member_order: MemberOrder,
    /// Give every generated directive `:noindex:` (for a second copy of an API).
    pub noindex: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            include_private: false,
            document_hidden: false,
            crate_name: None,
            max_items: 100_000,
            max_files: 4_096,
            max_file_bytes: 4 * 1024 * 1024,
            max_depth: 512,
            member_order: MemberOrder::Source,
            noindex: false,
        }
    }
}

/// One generated reStructuredText page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    /// Its relative path with `/` separators and the `.rst` extension: `index.rst` for
    /// the index, `my_crate.rst` for the crate root, `my_crate/parser.rst` for a module.
    pub path: String,
    /// The Rust path of the module it documents (`my_crate::parser`); empty for the index.
    pub module: String,
    /// The page's reStructuredText.
    pub text: String,
    /// Problems met writing it: unresolved intra-doc links, the item limit.
    pub warnings: Vec<Diagnostic>,
}

/// Writes the pages of `krate` as `options` chooses: one per documented module (public
/// ones, or every one with `include_private`), plus `index.rst` listing them. Borrows the
/// crate; never fails, since what cannot be linked or shown becomes a page warning.
pub fn document(krate: &Crate, options: &Options) -> Vec<Page> {
    let plan = plan::plan(krate, options);
    emit::pages(krate, &plan, options)
}

/// Loads the crate at `path` (a folder with `Cargo.toml`, a folder of `.rs` files, or a
/// root `.rs` file) with default [`Options`] and writes its pages. The load warnings are
/// added to the first page's warnings. Fails only when the crate root cannot be read.
pub fn document_crate(path: &Path) -> Result<Vec<Page>, Error> {
    let options = Options::default();
    let krate = load_crate(path, &options)?;
    let mut pages = document(&krate, &options);
    if let Some(first) = pages.first_mut() {
        let mut warnings = krate.warnings;
        warnings.append(&mut first.warnings);
        first.warnings = warnings;
    }
    Ok(pages)
}
