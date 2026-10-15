//! Private folders and files, and files replaced in one step.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Creates `dir` and its parents, readable only by the user where the system has
/// permissions.
pub(crate) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    private_builder().recursive(true).create(dir)
}

/// A folder builder that makes folders readable only by the user, where it can.
fn private_builder() -> fs::DirBuilder {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
}

/// Creates a new private folder in `parent` named `prefix` and a unique suffix.
pub(crate) fn make_unique_dir(parent: &Path, prefix: &str) -> std::io::Result<PathBuf> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    create_private_dir(parent)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = parent.join(format!("{prefix}{}-{nanos:08x}-{n}", std::process::id()));
        match private_builder().create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

/// Creates a new file readable and writable only by the user; fails if it exists.
pub(crate) fn create_private(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// Writes `text` beside `path`, syncs it, and renames it over `path`, so a reader sees
/// the old file or the new one. Returns the path that failed with the error.
pub(crate) fn write_atomically(path: &Path, text: &str) -> Result<(), (PathBuf, std::io::Error)> {
    let mut next = path.as_os_str().to_owned();
    next.push(".next");
    let next = PathBuf::from(next);
    let _ = fs::remove_file(&next);
    let written = create_private(&next).and_then(|mut f| {
        f.write_all(text.as_bytes())?;
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
