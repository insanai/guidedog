//! Completing the next folder into the whole next output (generation.odin's
//! `complete_next`).
//!
//! The next folder is the spare output: after a switch it holds the output the switch
//! replaced, which shares every unchanged file with the new one (they are hard links to
//! the same files), and the next build writes its changes into it and completes it. So
//! a commit costs what changed, and listing the folders, not a link or a removal for
//! every file of the site. Nothing in the spare output is trusted: a file is kept only
//! when it is the same file (by inode) as the one in the output, or this build wrote
//! it; everything else in it is replaced or removed.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gd_host::{FolderEntry, PathKind, Problem, io_problem};

use crate::generation::Generation;
use crate::tree::remove_spare;

/// What completing the next folder changed: the paths whose names changed (their
/// folders are flushed), and the files copied where they could not be linked (flushed
/// too).
#[derive(Debug, Default)]
pub(crate) struct Completed {
    pub changed: Vec<PathBuf>,
    pub copied: Vec<PathBuf>,
}

/// What completing the next folder works from.
struct NextOutput<'g> {
    generation: &'g Generation,
    obsolete: &'g HashSet<String>,
    /// Folders that hold a file this build wrote.
    staged_dirs: HashSet<String>,
    done: Completed,
}

/// Makes the next folder the whole next output: the files this build wrote, and every
/// file of the output in place that is not `obsolete`, linked, or copied where it
/// cannot be. A file of the output in place that Guidedog did not make, where the next
/// output has a folder, or below a path where it has a file, is in the way: the build
/// is not published (`build.publish`). Only names are listed: no file is read.
pub(crate) fn complete_next(
    generation: &Generation,
    obsolete: &HashSet<String>,
) -> Result<Completed, Problem> {
    let mut staged_dirs = HashSet::with_capacity(2 * generation.staged.len());
    for rel in &generation.staged {
        for (i, _) in rel.match_indices('/') {
            staged_dirs.insert(rel[..i].to_string());
        }
    }
    let mut n = NextOutput { generation, obsolete, staged_dirs, done: Completed::default() };
    let next = &generation.paths.next;
    std::fs::create_dir_all(next).map_err(|e| io_problem(next, &e))?;
    let out_dir = &generation.paths.out_dir;
    let out = gd_host::path_type(out_dir).map_err(|e| io_problem(out_dir, &e))?;
    n.complete_folder("", out.is_some().then_some(out_dir.as_path()), next)?;
    Ok(n.done)
}

/// One name of the next output: the entry of the output in place (`current`) and what
/// the next folder has there (`spare`); at least one of them is there.
struct Pair<'e> {
    current: Option<&'e FolderEntry>,
    spare: Option<&'e FolderEntry>,
}

