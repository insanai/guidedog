//! Helpers shared by the integration tests: Python files in a temporary root.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use gd_autodoc::Session;
use gd_pyscan::Project;

/// A temporary folder removed when dropped.
pub struct TempRoot(pub PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes `files` (relative path, text) below a new temporary folder.
pub fn temp_root(files: &[(&str, &str)]) -> TempRoot {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("autodoc-{}-{n}", std::process::id()));
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("a file has a folder")).expect("mkdir");
        std::fs::write(&path, text).expect("write");
    }
    TempRoot(dir)
}

/// A session over a project whose only root is `root`.
pub fn session(root: &TempRoot) -> Session {
    Session::new(Project::new(&[&root.0]).expect("the Python grammar loads"))
}
