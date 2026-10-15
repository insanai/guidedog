//! A build writes its output as one generation.
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
use gd_host::{Budget, Ledger, PathKind, Problem};
use gd_report::Report;

use crate::commit::abandon_paths;
use crate::manifest::{Manifest, OutputRecord, SITE_UNIT};
use crate::paths::{OutputPaths, unit_path};
use crate::publish::finish_interrupted_commits;
use crate::tree::{make_writable, remove_spare};

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
    pub(crate) paths: OutputPaths,
    all: bool,
    pub(crate) previous: Manifest,
    pub(crate) current: Manifest,
    pub(crate) staged: HashSet<String>,
    pub(crate) failed: usize,
    pub(crate) unsettled: bool,
    /// Units besides the site unit written, and kept, this generation.
    written: usize,
    reused: usize,
    site_written: bool,
}

impl Generation {
    /// Starts a generation in `paths`. First finishes or undoes any commit an earlier
    /// build left decided but unsettled (its journal), adding what that did to
    /// `reports`; then clears what a build that failed or stopped left staged, and the
    /// stage files a killed build left in the doctrees folder; then loads the previous
    /// manifest (a missing or older one is empty). Last, the spare and previous outputs
    /// an earlier version kept beside a custom output move into its cache folder, or
    /// go, when they are clearly Guidedog's (a note says which).
    ///
    /// Fails when an interrupted commit can be neither finished nor undone (the
    /// problem says how to settle it; nothing is touched), or when the budget cannot
    /// hold the manifest.
    pub fn begin(
        paths: OutputPaths,
        options: GenerationOptions<'_>,
        reports: &mut Vec<Report>,
    ) -> Result<Generation, Problem> {
        finish_interrupted_commits(&paths.doctrees, options.budget, reports)?;
        abandon_paths(&paths);
        // Every commit is settled, so no stage file is in use: those a killed build
        // left in the doctrees folder go. Those in the next folder go when the commit
        // completes it.
        gd_host::remove_stages(&paths.doctrees);
        let previous = Manifest::load(&paths.manifest, options.budget)?;
        let generation = Generation {
            paths,
            all: options.all,
            previous,
            current: Manifest::default(),
            staged: HashSet::new(),
            failed: 0,
            unsettled: false,
            written: 0,
            reused: 0,
            site_written: false,
        };
        generation.migrate_sibling_spares(reports);
        Ok(generation)
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
        if self.all {
            return None;
        }
        let old = self.previous.units.get(key)?;
        // A zero digest marks a unit an unreadable journal left untrusted.
        if old.inputs != inputs || old.inputs == Digest::default() {
            return None;
        }
        if old.lookups.iter().any(|(lookup, digest)| answer(lookup) != *digest) {
            return None;
        }
        if old.files.iter().any(|f| !self.paths.out_dir.join(f).is_file()) {
            return None;
        }
        let kept = old.clone();
        if key != SITE_UNIT {
            self.reused += 1;
        }
        self.current.units.insert(key.to_string(), kept);
        self.current.units.get(key)
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

    /// Fails with [`pages_not_written`] when a unit could not be written; the reports say
    /// why, and the previous output stays in place.
    pub fn check_written(&self) -> Result<(), Problem> {
        match self.failed {
            0 => Ok(()),
            failed => Err(pages_not_written(failed, &self.paths().out_dir)),
        }
    }

    /// How many units besides the site unit ([`SITE_UNIT`]) were written this
    /// generation: the pages, books or indexes the build's summary counts.
    pub fn written_count(&self) -> usize {
        self.written
    }

    /// How many units besides the site unit were kept from the previous generation.
    pub fn reused_count(&self) -> usize {
        self.reused
    }

    /// Whether the site unit was written (a builder whose whole output is that unit,
    /// such as singlehtml or gettext, writes only it).
    pub fn is_site_written(&self) -> bool {
        self.site_written
    }

    /// The files a unit of this generation, or of the previous one, already makes:
    /// the paths a plugin asset must not take (case-insensitively, or as a folder).
    pub fn taken_path(&self, rel: &str) -> Option<&str> {
        let units = self.current.units.values().chain(self.previous.units.values());
        let mut files = units.flat_map(|unit| unit.files.iter());
        files.find(|f| paths_conflict(f, rel)).map(String::as_str)
    }

    /// Writes a file of this build into the next folder. The next folder is the spare
    /// output, so a file or folder of an earlier output may be in the way, or a folder
    /// a user made read-only: then the folders on the way are made writable, the way
    /// is cleared (`clear_way`), and the file written again.
    fn write_staged(&self, rel: &str, data: &[u8]) -> Result<(), Problem> {
        let path = self.paths.next.join(rel);
        if gd_host::write_unsynced(&path, data).is_ok() {
            return Ok(());
        }
        let mut folder = path.parent();
        while let Some(dir) = folder {
            if gd_host::path_type(dir).ok().flatten() == Some(PathKind::Folder) {
                make_writable(dir);
            }
            if dir == self.paths.next {
                break;
            }
            folder = dir.parent();
        }
        self.clear_way(rel)?;
        gd_host::write_unsynced(&path, data)
    }

    /// Makes `rel` in the next folder free for a file of this build: every folder on
    /// the way is a folder, and nothing is at `rel`. What is removed is stale, from an
    /// earlier output; only a file or folder this build wrote there is a conflict.
    fn clear_way(&self, rel: &str) -> Result<PathBuf, Problem> {
        let next = &self.paths.next;
        let staged = next.join(rel);
        for (i, _) in rel.match_indices('/') {
            let folder = next.join(&rel[..i]);
            let kind =
                gd_host::path_type(&folder).map_err(|e| gd_host::io_problem(&folder, &e))?;
            match kind {
                None => break,
                Some(PathKind::Folder) => continue,
                Some(kind) => {
                    if self.staged.contains(&rel[..i]) {
                        return Err(self.written_twice(&rel[..i]));
                    }
                    remove_spare(&folder, kind).map_err(|e| gd_host::io_problem(&folder, &e))?;
                    break;
                }
            }
        }
        if let Some(parent) = staged.parent() {
            std::fs::create_dir_all(parent).map_err(|e| gd_host::io_problem(parent, &e))?;
        }
        let kind = gd_host::path_type(&staged).map_err(|e| gd_host::io_problem(&staged, &e))?;
        if let Some(kind) = kind {
            let below = |other: &String| {
                other.len() > rel.len()
                    && other.starts_with(rel)
                    && other[rel.len()..].starts_with('/')
            };
            if kind == PathKind::Folder && self.staged.iter().any(below) {
                return Err(self.written_twice(rel));
            }
            remove_spare(&staged, kind).map_err(|e| gd_host::io_problem(&staged, &e))?;
        }
        Ok(staged)
    }

    fn written_twice(&self, rel: &str) -> Problem {
        Problem::new(
            "build.output",
            "A FILE AND A FOLDER AT THE SAME PATH",
            &self.paths.out_dir.join(rel).display().to_string(),
            format!("This build writes a file at {rel} and files below it."),
            "Rename one of them: a page, and a file or folder copied by html_static_path or \
             html_extra_path, share the path.",
            1,
        )
    }

    fn outside_problem(&self, rel: &str) -> Problem {
        Problem::new(
            "build.output",
            "OUTPUT OUTSIDE THE OUTPUT FOLDER",
            rel,
            format!("A file would be written outside {}.", self.paths.out_dir.display()),
            "Report this as a Guidedog bug, with the command you ran and the output of \
             guidedog --version.",
            5,
        )
    }
}

/// Whether two paths of the output name the same file, ignoring case, or one is a
/// folder of the other.
fn paths_conflict(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    let (shorter, longer) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    longer
        .strip_prefix(shorter.as_str())
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
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
        let rel = self.file_path(rel)?;
        let g = &mut *self.generation;
        let current = if g.staged.contains(&rel) {
            g.paths.next.join(&rel)
        } else {
            g.paths.out_dir.join(&rel)
        };
        let unchanged = gd_host::same_contents(&current, data);
        self.files.push(rel.clone());
        if unchanged {
            return Ok(());
        }
        g.staged.insert(rel.clone());
        g.write_staged(&rel, data) // flushed by the commit.
    }

