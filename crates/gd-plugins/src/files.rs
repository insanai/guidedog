//! Private folders and files, and files replaced in one step: what a host or a publisher
//! needs to keep state (a registry, tokens, a frozen site) that only its user may read.
//! Where the system has permissions, folders are made `0700` and files `0600`.
//!
//! ```
//! let dir = gd_plugins::files::make_temp_dir(&std::env::temp_dir(), "doc-example")?;
//! gd_plugins::files::write_atomically(&dir.join("state.toml"), b"schema = 1\n")
//!     .map_err(|(_, e)| e)?;
//! std::fs::remove_dir_all(&dir)?;
//! # Ok::<(), std::io::Error>(())
//! ```

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Creates `dir` and its missing parents, readable only by the user. A folder that
/// exists already is kept as it is. Fails with the error of the folder that could not
/// be made.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    private_dir_builder().recursive(true).create(dir)
}

/// Makes a new folder in `parent`, readable only by the user, named `prefix-` and a
/// suffix unique to this process and call, and returns its path. `parent` must exist.
pub fn make_temp_dir(parent: &Path, prefix: &str) -> io::Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    loop {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("{prefix}-{}-{nanos:x}-{n}", std::process::id()));
        match private_dir_builder().create(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

fn private_dir_builder() -> fs::DirBuilder {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
}

/// Creates a new file readable and writable only by the user and opens it for writing;
/// fails if it exists.
pub fn create_private(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// Writes `data` to a private file beside `path` (its name and `.next`), syncs it and
/// renames it over `path`, so a reader sees the old file or the new one, never a part.
/// On failure the file beside is removed, and the error comes with the path that
/// failed: the file beside when writing it failed, `path` when the rename did.
pub fn write_atomically(path: &Path, data: &[u8]) -> Result<(), (PathBuf, io::Error)> {
    let mut next = path.as_os_str().to_owned();
    next.push(".next");
    let next = PathBuf::from(next);
    let _ = fs::remove_file(&next);
    let written = create_private(&next).and_then(|mut f| {
        f.write_all(data)?;
        f.sync_all()
    });
    if let Err(e) = written {
        let _ = fs::remove_file(&next);
        return Err((next, e));
    }
    if let Err(e) = fs::rename(&next, path) {
        let _ = fs::remove_file(&next);
        return Err((path.to_path_buf(), e));
    }
    Ok(())
}
