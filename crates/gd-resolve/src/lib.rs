//! gd-resolve: the project tables and cross-reference resolution of a Guidedog build,
//! without the document tree. `resolve` joins every document's symbols (`Tables`),
//! `load_intersphinx` loads other projects' inventories, `BuildView` borrows everything
//! a builder reads, `resolve_xref` answers one reference (noting its lookups in a
//! `Trace`), `lookup_answer` re-answers a recorded lookup for incremental builds, and
//! `create_index`, `module_index` and `write_inventory` give the indexes' data.
//! `OutputBuilder` is the trait the builder crates implement.
//!
//! ```no_run
//! # fn link(view: &gd_resolve::BuildView<'_>, from: gd_env::DocId) {
//! use gd_resolve::{XrefQuery, resolve_xref};
//! let mut trace = gd_env::Trace::new();
//! let query = XrefQuery { from, reftype: "ref".into(), target: "install".into(),
//!     ..XrefQuery::default() };
//! match resolve_xref(view, &query, &mut trace) {
//!     Ok(to) => println!("{:?} #{}", to.doc, to.anchor),
//!     Err(why) if why.warn => eprintln!("{why}"),
//!     Err(_) => {}
//! }
//! # }
//! ```
//!
//! Docs: docs/manual/internals/project.rst (the crate split) and incremental.rst (what
//! each lookup's answer covers). Nothing here prints; warnings are `gd_report::Report`s.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod cpp;
mod cpp_lookup;
mod external;
mod index;
mod intersphinx;
mod inventory;
mod local;
mod lookups;
mod modindex;
mod numbering;
mod odin_xref;
mod rust_xref;
mod suggest;
mod tables;
mod toctree;
mod view;
mod xref;

pub use cpp::{CppDeclaration, CppSymbol, CppSymbolId, CppTree};
pub use index::{IndexGroup, IndexLink, IndexSub, IndexTerm, create_index};
pub use intersphinx::{Intersphinx, IntersphinxOptions, load_intersphinx};
pub use inventory::write_inventory;
pub use local::option_key;
pub use modindex::{ModindexEntry, ModindexLetter, has_module_index, module_index};
pub use tables::{Tables, TargetRef, TreeEntry, normalize_name, object_key, resolve};
pub use view::{
    BuildView, Dates, ImageFile, OutputBuilder, chapter_key, doc_target, file_target,
    format_number, generated_page, normalize_doc, python_format, relative_url,
};
pub use xref::{Resolved, Unresolved, XrefQuery, lookup_answer, resolve_xref, role_takes};

#[cfg(test)]
mod tests;
