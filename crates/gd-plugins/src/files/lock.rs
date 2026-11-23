//! An exclusive lock on a lock file, released by an explicit unlock (GDS 0019).
//!
//! The lock is the system's advisory lock on an open file: `flock` on Unix and
//! `LockFileEx` on Windows, through [`std::fs::File::try_lock`]. The system releases
//! such a lock by itself only when the last descriptor of the open file closes. A child
//! process holds a copy of every descriptor from its creation until it starts its
//! program, so closing our descriptor is not a release: a child that another thread
//! created meanwhile keeps the lock for an instant, and the next taker would see a lock
//! nobody holds. An explicit unlock releases the lock for every copy at once, so
//! [`FileLock`] unlocks when dropped, before its file closes.
//!
//! The comments name the transitions of the model in GDS 0019 (`openFile`, `acquire`,
//! `refuse`, `unlock`, `close`), which `docs/gds/proofs` proves correct in Lean and the
//! tests of this module enumerate.
//!
//! ```
//! use gd_plugins::files::{LockError, lock_exclusive, make_temp_dir};
//!
//! let dir = make_temp_dir(&std::env::temp_dir(), "doc-lock")?;
//! let path = dir.join("state.lock");
//! let held = lock_exclusive(&path)?;
//! assert!(matches!(lock_exclusive(&path), Err(LockError::Held)));
//! drop(held);
//! lock_exclusive(&path)?;
//! std::fs::remove_dir_all(&dir)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;

/// An exclusive lock on a lock file, held until the value is dropped. A second lock of
/// the same file is refused while this one lives, in another process and in another
/// thread of this one. When the process ends without dropping it, the system releases
/// the lock.
#[derive(Debug)]
pub struct FileLock {
    file: File,
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // Transition `unlock`: release the lock for every copy of the descriptor, also
        // one in a child that has not started its program yet. A failure leaves the
        // release to the close that follows, as before this type existed.
        let _ = self.file.unlock();
        // Transition `close`: the file closes when the field drops.
    }
}

/// Why [`lock_exclusive`] took no lock.
#[derive(Debug)]
pub enum LockError {
    /// Another holder has the lock: a live one, or one that ended without releasing
    /// while a child it had just created had not yet started its program.
    Held,
    /// The lock file could not be opened or locked.
    Io(io::Error),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::Held => f.write_str("the lock is held"),
            LockError::Io(e) => write!(f, "the lock file failed: {e}"),
        }
    }
}

impl std::error::Error for LockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LockError::Held => None,
            LockError::Io(e) => Some(e),
        }
    }
}

impl From<io::Error> for LockError {
    fn from(e: io::Error) -> LockError {
        LockError::Io(e)
    }
}

/// Takes the exclusive lock of the lock file `path`, creating the file if it is
/// missing, without waiting. The folder of `path` must exist. Returns the lock, which
/// borrows nothing; fails with [`LockError::Held`] when the lock is held, and with
/// [`LockError::Io`] when the file cannot be opened or locked.
///
/// We keep the lock file after use: removing it would let two takers lock different
/// files at one path.
pub fn lock_exclusive(path: &Path) -> Result<FileLock, LockError> {
    take(path, false)
}

/// As [`lock_exclusive`], and a lock file it creates is readable and writable only by
/// the user, where the system has permissions.
pub fn lock_exclusive_private(path: &Path) -> Result<FileLock, LockError> {
    take(path, true)
}

fn take(path: &Path, private: bool) -> Result<FileLock, LockError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    // Transition `openFile`: an open file of our own, so its lock conflicts with every
    // other taker's, in this process too.
    let file = options.open(path)?;
    match file.try_lock() {
        // Transition `acquire`.
        Ok(()) => Ok(FileLock { file }),
        // Transition `refuse`: the file closes here; it never held the lock.
        Err(TryLockError::WouldBlock) => Err(LockError::Held),
        Err(TryLockError::Error(e)) => Err(LockError::Io(e)),
    }
}

#[cfg(test)]
mod model;
#[cfg(test)]
mod tests;