    /// Writes a text file of the unit, as [`Unit::emit`].
    pub fn emit_text(&mut self, rel: &str, text: &str) -> Result<(), Problem> {
        self.emit(rel, text.as_bytes())
    }

    /// Copies the file `from` into the unit at `rel`, read within `budget`.
    pub fn emit_copy(&mut self, from: &Path, rel: &str, budget: &Budget) -> Result<(), Problem> {
        let ledger = Ledger::new(budget);
        let data = gd_host::read_file(from, Some(&ledger), usize::MAX)?;
        self.emit(rel, &data)
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
        if !from.is_dir() {
            return Ok(());
        }
        let why = gd_host::confinement(roots, from);
        if let Some(w) =
            gd_conf::unread_warning("build.output", "FOLDER NOT COPIED", from, "folder", why)
        {
            reports.push(w);
            return Ok(());
        }
        // A work list of (folder, its path in the unit), depth first in name order.
        let mut pending = vec![(from.to_path_buf(), rel.to_string())];
        while let Some((dir, target)) = pending.pop() {
            let mut entries =
                gd_host::list_folder(&dir).map_err(|e| gd_host::io_problem(&dir, &e))?;
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            let mut folders = Vec::new();
            for e in entries {
                let name = e.name.to_string_lossy();
                if name.starts_with('.') {
                    continue;
                }
                let path = dir.join(&e.name);
                let is_dir = path.is_dir();
                let what = if is_dir { "folder" } else { "file" };
                let why = gd_host::confinement(roots, &path);
                if let Some(w) =
                    gd_conf::unread_warning("build.output", "FILE NOT COPIED", &path, what, why)
                {
                    reports.push(w);
                    continue;
                }
                let inner =
                    if target.is_empty() { name.into_owned() } else { format!("{target}/{name}") };
                if is_dir {
                    folders.push((path, inner));
                } else {
                    self.emit_copy(&path, &inner, budget)?;
                }
            }
            pending.extend(folders.into_iter().rev());
        }
        Ok(())
    }

