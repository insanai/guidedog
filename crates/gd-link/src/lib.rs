//! gd-link: the link pass of a Guidedog build, and rendering a linked page.
//!
//! Writing a page loads the document's cached object, runs the link pass over it, and
//! renders the result. The pass rewrites the tree for one output: cross-references
//! become links (`gd_resolve::resolve_xref` answers each), toctrees become navigation,
//! `only` keeps or drops its content, images and graphs get the URLs the output finds
//! them at, todolists collect the project's todos, discussion metadata gets readable
//! names, and sections and figures get their numbers.
//!
//! ```no_run
//! # fn page(view: &gd_resolve::BuildView<'_>, doc: gd_env::DocId) {
//! use gd_link::{PageRequest, Worker, render_options, render_page};
//! let mut worker = Worker::new(); // one per rendering thread
//! let format = gd_env::OutputFormat::Html;
//! let request = PageRequest { doc, format, renderer: "html",
//!     options: render_options(view, doc, format), ..PageRequest::default() };
//! let page = render_page(view, &mut worker, &request);
//! println!("{} bytes, {} assets, failed: {}", page.body.len(), page.assets.len(), page.failed);
//! # }
//! ```
//!
//! Docs: docs/manual/internals/project.rst. Nothing here prints; problems are reports
//! in the `Rendered` page.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod discussion;
mod extlinks;
mod graph;
mod graph_files;
mod graph_links;
mod graph_theme;
mod image;
mod mounts;
mod nav;
mod only;
mod page;
mod posts;
mod publication;
mod render;
mod rewrite;
mod source_links;
mod todolist;
mod xref;

pub use discussion::{Discussion, discussion_field, discussion_prefix, discussion_record};
pub use graph_theme::{
    ADAPTIVE_CLASS, AdaptError, GRAPH_ADAPTIVE_CSS, adapt_graph, graph_adaptive,
};
pub use image::{book_path, book_root};
pub use nav::{
    NavItem, NavOptions, TocChild, doc_limit, nav_for_toctree, only_shown, section_number,
    toc_order, toctree_entries, top_sections, untitled, within,
};
pub use only::{Tags, evaluate_only};
pub use render::{PageRequest, Worker, render_options, render_page, source_location};
pub use rewrite::{Action, Rewriter, Rule, rewrite_document};
pub use source_links::{MODULES_FOLDER, module_page, viewcode_pages};
pub use todolist::todo_order;
