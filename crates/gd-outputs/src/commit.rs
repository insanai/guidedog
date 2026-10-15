//! Publishing a generation whole, or none of it (commit.odin, publish.odin).
//!
//! Committing completes the next folder into the whole next output (hard links to the
//! unchanged files), writes the manifest and the environment beside their
//! destinations, flushes everything the commit will publish, and then decides the
//! commit by saving a journal (`<doctrees>/<builder>.commit.json`). Then, in order: the
//! output folder and the next are swapped in one step where the system can
//! (`gd_host::exchange`), in two renames elsewhere; the records are renamed over their
//! destinations; the names are flushed; the journal is removed. Readers of the output
//! folder see the previous output or the new one, never a mix. A build that finds a
//! journal finishes or undoes that commit before anything else (`Generation::begin`).

use gd_env::{Environment, ObjectStore};
use gd_host::Problem;
use gd_report::Report;

use crate::generation::Generation;

/// The commit journal's format version.
pub const JOURNAL_VERSION: u32 = 2;
/// The suffix of a builder's journal in the doctrees folder.
pub const JOURNAL_SUFFIX: &str = ".commit.json";
/// The suffix a record is written with beside its destination.
pub const PENDING_SUFFIX: &str = ".next";
/// The suffix of the link to the record a commit replaces, kept until it is done.
pub const PREVIOUS_SUFFIX: &str = ".previous";

/// What a commit publishes besides the output: the environment (`environment.json`),
/// and the cached objects and search words it names (those this build wrote, marked
/// `DocInfo::written`, are flushed first; those it no longer names are swept after).
#[derive(Debug, Clone, Copy)]
pub struct CommitRecords<'a> {
    /// The environment this build read.
    pub environment: &'a Environment,
    /// The store its objects and search words are in.
    pub store: &'a ObjectStore,
}

/// What a commit left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitState {
    /// The new output is published.
    Published,
    /// The previous output is kept: nothing was published, or a failed commit was
    /// undone.
    #[default]
    Kept,
    /// A decided commit is neither finished nor undone; the next build settles it.
    Unsettled,
}

/// The result of a commit.
#[derive(Debug, Clone, Default)]
pub struct CommitOutcome {
    /// What it left.
    pub state: CommitState,
    /// Why it did not publish; `None` when it did.
    pub problem: Option<Problem>,
    /// What it warned about (a previous output it could not remove, an undone commit).
    pub reports: Vec<Report>,
}

impl Generation {
    /// Publishes the generation, its manifest and `records`. Only what changed is
    /// published: a build that changed no file and no record renames nothing, and one
    /// that changed only records leaves the output folder in place. A failure before
    /// the commit is decided leaves everything as it was (`Kept`); one after it is
    /// undone, or left `Unsettled` with a problem that says how to settle it.
    pub fn commit(self, records: CommitRecords<'_>) -> CommitOutcome {
        let _ = records;
        todo!("wave 1: port commit_generation, plan_commit and finish_commit")
    }

    /// Drops what this build prepared for a commit it will not make: records written
    /// beside their destinations, and the links to the records they replace. The next
    /// folder stays as the spare output. Nothing is touched while a decided commit is
    /// unsettled. Dropping a generation does the same.
    pub fn abandon(self) {
        drop(self);
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        // wave 1: port abandon_generation (keep_spare, remove_previous_outputs, pending
        // records); a generation that was committed has nothing left to drop.
    }
}
