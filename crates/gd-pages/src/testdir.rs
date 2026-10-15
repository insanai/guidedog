//! Temporary folders for unit tests.

use std::path::{Path, PathBuf};

/// A private temporary folder removed when dropped.
pub(crate) struct TestDir(PathBuf);

impl TestDir {
    pub(crate) fn new(name: &str) -> TestDir {
        let base = std::env::temp_dir().join("gd-pages-tests");
        TestDir(crate::files::make_unique_dir(&base, name).unwrap())
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
