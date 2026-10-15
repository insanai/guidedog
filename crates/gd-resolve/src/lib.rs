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
//! Wave 0 of the port: the types and signatures are final; the bodies are ported from
//! resolve.odin, numbering.odin, lookups.odin, xref.odin, cpp_*.odin, odin_xref.odin,
//! intersphinx*.odin, inventory.odin, genindex.odin and modindex.odin in wave 1.

// Wave 0 holds signatures only; the fields are used once wave 1 ports the bodies.
#![allow(dead_code)]

mod cpp;
mod index;
mod intersphinx;
mod tables;
mod view;
mod xref;

pub use cpp::{CppDeclaration, CppSymbol, CppSymbolId, CppTree};
pub use index::{
    IndexGroup, IndexLink, IndexSub, IndexTerm, ModindexEntry, ModindexLetter, create_index,
    has_module_index, module_index, write_inventory,
};
pub use intersphinx::{Intersphinx, IntersphinxOptions, load_intersphinx};
pub use tables::{Tables, TargetRef, TreeEntry, object_key, resolve};
pub use view::{
    BuildView, Dates, OutputBuilder, doc_target, file_target, generated_page, normalize_doc,
    relative_url,
};
pub use xref::{Resolved, Unresolved, XrefQuery, lookup_answer, resolve_xref, role_takes};
