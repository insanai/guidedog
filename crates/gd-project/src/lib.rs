//! The Guidedog project builder: a handful of verbs over a loaded project.
//!
//! ```no_run
//! use gd_project::{BuildOptions, BuilderKind, LoadOptions};
//!
//! let project = gd_project::load("docs".as_ref(), &LoadOptions::default())?;
//! let result = gd_project::build(&project, BuilderKind::Html, &BuildOptions::default());
//! for report in &result.reports { /* gd_report::Report */ }
//! if let Some(problem) = &result.problem { eprintln!("{problem}"); }
//! gd_project::clean(&project)?;
//! # Ok::<(), gd_host::Problem>(())
//! ```
//!
//! A build discovers the documents (and the generated API pages), loads the
//! translations, reads what changed since the last build (gd-read), resolves the
//! project (gd-resolve), writes the output with the builder (gd-site, gd-book or
//! gd-intl) into a gd-outputs generation, and publishes it whole.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod build;
mod duration;
mod finish;
mod limits;
mod project;
mod write;

pub use build::build;
pub use gd_env::{BuildOptions, BuildResult, BuildStage, BuilderKind, Jobs};
pub use limits::{build_limits, trusted_policy, untrusted_policy};
pub use project::{LoadOptions, Project, clean, load, watch_roots};

#[cfg(test)]
mod tests;
