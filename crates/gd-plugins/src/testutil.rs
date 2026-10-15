//! Temporary folders for unit tests.

use std::path::{Path, PathBuf};

/// A private temporary folder removed when dropped.
pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new(name: &str) -> TempDir {
        let path = crate::snapshot::make_temp_dir(&std::env::temp_dir(), name).unwrap();
        TempDir(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// Writes `files` below the folder, each holding its own path as text.
    pub(crate) fn with_files(name: &str, files: &[&str]) -> TempDir {
        let dir = TempDir::new(name);
        for f in files {
            let path = dir.0.join(f);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, f).unwrap();
        }
        dir
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
