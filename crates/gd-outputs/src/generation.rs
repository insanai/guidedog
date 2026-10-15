//! A build writes its output as one generation (generation.odin).
//!
//! Every file it changes goes first to the next folder, a sibling of the output folder
//! (`.<name>.next`); a file whose bytes are already in place is only recorded. Only
//! when the build succeeds is the next folder completed with the rest of the output and
//! put in place of the output folder, whole (`commit`). A failed build, including one
//! `-W` fails, leaves the previous output exactly as it was, with the environment and
//! the manifest that describe it.
//!
//! `Generation::reuse` is the single cutoff point of incremental writing: a unit whose
//! inputs digest is unchanged and whose every recorded lookup still gets the same
//! answer keeps its files and reports its diagnostics again
//! (docs/manual/internals/incremental.rst).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gd_env::{Asset, Digest, Lookup};
use gd_host::{Budget, Problem};
use gd_report::Report;

use crate::manifest::{Manifest, OutputRecord};
use crate::paths::OutputPaths;

/// How a generation begins.
#[derive(Debug, Clone, Copy)]
pub struct GenerationOptions<'a> {
    /// `-a`: reuse no unit; write everything.
    pub all: bool,
    /// The build's budget, which the previous manifest is read within.
    pub budget: &'a Budget,
}

/// The output being written. Dropping it without [`Generation::commit`] abandons it:
/// the previous output, manifest and environment stay as they were (the next folder
/// stays as the spare output the next build completes).
#[derive(Debug)]
pub struct Generation {
    paths: OutputPaths,
    all: bool,
    previous: Manifest,
    current: Manifest,
    staged: HashSet<String>,
    failed: usize,
    unsettled: bool,
}

impl Generation {
    /// Starts a generation in `paths`. First finishes or undoes any commit an earlier
    /// build left decided but unsettled (its journal), adding what that did to
    /// `reports`; then clears what a build that failed or stopped left staged, and the
    /// stage files a killed build left in the doctrees folder; then loads the previous
    /// manifest (a missing or older one is empty).
    ///
    /// Fails when an interrupted commit can be neither finished nor undone (the
    /// problem says how to settle it; nothing is touched), or when the budget cannot
    /// hold the manifest.
    pub fn begin(
        paths: OutputPaths,
        options: GenerationOptions<'_>,
        reports: &mut Vec<Report>,
    ) -> Result<Generation, Problem> {
        let _ = (paths, options, reports);
        todo!("wave 1: port begin_generation and finish_interrupted_commits")
    }

    /// The paths of this generation.
    pub fn paths(&self) -> &OutputPaths {
        &self.paths
    }

    /// Keeps the unit `key` of the previous generation when its inputs digest is
    /// `inputs`, each of its recorded lookups still gets the answer it recorded
    /// (`answer`, gd-resolve's `lookup_answer` for the unit's page), and its files are
    /// still in place. A kept unit joins this generation as it was; its record is
    /// returned so the caller reports its diagnostics again and copies its assets.
    /// `None` (write the unit) under `-a`, for a new unit, or when anything differs.
    pub fn reuse(
        &mut self,
        key: &str,
        inputs: Digest,
        answer: impl Fn(&Lookup) -> Digest,
    ) -> Option<&OutputRecord> {
        let _ = (key, inputs, answer);
        todo!("wave 1: port reuse_unit with per-lookup answer digests")
    }

    /// Starts writing the unit `key`: the files it emits are recorded for it. Finish
    /// it with [`Unit::finish`]; a unit dropped unfinished counts as failed, and a
    /// generation with a failed unit is not published.
    pub fn unit(&mut self, key: &str) -> Unit<'_> {
        Unit { generation: self, key: key.to_string(), files: Vec::new(), done: false }
    }

    /// How many units could not be written.
    pub fn failed_count(&self) -> usize {
        self.failed
    }

    /// Whether a decided commit is neither finished nor undone (the next build settles
    /// it).
    pub fn is_unsettled(&self) -> bool {
        self.unsettled
    }

    /// The files a unit of this generation, or of the previous one, already makes:
    /// the paths a plugin asset must not take (case-insensitively, or as a folder).
    pub fn taken_path(&self, rel: &str) -> Option<&str> {
        let _ = rel;
        todo!("wave 1: port plugin_asset_taken")
    }
}

/// The record a finished unit gives: everything but its files, which it emitted.
#[derive(Debug, Clone, Default)]
pub struct UnitRecord {
    /// The digest of its inputs, lookups excluded.
    pub inputs: Digest,
    /// The images and downloads it needs copied.
    pub assets: Vec<Asset>,
    /// What its references looked up, with the digest of each answer now.
    pub lookups: Vec<(Lookup, Digest)>,
    /// What writing it reported.
    pub reports: Vec<Report>,
}

/// One output unit being written. Paths are below the output folder, with `/`
/// separators; a path that leaves it is the problem `build.output` (exit 5).
#[derive(Debug)]
pub struct Unit<'g> {
    generation: &'g mut Generation,
    key: String,
    files: Vec<String>,
    done: bool,
}

impl Unit<'_> {
    /// Writes a file of the unit. A file whose bytes are already in place is only
    /// recorded, so unchanged files keep their times; the file in place is compared in
    /// chunks, never read whole. Others go to the next folder, unflushed (the commit
    /// flushes them).
    pub fn emit(&mut self, rel: &str, data: &[u8]) -> Result<(), Problem> {
        let _ = (rel, data, &self.generation, &self.key, &mut self.files);
        todo!("wave 1: port emit and write_staged")
    }

    /// Writes a text file of the unit, as [`Unit::emit`].
    pub fn emit_text(&mut self, rel: &str, text: &str) -> Result<(), Problem> {
        self.emit(rel, text.as_bytes())
    }

    /// Copies the file `from` into the unit at `rel`, read within `budget`.
    pub fn emit_copy(&mut self, from: &Path, rel: &str, budget: &Budget) -> Result<(), Problem> {
        let _ = (from, rel, budget);
        todo!("wave 1: port emit_copy")
    }

    /// Copies the folder `from` into the unit at `rel`, leaving out hidden names. A
    /// folder or file outside the canonical `roots` (as named, or through a link) is
    /// left out with a `build.output.outside` or `build.output.link` warning added to
    /// `reports` (`gd_conf::unread_warning`).
    pub fn emit_tree(
        &mut self,
        from: &Path,
        rel: &str,
        roots: &[PathBuf],
        budget: &Budget,
        reports: &mut Vec<Report>,
    ) -> Result<(), Problem> {
        let _ = (from, rel, roots, budget, reports);
        todo!("wave 1: port emit_tree")
    }

    /// Where a producer that writes its own file (Typst) writes the file `rel` of the
    /// unit: a fresh path in the next folder with nothing there, which never reaches a
    /// file the spare output shares with the output.
    pub fn stage_path(&mut self, rel: &str) -> Result<PathBuf, Problem> {
        let _ = rel;
        todo!("wave 1: port stage_path and clear_way")
    }

    /// Records a file of the output that is already as this generation would write it
    /// (a message template that differs only in its creation date).
    pub fn keep_file(&mut self, rel: &str) {
        self.files.push(rel.to_string());
    }

    /// Records the unit as written, with its record.
    pub fn finish(mut self, record: UnitRecord) {
        let _ = record;
        self.done = true;
        todo!("wave 1: port end_unit")
    }

    /// Records the unit as not written: the generation will not be published.
    pub fn fail(mut self) {
        self.done = true;
        self.generation.failed += 1;
    }
}

impl Drop for Unit<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.generation.failed += 1;
        }
    }
}
