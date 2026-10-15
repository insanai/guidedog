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

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gd_env::{Environment, ObjectStore};
use gd_host::{PathKind, Problem, io_problem, remove_tree};
use gd_report::Report;

use crate::generation::Generation;
use crate::journal::{Journal, PendingFile};
use crate::next::{Completed, complete_next};
use crate::paths::{OutputPaths, contained};
use crate::publish::{finish_commit, remove_previous_output};

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

impl CommitOutcome {
    fn kept(problem: Problem) -> CommitOutcome {
        CommitOutcome { state: CommitState::Kept, problem: Some(problem), reports: Vec::new() }
    }
}

impl Generation {
    /// Publishes the generation, its manifest and `records`. Only what changed is
    /// published: a build that changed no file and no record renames nothing, and one
    /// that changed only records leaves the output folder in place. A failure before
    /// the commit is decided leaves everything as it was (`Kept`); one after it is
    /// undone, or left `Unsettled` with a problem that says how to settle it.
    pub fn commit(mut self, records: CommitRecords<'_>) -> CommitOutcome {
        if self.failed > 0 {
            return CommitOutcome::kept(failed_problem(self.failed));
        }
        let (mut journal, obsolete) = match self.prepare_commit(records.environment) {
            Ok(prepared) => prepared,
            Err(p) => return CommitOutcome::kept(p),
        };
        if self.staged.is_empty() && obsolete.is_empty() && journal.records.is_empty() {
            sweep(records);
            return CommitOutcome { state: CommitState::Published, ..CommitOutcome::default() };
        }
        if let Err(p) = self.decide(&mut journal, &obsolete, records) {
            return CommitOutcome::kept(p);
        }
        let mut reports = Vec::new();
        let (problem, state) = finish_commit(&journal, &self.paths.journal, &mut reports);
        self.unsettled = state == CommitState::Unsettled;
        if state == CommitState::Published {
            remove_previous_output(&journal, &mut reports);
            sweep(records);
        }
        CommitOutcome { state, problem, reports }
    }

    /// Completes the next folder, flushes everything the journal will publish, and
    /// saves the journal, durably. A failure leaves no journal.
    fn decide(
        &self,
        journal: &mut Journal,
        obsolete: &HashSet<String>,
        records: CommitRecords<'_>,
    ) -> Result<(), Problem> {
        let mut completed = Completed::default();
        if !self.staged.is_empty() || !obsolete.is_empty() {
            completed = complete_next(self, obsolete)?;
            let next = &self.paths.next;
            journal.next_id = gd_host::identity(next).map_err(|e| io_problem(next, &e))?;
            journal.next = Some(next.clone());
        }
        self.sync_generation(journal, &completed, records)?;
        let path = &self.paths.journal;
        let text = journal.to_json().ok_or_else(|| records_problem(path))?;
        let saved = gd_host::publish(path, text.as_bytes(), true);
        if saved.is_err() {
            let _ = std::fs::remove_file(path);
        }
        saved
    }

    /// Writes the records, lists the obsolete files, and starts the journal: where the
    /// output goes, and where the previous output goes once replaced. A previous output
    /// an earlier commit could not remove stays where it is, and this one goes beside
    /// it (old-2, old-3, ...).
    fn prepare_commit(
        &self,
        environment: &Environment,
    ) -> Result<(Journal, HashSet<String>), Problem> {
        let obsolete = self.obsolete_files();
        let records = self.write_records(environment)?;
        let mut old = self.paths.old.clone();
        for n in 2.. {
            if gd_host::path_type(&old).map_err(|e| io_problem(&old, &e))?.is_none() {
                break;
            }
            let mut name = self.paths.old.clone().into_os_string();
            name.push(format!("-{n}"));
            old = PathBuf::from(name);
        }
        let journal = Journal {
            out_dir: self.paths.out_dir.clone(),
            next: None,
            old,
            next_id: None,
            records,
        };
        Ok((journal, obsolete))
    }

    /// The files the previous generation made that this one did not. Files Guidedog
    /// did not make are kept in the next output.
    fn obsolete_files(&self) -> HashSet<String> {
        let keep: HashSet<&String> =
            self.current.units.values().flat_map(|unit| &unit.files).collect();
        let previous = self.previous.units.values().flat_map(|unit| &unit.files);
        previous.filter(|f| !keep.contains(f) && contained(f)).cloned().collect()
    }

    /// Writes the manifest and the environment beside their destinations, unflushed,
    /// and returns those it wrote, each with a link to the record it replaces: a record
    /// whose destination already holds exactly its bytes is left as it is.
    fn write_records(&self, environment: &Environment) -> Result<Vec<PendingFile>, Problem> {
        let texts = [self.current.to_json(), environment.to_json()];
        let mut out = Vec::with_capacity(texts.len());
        for (mut record, text) in pending_records(&self.paths).into_iter().zip(texts) {
            if gd_host::same_contents(&record.to, text.as_bytes()) {
                continue;
            }
            gd_host::write_unsynced(&record.from, text.as_bytes())?;
            let exists = gd_host::path_type(&record.to).map_err(|e| io_problem(&record.to, &e))?;
            match (&record.previous, exists) {
                (Some(previous), Some(_)) => {
                    let _ = std::fs::remove_file(previous);
                    gd_host::link_file(&record.to, previous, PathKind::File)
                        .map_err(|e| io_problem(previous, &e))?;
                }
                _ => record.previous = None,
            }
            out.push(record);
        }
        Ok(out)
    }

