//! gd-read: incremental, parallel reading of a Guidedog project's documents. Each source
//! is read once; a document whose source, settings and dependencies did not change keeps
//! what the previous build recorded, the rest are parsed (the Sphinx layer for
//! reStructuredText, MyST for Markdown), translated (gd-i18n), cached as objects
//! (`gd_env::ObjectStore`), and their symbols collected into a `gd_env::DocInfo`.
//! Python autodoc and the odin domain's generating directives are served while reading;
//! the Odin and Rust API pages are generated documents (`api_sources`).
//!
//! ```no_run
//! # fn build(ctx: &gd_read::ReadContext<'_>, previous: gd_env::Environment)
//! # -> Result<(), gd_host::Problem> {
//! let mut reports = Vec::new();
//! let sources = gd_env::discover(ctx.config)?;
//! let pages = gd_read::api_sources(ctx.config, &sources, &mut reports);
//! let sources = gd_env::with_generated(sources, pages);
//! let outcome = gd_read::read_documents(ctx, previous, &sources)?;
//! println!("{} parsed, {} unchanged", outcome.read, outcome.reused);
//! # Ok(())
//! # }
//! ```
//!
//! Docs: docs/manual/internals/project.rst and incremental.rst ("The reading layer").
//! Nothing here prints; problems are `gd_host::Problem`s and `gd_report::Report`s.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod api;
mod autodoc;
mod collect;
mod config;
mod context;
mod files;
mod inheritance;
mod inheritance_graph;
mod messages;
mod odin;
mod parse;
mod read;
mod rustcache;
mod rustdoc;
mod search;
mod sources;
mod translate;

pub use api::api_sources;
pub use config::{myst_settings, sphinx_config};
pub use context::{ReadContext, ReadOutcome, ReadingTime};
pub use read::{read_documents, reread_document, stale_warning};
