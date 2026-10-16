//! File-system helpers the tasks share: temporary folders removed on drop, copying and
//! removing trees, hashing files, and I/O failures as reports.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Failure, Result, fail};

/// The failure for an I/O error on `path`; `action` is a verb such as "read".
pub(crate) fn io_failure(action: &str, path: &Path, err: &io::Error) -> Failure {
    fail(
        "io",
        "FILE PROBLEM",
        &format!("I could not {action} {}: {err}.", path.display()),
        "Check that the path exists and that it can be written to.",
    )
}

/// Reads a whole text file.
pub(crate) fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|err| io_failure("read", path, &err))
}

/// Writes a whole file.
pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    fs::write(path, contents).map_err(|err| io_failure("write", path, &err))
}

/// Creates a folder and its parents.
pub(crate) fn create_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|err| io_failure("create", path, &err))
}

/// Removes a file or a folder tree; a path that does not exist is fine.
pub(crate) fn remove(path: &Path) -> Result<()> {
    let result = match fs::symlink_metadata(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => Err(err),
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
    };
    result.map_err(|err| io_failure("remove", path, &err))
}

/// Moves a file or folder; both paths are on one file system.
pub(crate) fn rename(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).map_err(|err| io_failure("move", from, &err))
}

/// Copies a file.
pub(crate) fn copy(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to).map(drop).map_err(|err| io_failure("copy", from, &err))
}

/// Copies the contents of the folder `from` into `to` (like `cp -R from/. to/`),
/// creating `to` and replacing files already there.
pub(crate) fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    create_dir(to)?;
    for entry in sorted_entries(from)? {
        let target = to.join(entry.file_name().unwrap_or_default());
        if entry.is_dir() {
            copy_tree(&entry, &target)?;
        } else {
            copy(&entry, &target)?;
        }
    }
    Ok(())
}

/// The paths in a folder, sorted by name.
pub(crate) fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(dir).map_err(|err| io_failure("read", dir, &err))?;
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(entry.map_err(|err| io_failure("read", dir, &err))?.path());
    }
    paths.sort();
    Ok(paths)
}

/// The lowercase hex SHA-256 of a file, read in chunks.
pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|err| io_failure("read", path, &err))?;
    let mut hasher = gd_sha::Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let n = file.read(&mut buffer).map_err(|err| io_failure("read", path, &err))?;
        if n == 0 {
            return Ok(gd_sha::to_hex(&hasher.finish()));
        }
        hasher.update(&buffer[..n]);
    }
}

/// Whether a path is a file the system would run.
pub(crate) fn is_executable(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// Sets a file's permissions to `rwxr-xr-x` (nothing to do on Windows).
pub(crate) fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, mode).map_err(|err| io_failure("change", path, &err))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// A path given on the command line, made absolute against the current folder.
pub(crate) fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir().map(|dir| dir.join(path)).unwrap_or_else(|_| path.to_path_buf())
}

/// A new, empty temporary folder, removed with everything in it when dropped.
#[derive(Debug)]
pub(crate) struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates the folder under the system's temporary folder.
    pub fn new(label: &str) -> Result<TempDir> {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let name = format!("xtask-{label}-{}-{nanos}-{n}", std::process::id());
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).map_err(|err| io_failure("create", &path, &err))?;
        Ok(TempDir { path })
    }

    /// The folder.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_trees_and_removes_them_on_drop() {
        let Ok(temp) = TempDir::new("test") else { panic!("no temporary folder") };
        let kept = temp.path().to_path_buf();
        let from = kept.join("a");
        assert!(create_dir(&from.join("b")).is_ok());
        assert!(write(&from.join("b/c.txt"), "abc").is_ok());
        assert!(copy_tree(&from, &kept.join("z")).is_ok());
        assert_eq!(read_text(&kept.join("z/b/c.txt")).ok().as_deref(), Some("abc"));
        let sum = sha256_file(&kept.join("z/b/c.txt")).ok();
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sum.as_deref(), Some(expected));
        drop(temp);
        assert!(!kept.exists());
    }
}