    /// Where a producer that writes its own file (Typst) writes the file `rel` of the
    /// unit: a fresh path in the next folder with nothing there, which never reaches a
    /// file the spare output shares with the output.
    pub fn stage_path(&mut self, rel: &str) -> Result<PathBuf, Problem> {
        let rel = self.file_path(rel)?;
        let staged = self.generation.clear_way(&rel)?;
        self.files.push(rel.clone());
        self.generation.staged.insert(rel);
        Ok(staged)
    }

    /// Records a file of the output that is already as this generation would write it
    /// (a message template that differs only in its creation date).
    pub fn keep_file(&mut self, rel: &str) {
        if let Some(rel) = unit_path(rel) {
            self.files.push(rel);
        }
    }

    /// The files this unit has emitted or recorded so far, normalized, in order.
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// The file this unit has emitted that `rel` would conflict with: the same file
    /// ignoring case, or one is a folder of the other.
    pub fn own_path(&self, rel: &str) -> Option<&str> {
        self.files.iter().find(|f| paths_conflict(f, rel)).map(String::as_str)
    }

    /// The files this unit, or another of its generation or the previous one, already
    /// makes that `rel` would conflict with, as [`Generation::taken_path`].
    pub fn taken_path(&self, rel: &str) -> Option<&str> {
        self.own_path(rel).or_else(|| self.generation.taken_path(rel))
    }

    /// Records the unit as written, with its record.
    pub fn finish(mut self, record: UnitRecord) {
        self.done = true;
        let unit = OutputRecord {
            inputs: record.inputs,
            files: std::mem::take(&mut self.files),
            assets: record.assets,
            lookups: record.lookups,
            reports: record.reports,
        };
        if self.key == SITE_UNIT {
            self.generation.site_written = true;
        } else {
            self.generation.written += 1;
        }
        self.generation.current.units.insert(std::mem::take(&mut self.key), unit);
    }

    /// Records the unit as not written: the generation will not be published.
    pub fn fail(mut self) {
        self.done = true;
        self.generation.failed += 1;
    }

    fn file_path(&self, rel: &str) -> Result<String, Problem> {
        unit_path(rel).ok_or_else(|| self.generation.outside_problem(rel))
    }
}

impl Drop for Unit<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.generation.failed += 1;
        }
    }
}

/// `build.output`: `failed` documents could not be written to `out_dir`, so the build's
/// output was not published; the previous output is unchanged.
pub fn pages_not_written(failed: usize, out_dir: &Path) -> Problem {
    Problem::new(
        "build.output",
        "PAGES NOT WRITTEN",
        &out_dir.display().to_string(),
        format!(
            "{failed} document(s) could not be written, so this build's output was not \
             published. The previous output is unchanged."
        ),
        "Fix the errors reported above for each page, then run the build again.",
        1,
    )
}
