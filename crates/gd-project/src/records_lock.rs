//! The lock a build holds on its records folder (GDS 0020), so that two builds never
//! write one output and its records at once.
//!
//! A build's records folder holds its environment, objects, manifests and commit journal;
//! a second build of the same output would mix its files with the first's, and the
//! recovery of an interrupted commit would undo a commit that is still running. The lock
//! is `gd_plugins::files::lock_exclusive` (GDS 0019): a build that finds it held stops
//! with `build.busy` before it writes anything.

use std::path::Path;

use gd_host::{Problem, io_problem};
use gd_plugins::files::{FileLock, LockError, lock_exclusive};

/// The lock file's name in the records folder.
pub const RECORDS_LOCK: &str = "build.lock";

/// Takes the lock of the records folder `dir`, making the folder when it is missing.
/// Returns the lock, held until it is dropped. Fails with `build.busy` (exit 4) when
/// another build holds it, and `host.io` when the folder or the lock file fails.
pub(crate) fn lock_records(dir: &Path) -> Result<FileLock, Problem> {
    std::fs::create_dir_all(dir).map_err(|e| io_problem(dir, &e))?;
    let path = dir.join(RECORDS_LOCK);
    lock_exclusive(&path).map_err(|e| match e {
        LockError::Held => busy(dir),
        LockError::Io(e) => io_problem(&path, &e),
    })
}

fn busy(dir: &Path) -> Problem {
    Problem::new(
        "build.busy",
        "ANOTHER BUILD WRITES THIS OUTPUT",
        &dir.display().to_string(),
        "Another guidedog build holds the lock of this output's records folder.",
        "Wait for that build to finish, or build into another folder.",
        4,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_build_of_one_output_is_busy() {
        let root = gd_plugins::files::make_temp_dir(&std::env::temp_dir(), "records-lock")
            .unwrap_or_else(|e| panic!("{e}"));
        let dir = root.join(".html.guidedog");
        let held = lock_records(&dir).unwrap_or_else(|p| panic!("{p}"));
        let refused = lock_records(&dir).err().map(|p| (p.code, p.exit));
        assert_eq!(refused, Some(("build.busy", 4)));
        drop(held);
        assert!(lock_records(&dir).is_ok(), "the lock is free once released");
        let _ = std::fs::remove_dir_all(&root);
    }
}