    /// Makes durable, before the journal is saved, everything the journal will
    /// publish: the files written for this generation (staged, copied, and the records
    /// written beside their destinations), the objects and search words this build
    /// wrote, and then every folder of the next output and the folders that hold the
    /// records, so the names are durable too. A file linked into the next output is
    /// the published file itself, already durable.
    fn sync_generation(
        &self,
        journal: &Journal,
        completed: &Completed,
        records: CommitRecords<'_>,
    ) -> Result<(), Problem> {
        let staged: Vec<PathBuf> = match &journal.next {
            Some(next) => self.staged.iter().map(|rel| next.join(rel)).collect(),
            None => Vec::new(),
        };
        let mut kept = Vec::new();
        for r in &journal.records {
            kept.push(r.from.clone());
            kept.extend(r.previous.clone());
        }
        for doc in records.environment.docs.iter().filter(|d| d.written) {
            let cached = doc.object.iter().chain(&doc.search);
            kept.extend(cached.map(|file| records.store.path_of(file)));
        }
        let files: Vec<PathBuf> =
            staged.iter().chain(&completed.copied).chain(&kept).cloned().collect();
        gd_host::sync_all(&files)?;
        let mut folders = Vec::new();
        if let Some(next) = &journal.next {
            let parent = next.parent().unwrap_or(Path::new("")).to_path_buf();
            let placed: Vec<PathBuf> = completed.changed.iter().chain(&staged).cloned().collect();
            folders.extend(gd_host::folders_of(&placed, &parent));
            folders.push(next.clone());
            folders.push(parent);
        }
        folders.extend(gd_host::folders_of(&kept, &self.paths.build_dir));
        gd_host::sync_all(&folders)
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        if !self.unsettled {
            abandon_paths(&self.paths);
        }
    }
}

impl Generation {
    /// Drops what this build prepared for a commit it will not make: records written
    /// beside their destinations, and the links to the records they replace. The next
    /// folder stays as the spare output. Nothing is touched while a decided commit is
    /// unsettled. Dropping a generation does the same.
    pub fn abandon(self) {
        drop(self);
    }
}

/// Drops what a build prepared for a commit it did not decide, or that is settled: the
/// records written beside their destinations, and the links to the records they
/// replaced. The next folder stays, as the spare output; a previous output a commit
/// replaced in two renames becomes the spare output when there is none, and is removed
/// otherwise. The output in place and its records stay as they are.
pub(crate) fn abandon_paths(paths: &OutputPaths) {
    let _ = remove_tree(&paths.legacy);
    keep_spare(&paths.next, &paths.old);
    remove_previous_outputs(&paths.old);
    for record in pending_records(paths) {
        let _ = std::fs::remove_file(&record.from);
        if let Some(previous) = &record.previous {
            let _ = std::fs::remove_file(previous);
        }
    }
}

/// Makes a previous output at `old` the spare output at `next`, when there is no spare
/// output.
pub(crate) fn keep_spare(next: &Path, old: &Path) {
    let has_next = gd_host::path_type(next);
    let has_old = gd_host::path_type(old);
    if let (Ok(None), Ok(Some(PathKind::Folder))) = (has_next, has_old) {
        let _ = std::fs::rename(old, next);
    }
}

/// Removes what earlier commits could not: the previous output at `old`, and those
/// they moved beside it (old-2, old-3, ...) because it was still there.
fn remove_previous_outputs(old: &Path) {
    let (Some(parent), Some(name)) = (old.parent(), old.file_name()) else { return };
    let name = name.to_string_lossy();
    let Ok(entries) = gd_host::list_folder(parent) else { return };
    for e in entries {
        let entry = e.name.to_string_lossy();
        let Some(rest) = entry.strip_prefix(name.as_ref()) else { continue };
        if rest.is_empty() || rest.starts_with('-') {
            let _ = remove_tree(&parent.join(&e.name));
        }
    }
}

/// Where a commit writes the manifest and the environment first, and where it keeps
/// the records they replace.
fn pending_records(paths: &OutputPaths) -> [PendingFile; 2] {
    let beside = |path: &Path, suffix: &str| {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    };
    let environment = paths.doctrees.join(format!("{}.environment.json", paths.builder()));
    [
        PendingFile {
            from: beside(&paths.manifest, PENDING_SUFFIX),
            to: paths.manifest.clone(),
            previous: Some(beside(&paths.manifest, PREVIOUS_SUFFIX)),
        },
        PendingFile {
            from: beside(&environment, PENDING_SUFFIX),
            to: paths.doctrees.join("environment.json"),
            previous: Some(beside(&environment, PREVIOUS_SUFFIX)),
        },
    ]
}

/// Removes the objects and search words the environment does not name: those of
/// documents that are gone or changed, and those a failed build wrote.
fn sweep(records: CommitRecords<'_>) {
    let docs = records.environment.docs.iter();
    let named = docs.flat_map(|d| d.object.iter().chain(&d.search)).map(|f| f.path.as_str());
    records.store.sweep(named);
}

pub(crate) fn records_problem(path: &Path) -> Problem {
    Problem::new(
        "build.output",
        "CANNOT SAVE THE BUILD RECORDS",
        &path.display().to_string(),
        "The build's records could not be encoded: a path in them is not Unicode.",
        "Move the project to a folder whose path is plain Unicode text, then build again.",
        5,
    )
}

fn failed_problem(failed: usize) -> Problem {
    let units =
        if failed == 1 { "One output unit".to_string() } else { format!("{failed} output units") };
    Problem::new(
        "build.output.failed",
        "THE OUTPUT WAS NOT WRITTEN WHOLE",
        "",
        format!(
            "{units} could not be written, so nothing was published: the previous output \
             and the build records are as they were."
        ),
        "Fix the errors reported for those pages, then build again.",
        1,
    )
}
