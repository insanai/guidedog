//! One source update is one atomic rename; no multi-file journal is needed.

#[cfg(unix)]
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gd_host::{Problem, io_problem};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(crate) fn write_source(path: &Path, before: Option<&str>, after: &str) -> Result<(), Problem> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let lock_path = parent.join(".guidedog-ssg.lock");
    if std::fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.is_file()) {
        return Err(conflict(path, "The publication lock is not a regular file."));
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| io_problem(&lock_path, &e))?;
    lock.try_lock().map_err(|_| conflict(path, "Another publication command holds the lock."))?;
    if let Some(before) = before {
        let current = crate::lifecycle::read_source(path)?;
        if current != before {
            return Err(conflict(path, "The source changed while preparing the edit."));
        }
    } else if path.try_exists().map_err(|e| io_problem(path, &e))? {
        return Err(conflict(path, "The destination already exists."));
    }
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let temporary = Temporary(parent.join(format!(".ssg-{}-{serial}.tmp", std::process::id())));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.0)
        .map_err(|e| io_problem(&temporary.0, &e))?;
    if before.is_some() {
        let permissions = std::fs::metadata(path).map_err(|e| io_problem(path, &e))?.permissions();
        file.set_permissions(permissions).map_err(|e| io_problem(path, &e))?;
    }
    file.write_all(after.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| io_problem(&temporary.0, &e))?;
    drop(file);
    if before.is_some() {
        std::fs::rename(&temporary.0, path).map_err(|e| io_problem(path, &e))?;
    } else {
        // Unlike rename, a hard link cannot overwrite an existing destination.
        std::fs::hard_link(&temporary.0, path).map_err(|e| io_problem(path, &e))?;
    }
    drop(temporary);
    sync_directory(parent)?;
    drop(lock);
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), Problem> {
    #[cfg(unix)]
    File::open(path).and_then(|f| f.sync_all()).map_err(|e| io_problem(path, &e))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn conflict(path: &Path, message: &str) -> Problem {
    Problem::new(
        "ssg.conflict",
        "SOURCE UPDATE REFUSED",
        &path.display().to_string(),
        message,
        "Keep the existing source, resolve the conflict, then retry.",
        2,
    )
}
