//! Change detection by polling: a fingerprint of every watched entry's name, type,
//! size and modification time. Std has no file-notification API, and a preview only
//! needs to know "did anything change since the last build" when a page is requested.

use std::collections::hash_map::DefaultHasher;
use std::fs::{self, Metadata};
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// How deep a watched tree is followed; deeper folders are not fingerprinted.
const MAX_DEPTH: usize = 64;

/// The sources a preview watches: root folders (or single files) and the paths to
/// leave out, such as the build folder inside the source folder.
///
/// Entries whose names start with `.` (version control, editor and lock files) are
/// never watched, nor are folders' own times: creating the excluded build folder must
/// not itself look like a change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watch {
    /// Folders or files to watch. A root inside an earlier root is watched once.
    pub roots: Vec<PathBuf>,
    /// Paths left out wherever they appear (compared as given, not canonicalized).
    pub skip: Vec<PathBuf>,
}

impl Watch {
    /// A fingerprint of every watched entry. Equal stamps mean no watched file was
    /// added, removed, resized or touched. Reads metadata only, never contents; a
    /// missing or unreadable root has its own marker, so removing and restoring it
    /// both count as changes. Stamps are comparable within one process only.
    pub fn stamp(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for (i, root) in self.roots.iter().enumerate() {
            if self.roots[..i].iter().any(|earlier| root.starts_with(earlier)) {
                continue;
            }
            self.hash_tree(&mut hasher, root, 0);
        }
        hasher.finish()
    }

    fn is_skipped(&self, path: &Path) -> bool {
        self.skip.iter().any(|s| s == path)
    }

    fn hash_tree(&self, hasher: &mut DefaultHasher, dir: &Path, depth: usize) {
        if self.is_skipped(dir) || depth > MAX_DEPTH {
            return;
        }
        hasher.write(dir.as_os_str().as_encoded_bytes());
        hasher.write_u8(0);
        match fs::metadata(dir) {
            Ok(m) if !m.is_dir() => return hash_metadata(hasher, &m),
            Ok(_) => {}
            Err(_) => return hasher.write_u8(255),
        }
        let Ok(listing) = fs::read_dir(dir) else {
            return hasher.write_u8(255);
        };
        // Sorted, so the stamp does not depend on the order the system lists in.
        let mut entries: Vec<_> = listing.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            let path = entry.path();
            if name.as_encoded_bytes().starts_with(b".") || self.is_skipped(&path) {
                continue;
            }
            hasher.write(name.as_encoded_bytes());
            hasher.write_u8(0);
            self.hash_entry(hasher, &path, depth);
        }
    }

    /// Folders recurse; a symbolic link to a folder is hashed by its own metadata
    /// only, so link cycles cannot loop. Links to files follow to the target.
    fn hash_entry(&self, hasher: &mut DefaultHasher, path: &Path, depth: usize) {
        let Ok(own) = fs::symlink_metadata(path) else {
            return hasher.write_u8(254);
        };
        if own.is_dir() {
            hasher.write_u8(1);
            return self.hash_tree(hasher, path, depth + 1);
        }
        match fs::metadata(path) {
            Ok(target) if !target.is_dir() => hash_metadata(hasher, &target),
            _ => hash_metadata(hasher, &own),
        }
    }
}

fn hash_metadata(hasher: &mut DefaultHasher, m: &Metadata) {
    hasher.write_u8(if m.is_file() { 2 } else { 3 });
    hasher.write_u64(m.len());
    let nanos = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    hasher.write_u128(nanos);
}

#[cfg(test)]
mod tests {
    use super::Watch;
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    /// A temporary folder removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let path = std::env::temp_dir()
                .join(format!("gd-serve-{name}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp dir");
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn put(dir: &Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).expect("write fixture");
    }

    #[test]
    fn detects_deletions_and_edits_below_the_newest_timestamp() {
        let temp = TempDir::new("watch");
        let dir = &temp.0;
        let build = dir.join("_build");
        let watch = Watch { roots: vec![dir.clone()], skip: vec![build.clone()] };
        put(dir, "future.txt", "A file dated ahead of the clock.\n");
        let future = SystemTime::now() + Duration::from_secs(24 * 3600);
        let file = File::options().write(true).open(dir.join("future.txt")).expect("open");
        file.set_modified(future).expect("set time");
        put(dir, "index.rst", "Before.\n");
        put(dir, "removed.rst", "A removed page.\n");
        let before = watch.stamp();
        assert_eq!(watch.stamp(), before);
        put(dir, "index.rst", "After editing an older file.\n");
        let edited = watch.stamp();
        assert_ne!(edited, before);
        fs::remove_file(dir.join("removed.rst")).expect("remove");
        let deleted = watch.stamp();
        assert_ne!(deleted, edited);
        fs::create_dir_all(&build).expect("build dir");
        put(&build, "index.html", "Generated output.\n");
        put(dir, ".guidedog.lock", "Runtime state.\n");
        assert_eq!(watch.stamp(), deleted);
    }

    #[test]
    fn watches_shared_extra_files_and_root_removal() {
        let temp = TempDir::new("shared");
        let dir = temp.0.join("docs");
        let shared = temp.0.join("shared");
        fs::create_dir(&dir).expect("docs");
        fs::create_dir(&shared).expect("shared");
        put(&shared, "extra.txt", "Before.\n");
        let watch = Watch { roots: vec![dir.clone(), shared.join("extra.txt")], skip: vec![] };
        let before = watch.stamp();
        put(&shared, "extra.txt", "After editing a shared file.\n");
        let edited = watch.stamp();
        assert_ne!(edited, before);
        fs::remove_dir_all(&shared).expect("remove shared");
        let removed = watch.stamp();
        assert_ne!(removed, edited);
        fs::create_dir(&shared).expect("restore shared");
        let folder = Watch { roots: vec![dir, shared.clone()], skip: vec![] };
        let before = folder.stamp();
        put(&shared, "messages.po", "New input.\n");
        assert_ne!(folder.stamp(), before);
    }

    #[test]
    fn nested_roots_are_watched_once() {
        let temp = TempDir::new("nested");
        let inner = temp.0.join("inner");
        fs::create_dir(&inner).expect("inner");
        put(&inner, "a.rst", "A.\n");
        let once = Watch { roots: vec![temp.0.clone()], skip: vec![] };
        let twice = Watch { roots: vec![temp.0.clone(), inner], skip: vec![] };
        assert_eq!(once.stamp(), twice.stamp());
    }
}
