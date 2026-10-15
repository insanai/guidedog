//! gd-outputs: how a Guidedog build writes and publishes its output. One `Generation`
//! per build and builder: units of output (`Unit`) written to a spare folder beside the
//! output, reused when their inputs and lookups are unchanged (`Generation::reuse`, the
//! cutoff point of incremental builds), and published whole by a journaled commit that
//! survives a crash (`Generation::commit`). Also `output_paths` and `clean`.
//!
//! ```no_run
//! # use gd_env::Environment;
//! # fn build(config: &gd_conf::Config, env: &Environment) -> Result<(), gd_host::Problem> {
//! use gd_outputs::{CommitRecords, Generation, GenerationOptions, UnitRecord, output_paths};
//! let budget = gd_host::Budget::new(gd_env::BUILD_BUDGET);
//! let paths = output_paths(config, gd_env::BuilderKind::Text, None)?;
//! let store = gd_env::ObjectStore::new(&paths.doctrees);
//! let mut reports = Vec::new();
//! let mut generation =
//!     Generation::begin(paths, GenerationOptions { all: false, budget: &budget }, &mut reports)?;
//! let inputs = gd_env::Digest::of_text("index source");
//! if generation.reuse("index", inputs, |_| gd_env::Digest::default()).is_none() {
//!     let mut unit = generation.unit("index");
//!     unit.emit_text("index.txt", "Hello\n")?;
//!     unit.finish(UnitRecord { inputs, ..UnitRecord::default() });
//! }
//! let outcome = generation.commit(CommitRecords { environment: env, store: &store });
//! # let _ = outcome;
//! # Ok(())
//! # }
//! ```
//!
//! Ported from generation.odin, commit.odin, publish.odin, build_paths.odin, remove.odin
//! and `clean` of build.odin; docs/manual/internals/publication.rst explains the commit.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod clean;
mod commit;
mod generation;
mod hook;
mod journal;
mod manifest;
mod migrate;
mod next;
mod paths;
mod publish;
mod tree;

#[cfg(test)]
mod tests;

pub use clean::clean;
pub use commit::{
    CommitOutcome, CommitRecords, CommitState, JOURNAL_SUFFIX, JOURNAL_VERSION, PENDING_SUFFIX,
    PREVIOUS_SUFFIX,
};
pub use generation::{Generation, GenerationOptions, Unit, UnitRecord, pages_not_written};
pub use manifest::{MANIFEST_VERSION, Manifest, OutputRecord, SITE_UNIT};
pub use paths::{OutputPaths, contained, output_absolute, output_paths};
