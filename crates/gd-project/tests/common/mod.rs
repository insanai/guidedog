//! Small projects in temporary folders for gd-project's end-to-end tests.

#![allow(dead_code, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use gd_project::{BuildOptions, BuildResult, BuilderKind, LoadOptions, Project};

/// A temporary folder removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("gd-project-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir.canonicalize().unwrap())
    }

    /// Writes `text` to `relative`, making its folders.
    pub fn put(&self, relative: &str, text: &str) {
        put(&self.0.join(relative), text);
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    pub fn read(&self, relative: &str) -> String {
        let path = self.0.join(relative);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.0.join(relative).exists()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A three-document project: index (reStructuredText) with a toctree of guide (rst)
/// and notes (MyST).
pub fn demo(name: &str) -> TempDir {
    let dir = TempDir::new(name);
    dir.put(
        "conf.toml",
        "project = \"Demo\"\nauthor = \"Ada\"\nsource_suffix = [\".rst\", \".md\"]\n\
         extensions = [\"myst_parser\"]\n",
    );
    dir.put(
        "index.rst",
        "Demo\n====\n\n.. toctree::\n\n   guide\n   notes\n\nSee :doc:`guide`.\n",
    );
    dir.put(
        "guide.rst",
        "Guide\n=====\n\nSome *text*. See :ref:`usage`.\n\n.. _usage:\n\nUsage\n-----\n\nUse it.\n",
    );
    dir.put("notes.md", "# Notes\n\nA note about {ref}`usage`.\n");
    dir
}

pub fn load(dir: &Path) -> Project {
    gd_project::load(dir, &LoadOptions::default()).unwrap()
}

pub fn build(project: &Project, kind: BuilderKind) -> BuildResult {
    gd_project::build(project, kind, &BuildOptions::default())
}

/// Fails with the result's problem and reports unless it built and published.
pub fn assert_built(r: &BuildResult) {
    assert!(r.exit == 0 && r.published, "{:#?}\n{:#?}", r.problem, r.reports);
}

/// The message of the first report with `code`.
pub fn report_with<'r>(r: &'r BuildResult, code: &str) -> Option<&'r str> {
    r.reports.iter().find(|rep| rep.code == code).map(|rep| rep.message.as_str())
}
