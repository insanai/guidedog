//! Atomic publication: every output is written to a staging file beside its
//! destination, flushed, and renamed over it, so the destination holds its old bytes or
//! the new ones, never a part; a failure leaves the prior artifact as it was.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::problem::{Problem, io_problem};
use crate::workers::run_workers;

/// Begins the name of every staging file: then ten digits, then the destination's
/// extension (`.guidedog-stage-0123456789.html`).
pub const STAGE_PREFIX: &str = ".guidedog-stage-";

const STAGE_DIGITS: usize = 10;

/// How many flushes `sync_all` keeps in flight; storage completes several at once far
/// sooner than one after another.
const SYNC_WORKERS: usize = 8;

/// A staging file beside a destination, for output written in place by another
/// producer (Typst) or by `publish`. Dropping it removes the file unless it was
/// committed, so an abandoned stage leaves nothing behind.
#[derive(Debug)]
pub struct Stage {
    path: PathBuf,
    committed: bool,
}

impl Stage {
    /// Creates an empty staging file in the folder of `destination`, making the folders
    /// on its way. Fails with `host.io`.
    pub fn beside(destination: &Path) -> Result<Stage, Problem> {
        let folder = destination.parent().unwrap_or(Path::new(""));
        if !folder.as_os_str().is_empty() {
            // Other threads may make the same folders at once; create_dir_all copes.
            fs::create_dir_all(folder).map_err(|e| io_problem(folder, &e))?;
        }
        let extension = destination.extension().map(|e| e.to_string_lossy());
        let mut last = io::Error::from(io::ErrorKind::AlreadyExists);
        for _ in 0..16 {
            let mut name = format!("{STAGE_PREFIX}{:010}", stage_digits());
            if let Some(ext) = &extension {
                name.push('.');
                name.push_str(ext);
            }
            let path = folder.join(name);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Stage { path, committed: false }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = e,
                Err(e) => return Err(io_problem(destination, &e)),
            }
        }
        Err(io_problem(destination, &last))
    }

    /// The staging file's path, for a producer to write to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Flushes the staging file and renames it over `destination`, then flushes the
    /// folder so the name is durable. An existing destination is refused
    /// (`host.output.exists`) unless `force` is set; on any failure the destination is
    /// unchanged and the stage removed.
    pub fn commit(mut self, destination: &Path, force: bool) -> Result<(), Problem> {
        if !force && fs::symlink_metadata(destination).is_ok() {
            return Err(exists_problem(destination));
        }
        let file = File::options().write(true).open(&self.path);
        file.and_then(|f| f.sync_all()).map_err(|e| io_problem(destination, &e))?;
        fs::rename(&self.path, destination).map_err(|e| io_problem(destination, &e))?;
        self.committed = true;
        if let Some(folder) = destination.parent() {
            sync_path(folder).map_err(|e| io_problem(folder, &e))?;
        }
        Ok(())
    }

    /// Renames the staging file over `destination` without flushing anything.
    fn rename_unsynced(mut self, destination: &Path) -> Result<(), Problem> {
        fs::rename(&self.path, destination).map_err(|e| io_problem(destination, &e))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Ten pseudo-random decimal digits: std's random hasher keys mixed with a counter.
fn stage_digits() -> u64 {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let mut hasher = std::hash::RandomState::new().build_hasher();
    hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    hasher.finish() % 10_000_000_000
}

pub(crate) fn exists_problem(path: &Path) -> Problem {
    Problem::new(
        "host.output.exists",
        "OUTPUT ALREADY EXISTS",
        &path.display().to_string(),
        "The destination file exists and was left unchanged.",
        "Pass --force to replace it, or choose another --output path.",
        2,
    )
}

/// Writes `data` to `path` through a flushed staging file renamed over it, making the
/// folders on its way. An existing `path` is refused (`host.output.exists`) unless
/// `force` is set; any failure leaves `path` unchanged.
pub fn publish(path: &Path, data: &[u8], force: bool) -> Result<(), Problem> {
    if !force && fs::symlink_metadata(path).is_ok() {
        return Err(exists_problem(path));
    }
    let stage = Stage::beside(path)?;
    write_stage(&stage, path, data)?;
    stage.commit(path, force)
}

fn write_stage(stage: &Stage, path: &Path, data: &[u8]) -> Result<(), Problem> {
    let write = || File::options().write(true).open(stage.path())?.write_all(data);
    write().map_err(|e| io_problem(path, &e))
}

/// Writes `data` as `publish` does (replacing `path`), but flushes nothing: a
/// publication of many files writes them all, then makes them durable in one pass
/// (`sync_all` of the files and their `folders_of`).
pub fn write_unsynced(path: &Path, data: &[u8]) -> Result<(), Problem> {
    let stage = Stage::beside(path)?;
    write_stage(&stage, path, data)?;
    stage.rename_unsynced(path)
}

/// Flushes files and folders to storage, several at once, and returns the first that
/// failed. A folder is flushed so the names in it are durable (skipped on Windows,
/// which keeps them durable itself); a path that is gone or a link has nothing to flush.
pub fn sync_all(paths: &[PathBuf]) -> Result<(), Problem> {
    let next = AtomicUsize::new(0);
    let failed: Mutex<Option<(usize, io::Error)>> = Mutex::new(None);
    run_workers(SYNC_WORKERS.min(paths.len()), |_| {
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(path) = paths.get(i) else { return };
            if let Err(e) = sync_path(path) {
                let mut first = failed.lock().unwrap_or_else(|p| p.into_inner());
                if first.as_ref().is_none_or(|(j, _)| i < *j) {
                    *first = Some((i, e));
                }
            }
        }
    });
    match failed.into_inner().unwrap_or_else(|p| p.into_inner()) {
        Some((i, e)) => Err(io_problem(&paths[i], &e)),
        None => Ok(()),
    }
}

