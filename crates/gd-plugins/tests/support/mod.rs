//! Helpers for the process tests: the example programs cargo built, and private
//! folders beside them.

// Each test file uses a different part of this module.
#![allow(dead_code)]
#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::path::{Path, PathBuf};

/// The folder cargo builds examples into: `target/<profile>/examples`.
pub fn examples_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent().unwrap().parent().unwrap().join("examples")
}

/// A private folder removed when dropped, on the same disk as the example programs,
/// so programs can be hard-linked into it.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let base = examples_dir().parent().unwrap().join("gd-plugins-tests");
        let dir = base.join(format!("{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    pub fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    /// Links (or copies) the example program `example` into the folder as `name`.
    /// Linking the built file avoids exec racing a fresh write on Linux.
    pub fn program(&self, example: &str, name: &str) -> String {
        let source = examples_dir().join(format!("{example}{}", std::env::consts::EXE_SUFFIX));
        let target = self.0.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if std::fs::hard_link(&source, &target).is_err() {
            std::fs::copy(&source, &target).unwrap();
        }
        target.to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}