impl NextOutput<'_> {
    /// Makes the folder `to` of the next output (`rel` below the output folder) match
    /// the folder `from` of the output in place (`None` when it has none), keeping what
    /// this build wrote there. Returns how many entries `to` holds after.
    fn complete_folder(
        &mut self,
        rel: &str,
        from: Option<&Path>,
        to: &Path,
    ) -> Result<usize, Problem> {
        let current = match from {
            Some(dir) => gd_host::list_folder(dir).map_err(|e| io_problem(dir, &e))?,
            None => Vec::new(),
        };
        let spare = gd_host::list_folder(to).map_err(|e| io_problem(to, &e))?;
        let index: HashMap<&OsString, usize> =
            spare.iter().enumerate().map(|(i, s)| (&s.name, i)).collect();
        let mut seen = vec![false; spare.len()];
        let mut kept = 0;
        for e in &current {
            let s = index.get(&e.name).map(|&i| {
                seen[i] = true;
                &spare[i]
            });
            if self.complete_entry(rel, from, to, Pair { current: Some(e), spare: s })? {
                kept += 1;
            }
        }
        for (s, _) in spare.iter().zip(&seen).filter(|(_, seen)| !**seen) {
            if self.complete_entry(rel, None, to, Pair { current: None, spare: Some(s) })? {
                kept += 1;
            }
        }
        Ok(kept)
    }

    /// Makes one name of the next output; returns whether the name is in it after.
    fn complete_entry(
        &mut self,
        rel: &str,
        from: Option<&Path>,
        to: &Path,
        pair: Pair<'_>,
    ) -> Result<bool, Problem> {
        let Some(name) = pair.current.or(pair.spare).map(|e| &e.name) else { return Ok(false) };
        let text = name.to_string_lossy();
        let r = if rel.is_empty() { text.into_owned() } else { format!("{rel}/{text}") };
        let dst = to.join(name);
        match (pair.current, from) {
            (Some(e), Some(from)) if e.kind == PathKind::Folder => {
                self.complete_folder_entry(&r, &from.join(name), &dst, pair.spare)
            }
            (Some(e), Some(from)) => {
                self.complete_file_entry(&r, e, &from.join(name), &dst, pair.spare)
            }
            _ => self.complete_spare_entry(&r, &dst, pair.spare),
        }
    }

    /// A name only in the next folder: this build wrote it, or it is stale.
    fn complete_spare_entry(
        &mut self,
        r: &str,
        dst: &Path,
        spare: Option<&FolderEntry>,
    ) -> Result<bool, Problem> {
        let Some(s) = spare else { return Ok(false) };
        if s.kind == PathKind::Folder && self.staged_dirs.contains(r) {
            return Ok(self.complete_folder(r, None, dst)? > 0);
        }
        if s.kind != PathKind::Folder && self.generation.staged.contains(r) {
            return Ok(true);
        }
        self.drop_stale(dst, s.kind)?;
        Ok(false)
    }

    /// A folder of the output in place.
    fn complete_folder_entry(
        &mut self,
        r: &str,
        src: &Path,
        dst: &Path,
        spare: Option<&FolderEntry>,
    ) -> Result<bool, Problem> {
        if let Some(s) = spare.filter(|s| s.kind != PathKind::Folder) {
            if self.generation.staged.contains(r) {
                self.check_replaced(r, src)?; // a folder becomes a file.
                return Ok(true);
            }
            self.drop_stale(dst, s.kind)?;
        }
        if spare.is_none_or(|s| s.kind != PathKind::Folder) {
            std::fs::create_dir(dst).map_err(|e| io_problem(dst, &e))?;
            self.done.changed.push(dst.to_path_buf());
        }
        let kept = self.complete_folder(r, Some(src), dst)?;
        // A folder nothing is left in goes, unless it was empty in the output too.
        if kept == 0 && !folder_was_empty(src) {
            self.drop_stale(dst, PathKind::Folder)?;
            return Ok(false);
        }
        Ok(true)
    }

    /// A file (or link) of the output in place.
    fn complete_file_entry(
        &mut self,
        r: &str,
        e: &FolderEntry,
        src: &Path,
        dst: &Path,
        spare: Option<&FolderEntry>,
    ) -> Result<bool, Problem> {
        if self.generation.staged.contains(r) {
            return Ok(true); // this build's file, already there.
        }
        let obsolete = self.obsolete.contains(r);
        if spare.is_some_and(|s| s.kind == PathKind::Folder) && self.staged_dirs.contains(r) {
            // A file becomes a folder.
            if !obsolete {
                return Err(self.in_the_way(r, "folder"));
            }
            return Ok(self.complete_folder(r, None, dst)? > 0);
        }
        if obsolete {
            if let Some(s) = spare {
                self.drop_stale(dst, s.kind)?;
            }
            return Ok(false);
        }
        if let Some(s) = spare {
            if s.kind == e.kind && s.inode == e.inode && e.inode != 0 {
                return Ok(true);
            }
            self.drop_stale(dst, s.kind)?;
        }
        let copied = gd_host::link_file(src, dst, e.kind).map_err(|err| io_problem(dst, &err))?;
        if copied {
            self.done.copied.push(dst.to_path_buf());
        }
        self.done.changed.push(dst.to_path_buf());
        Ok(true)
    }

    /// Removes a stale name from the next folder, and notes the change.
    fn drop_stale(&mut self, path: &Path, kind: PathKind) -> Result<(), Problem> {
        remove_spare(path, kind).map_err(|e| io_problem(path, &e))?;
        self.done.changed.push(path.to_path_buf());
        Ok(())
    }

    /// Checks a folder of the output in place whose path the next output gives to a
    /// file: every file below it must be obsolete, so none is lost.
    fn check_replaced(&self, rel: &str, from: &Path) -> Result<(), Problem> {
        let mut pending = vec![(rel.to_string(), from.to_path_buf())];
        while let Some((rel, dir)) = pending.pop() {
            let entries = gd_host::list_folder(&dir).map_err(|e| io_problem(&dir, &e))?;
            for e in entries {
                let r = format!("{rel}/{}", e.name.to_string_lossy());
                if e.kind == PathKind::Folder {
                    pending.push((r, dir.join(&e.name)));
                } else if !self.obsolete.contains(&r) {
                    return Err(self.in_the_way(&r, "file"));
                }
            }
        }
        Ok(())
    }

    fn in_the_way(&self, rel: &str, what: &str) -> Problem {
        let path = self.generation.paths.out_dir.join(rel).display().to_string();
        Problem::new(
            "build.publish",
            "A FILE IN THE OUTPUT IS IN THE WAY",
            &path,
            format!(
                "This build writes a {what} where the output folder has {rel}, which \
                 Guidedog did not make, so the output was not published."
            ),
            format!(
                "Move or remove {path}, or run `guidedog clean` to empty the build folder, \
                 then build again."
            ),
            1,
        )
    }
}

/// Whether a folder of the output in place has no entries: such a folder is kept, as
/// its maker left it.
fn folder_was_empty(dir: &Path) -> bool {
    gd_host::list_folder(dir).is_ok_and(|entries| entries.is_empty())
}