/// Flushes each path in turn on the calling thread, as `sync_all` does with threads.
pub fn sync_each(paths: &[PathBuf]) -> Result<(), Problem> {
    for path in paths {
        sync_path(path).map_err(|e| io_problem(path, &e))?;
    }
    Ok(())
}

/// Flushes one file, or one folder's names; nothing for a link or a missing path.
pub(crate) fn sync_path(path: &Path) -> io::Result<()> {
    let info = match fs::symlink_metadata(path) {
        Ok(info) => info,
        Err(e) if is_not_found(&e) => return Ok(()),
        Err(e) => return Err(e),
    };
    if info.file_type().is_symlink() || (cfg!(windows) && info.is_dir()) {
        return Ok(());
    }
    File::open(path)?.sync_all()
}

/// Whether an error says nothing is at a path: it does not exist, or a folder on the way
/// is a file.
pub(crate) fn is_not_found(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory)
}

/// Lists, once each and sorted, the folders that hold `paths` and those between them
/// and `top` (included): the folders whose names a publication adds or changes. A path
/// not below `top` gives only its own folder.
pub fn folders_of(paths: &[PathBuf], top: &Path) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    for path in paths {
        let mut folder = path.parent();
        while let Some(dir) = folder {
            if !seen.insert(dir.to_path_buf()) || dir == top || !dir.starts_with(top) {
                break;
            }
            folder = dir.parent();
        }
    }
    seen.into_iter().collect()
}

/// Reports whether `name` is one `Stage` gives a staging file: `STAGE_PREFIX`, ten
/// digits, then nothing or one extension. Any other name, however close, is not ours.
pub fn is_stage_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(STAGE_PREFIX) else { return false };
    let Some(digits) = rest.get(..STAGE_DIGITS) else { return false };
    if !digits.bytes().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let ext = &rest[STAGE_DIGITS..];
    ext.is_empty() || (ext.starts_with('.') && !ext[1..].contains('.'))
}

/// Removes, in `dir` and every folder below it, the staging files a stopped process
/// (killed, or the machine lost power) left behind: regular files whose names
/// `is_stage_name` accepts. Everything else stays; a folder that cannot be listed is
/// left as it is. The caller makes sure no stage below `dir` is still in use.
pub fn remove_stages(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            remove_stages(&entry.path());
        } else if kind.is_file() && entry.file_name().to_str().is_some_and(is_stage_name) {
            let _ = fs::remove_file(entry.path());
        }
    }
}
