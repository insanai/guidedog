//! Moving the spare output of a custom output into its cache folder.
//!
//! Earlier versions kept a custom output's spare output and previous output beside it
//! (`.<name>.next`, `.<name>.old`); now they live in its cache folder
//! (`.<name>.guidedog`), so the output leaves one hidden folder beside it. A build finds
//! the old folders and moves the spare output to its new place, or removes it, but only
//! when it is clearly Guidedog's: a folder whose every file is one Guidedog made for
//! this output. A folder that only looks like one is left as it is.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gd_host::{PathKind, is_stage_name};
use gd_report::Report;

use crate::generation::Generation;
use crate::tree::{remove_spare, unlock};

impl Generation {
    /// Moves or removes the spare and previous outputs an earlier version left beside
    /// a custom output, adding a note for each to `reports`. Errors leave the folder as
    /// it is: the next build tries again.
    pub(crate) fn migrate_sibling_spares(&self, reports: &mut Vec<Report>) {
        for old in self.paths.sibling_spares() {
            if gd_host::path_type(&old).ok().flatten().is_none() {
                continue;
            }
            if !self.is_own_folder(&old) {
                reports.push(foreign_note(&old));
                continue;
            }
            if let Some(note) = self.retire(&old) {
                reports.push(note);
            }
        }
    }

    /// Makes an old-layout folder the spare output when there is none, or removes it.
    fn retire(&self, old: &Path) -> Option<Report> {
        let next = &self.paths.next;
        let has_spare = gd_host::path_type(next).ok().flatten().is_some();
        if !has_spare && std::fs::rename(old, next).is_ok() {
            return Some(moved_note(old, next));
        }
        unlock(old);
        remove_spare(old, PathKind::Folder).ok()?;
        Some(removed_note(old))
    }

    /// Whether `dir` is a folder Guidedog made for this output: every file in it is a
    /// stage file, a file the previous manifest names, or the same file (by inode) as
    /// the output's file at its path. Only names are listed: no file is read.
    fn is_own_folder(&self, dir: &Path) -> bool {
        if gd_host::path_type(dir).ok().flatten() != Some(PathKind::Folder) {
            return false;
        }
        let made: HashSet<&str> = self
            .previous
            .units
            .values()
            .flat_map(|unit| unit.files.iter().map(String::as_str))
            .collect();
        let mut pending = vec![(String::new(), dir.to_path_buf())];
        while let Some((rel, folder)) = pending.pop() {
            let Ok(entries) = gd_host::list_folder(&folder) else { return false };
            let output = self.output_inodes(&rel);
            for e in entries {
                let name = e.name.to_string_lossy();
                let r = if rel.is_empty() { name.to_string() } else { format!("{rel}/{name}") };
                if e.kind == PathKind::Folder {
                    pending.push((r, folder.join(&e.name)));
                    continue;
                }
                let same = e.inode != 0 && output.get(name.as_ref()) == Some(&e.inode);
                if !(is_stage_name(&name) || made.contains(r.as_str()) || same) {
                    return false;
                }
            }
        }
        true
    }

    /// The inodes of the files in the output's folder `rel`, by name.
    fn output_inodes(&self, rel: &str) -> HashMap<String, u64> {
        let dir: PathBuf = self.paths.out_dir.join(rel);
        let entries = gd_host::list_folder(&dir).unwrap_or_default();
        let files = entries.into_iter().filter(|e| e.kind != PathKind::Folder);
        files.map(|e| (e.name.to_string_lossy().into_owned(), e.inode)).collect()
    }
}

fn moved_note(old: &Path, next: &Path) -> Report {
    let message = format!(
        "The spare output an earlier version kept at {} now lives at {}, in the output's \
         cache folder.",
        old.display(),
        next.display()
    );
    Report::note("build.output.layout", "SPARE OUTPUT MOVED", &message)
        .hint("Nothing to do: the output leaves one hidden folder beside it from now on.")
}

fn removed_note(old: &Path) -> Report {
    let message = format!(
        "{} held an output an earlier version left beside this one, and was removed.",
        old.display()
    );
    Report::note("build.output.layout", "OLD OUTPUT REMOVED", &message)
        .hint("Nothing to do: the output's cache folder holds such folders from now on.")
}

fn foreign_note(old: &Path) -> Report {
    let message = format!(
        "{} has the name of a folder earlier versions kept beside the output, but holds \
         files Guidedog did not make for it, so it was left as it is.",
        old.display()
    );
    Report::note("build.output.layout", "FOLDER BESIDE THE OUTPUT KEPT", &message).hint(
        "Remove it if it is an old Guidedog output; if it is yours, rename it to stop this note.",
    )
}
