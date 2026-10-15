//! The source files a document's generating directives read (Python for autodoc, Odin
//! for the odin domain): confined to their folders, read once per document, and kept
//! with the digest of their bytes, which become the document's dependencies.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use gd_conf::Digest;
use gd_env::Dependency;

/// What one document's directives read, shared by the readers they make.
#[derive(Debug, Default)]
struct FileLog {
    /// Each file read, by path: the digest of its bytes and its text.
    files: BTreeMap<PathBuf, (Digest, String)>,
    /// Something was not found: the document is read again next build.
    missing: bool,
}

/// A confined, caching reader over the files below some folders. Clones share the
/// files read, so a reader handed to an analyser records into the document's log.
#[derive(Debug, Clone)]
pub(crate) struct SourceFiles {
    /// The folders, canonical (`gd_host::canonical_roots`).
    readable: Arc<[PathBuf]>,
    log: Arc<Mutex<FileLog>>,
}

impl SourceFiles {
    /// Files below `roots` (as written; they are resolved here).
    pub(crate) fn new(roots: &[PathBuf]) -> SourceFiles {
        SourceFiles { readable: gd_host::canonical_roots(roots).into(), log: Arc::default() }
    }

    /// Whether `path` lies below the folders, links followed.
    pub(crate) fn allows(&self, path: &Path) -> bool {
        gd_host::confined(&self.readable, path)
    }

    /// The text of the file at `path`: `None` outside the folders, when it cannot be
    /// read, when it is larger than a document may be, or when it is not UTF-8. A file
    /// is read from the disk once; later reads take the text kept.
    pub(crate) fn read(&self, path: &Path) -> Option<String> {
        if let Some((_, text)) = lock(&self.log).files.get(path) {
            return Some(text.clone());
        }
        if !self.allows(path) {
            return None;
        }
        let bytes = gd_host::read_file(path, None, gd_host::MAX_SOURCE_BYTES).ok()?;
        let digest = Digest::of_parts(&[&bytes]);
        let text = String::from_utf8(bytes).ok()?;
        lock(&self.log).files.insert(path.to_path_buf(), (digest, text.clone()));
        Some(text)
    }

    /// The entries of the folder at `dir`, sorted, folders ending with `/`; `None`
    /// outside the folders or when it cannot be listed.
    pub(crate) fn list(&self, dir: &Path) -> Option<Vec<String>> {
        if !self.allows(dir) {
            return None;
        }
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).ok()? {
            let entry = entry.ok()?;
            let mut name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() {
                name.push('/');
            }
            out.push(name);
        }
        out.sort();
        Some(out)
    }

    /// Notes that something a directive asked for was not found.
    pub(crate) fn note_missing(&self) {
        lock(&self.log).missing = true;
    }

    /// Adds the files read to `deps`, sorted by path. When something was not found,
    /// `missing` is added too: a path that names no file, so the document is read again
    /// every build in case the source appears.
    pub(crate) fn dependencies(&self, missing: &str, deps: &mut Vec<Dependency>) {
        let log = lock(&self.log);
        for (path, (digest, _)) in &log.files {
            deps.push(Dependency { path: path.clone(), digest: *digest, included: false });
        }
        if log.missing {
            deps.push(Dependency { path: missing.into(), ..Dependency::default() });
        }
    }
}

/// Locks a log; a thread that panicked while holding it left it whole (each change is
/// one insert or one flag), so its contents are still used.
fn lock(log: &Mutex<FileLog>) -> MutexGuard<'_, FileLog> {
    log.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_confined_cached_and_recorded() {
        let dir = std::env::temp_dir().join(format!("gd-read-files-{}", std::process::id()));
        let inside = dir.join("src");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(inside.join("a.py"), "x = 1\n").unwrap();
        std::fs::write(dir.join("outside.py"), "y = 2\n").unwrap();
        let files = SourceFiles::new(std::slice::from_ref(&inside));
        assert_eq!(files.read(&inside.join("a.py")).as_deref(), Some("x = 1\n"));
        assert_eq!(files.read(&dir.join("outside.py")), None);
        assert_eq!(files.list(&inside), Some(vec!["a.py".to_string()]));
        assert_eq!(files.list(&dir), None);
        std::fs::write(inside.join("a.py"), "x = 3\n").unwrap();
        assert_eq!(files.read(&inside.join("a.py")).as_deref(), Some("x = 1\n"), "cached");
        files.note_missing();
        let mut deps = Vec::new();
        files.dependencies("<missing>", &mut deps);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].digest, Digest::of_parts(&[b"x = 1\n"]));
        assert_eq!(deps[1].path, PathBuf::from("<missing>"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
