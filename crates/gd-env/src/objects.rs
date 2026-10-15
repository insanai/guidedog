//! The object store: documents' parsed trees, and their search words, kept below the
//! doctrees folder in files named by the digest of their bytes.
//!
//! `objects/<docname>.<digest16>.gdo` keeps a document's folders, so distinct docnames
//! never share an object (`a/b` and `a__b` differ). Once written a file never changes:
//! an environment that names it can use it whatever later builds wrote, and a commit
//! publishes the environment naming it only after flushing it (gd-outputs). Files the
//! environment no longer names are swept after a commit.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gd_conf::Digest;
use gd_host::Problem;
use guidedoc::{Document, Limits, Status, Workspace};

use crate::environment::{CachedFile, digest_name};

/// The folder of documents' objects, below the doctrees folder.
pub const OBJECTS_FOLDER: &str = "objects";
/// The folder of documents' search words, below the doctrees folder.
pub const SEARCH_FOLDER: &str = "search";

/// Why a cached object could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectError {
    /// What went wrong, with a hint.
    pub problem: Problem,
    /// The cache is at fault (missing, damaged, another document's, unloadable): the
    /// document is read again from its source. False for a problem that a new read
    /// would meet too (the budget).
    pub stale: bool,
}

/// The cache files of one build folder: objects and search words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStore {
    doctrees: PathBuf,
}

impl ObjectStore {
    /// The store below `doctrees` (`<build_dir>/.doctrees`). Nothing is created yet.
    pub fn new(doctrees: impl Into<PathBuf>) -> ObjectStore {
        ObjectStore { doctrees: doctrees.into() }
    }

    /// The doctrees folder.
    pub fn doctrees(&self) -> &Path {
        &self.doctrees
    }

    /// The absolute path of a cached file.
    pub fn path_of(&self, file: &CachedFile) -> PathBuf {
        self.doctrees.join(&file.path)
    }

    /// Creates the objects folder, so reading threads never race to create it.
    pub fn prepare(&self) -> Result<(), Problem> {
        let dir = self.doctrees.join(OBJECTS_FOLDER);
        std::fs::create_dir_all(&dir).map_err(|e| gd_host::io_problem(&dir, &e))
    }

    /// Writes `doc` as the object of `docname`, unless a file with its bytes is already
    /// there, without flushing it (the commit that publishes the environment naming it
    /// flushes it first). Returns the cached file the environment records.
    pub fn save_object(&self, docname: &str, doc: Document<'_>) -> Result<CachedFile, Problem> {
        let mut bytes = Vec::new();
        guidedoc::write_object(doc, &mut bytes);
        self.save_bytes(OBJECTS_FOLDER, docname, ".gdo", &bytes)
    }

    /// Writes a document's search words (`search/<docname>.<digest16>.txt`), as
    /// [`ObjectStore::save_object`] writes objects.
    pub fn save_search(&self, docname: &str, words: &str) -> Result<CachedFile, Problem> {
        self.save_bytes(SEARCH_FOLDER, docname, ".txt", words.as_bytes())
    }

    fn save_bytes(
        &self,
        folder: &str,
        docname: &str,
        suffix: &str,
        bytes: &[u8],
    ) -> Result<CachedFile, Problem> {
        let digest = Digest::of_parts(&[bytes]);
        let path = format!("{folder}/{}", digest_name(docname, &digest, suffix));
        let full = self.doctrees.join(&path);
        if !gd_host::same_contents(&full, bytes) {
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).map_err(|e| gd_host::io_problem(parent, &e))?;
            }
            gd_host::write_unsynced(&full, bytes)?;
        }
        Ok(CachedFile { path, digest })
    }

    /// Reads a cached file's bytes back and checks them against its digest.
    pub fn load_bytes(&self, file: &CachedFile) -> Result<Vec<u8>, ObjectError> {
        let path = self.path_of(file);
        let bytes = gd_host::read_file(&path, None, usize::MAX).map_err(|problem| {
            let stale = problem.code == "host.io" || problem.code == "host.file-type";
            ObjectError { problem, stale }
        })?;
        if Digest::of_parts(&[&bytes]) != file.digest {
            return Err(ObjectError { problem: mismatch(&path), stale: true });
        }
        Ok(bytes)
    }

    /// Loads the object `file` names into `ws` as a new conversion, within `limits`.
    /// Fails, stale, when the object is missing, is not the one recorded (damaged, or
    /// another document's: it would publish the wrong content), or the engine cannot
    /// load it; the document is then read again from its source.
    pub fn load_object<'w>(
        &self,
        ws: &'w mut Workspace,
        file: &CachedFile,
        limits: &Limits,
    ) -> Result<Document<'w>, ObjectError> {
        let bytes = self.load_bytes(file)?;
        let path = self.path_of(file);
        guidedoc::load_object(ws, &bytes, limits).map_err(|fail| ObjectError {
            problem: Problem::new(
                "build.object",
                "CACHED DOCUMENT IS UNUSABLE",
                &path.display().to_string(),
                format!("The cached object could not be loaded: {fail}"),
                "Rebuild with guidedog build -E.",
                5,
            ),
            // A limit reached would be reached again by a new read.
            stale: !matches!(fail.status, Status::Limit | Status::Capacity),
        })
    }

    /// Removes the objects and search words not in `keep` (paths below the doctrees
    /// folder, as `CachedFile::path`): those of documents that are gone or changed, and
    /// those a failed build wrote. Empty folders go too. Errors are ignored: a file
    /// left behind is swept by a later build.
    pub fn sweep<'a>(&self, keep: impl IntoIterator<Item = &'a str>) {
        let keep: HashSet<&str> = keep.into_iter().collect();
        for folder in [OBJECTS_FOLDER, SEARCH_FOLDER] {
            self.sweep_folder(&self.doctrees.join(folder), &keep);
        }
    }

    fn sweep_folder(&self, dir: &Path, keep: &HashSet<&str>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                self.sweep_folder(&path, keep);
                let _ = std::fs::remove_dir(&path); // only when it is empty.
                continue;
            }
            let rel = crate::discover::relative_to(&self.doctrees, &path);
            if !keep.contains(rel.as_str()) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

fn mismatch(path: &Path) -> Problem {
    Problem::new(
        "build.object",
        "CACHED DOCUMENT DOES NOT MATCH",
        &path.display().to_string(),
        "The cached object is not the one its document wrote.",
        "Build again with guidedog build -E to read every document afresh.",
        5,
    )
}
