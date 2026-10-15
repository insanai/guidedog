//! `Snapshot`: a complete, checked, digested site that a publisher publishes.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use gd_sha::Sha256;

use crate::paths::is_servable;
use crate::problem::{self, Problem};

const DEFAULT_MAX_FILES: usize = 100_000;
/// 100 MiB less a byte: the largest file GitHub accepts.
const DEFAULT_MAX_FILE_BYTES: u64 = 100 * 1024 * 1024 - 1;
/// 1 GiB: the largest GitHub Pages site.
const DEFAULT_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_DEPTH: usize = 64;

/// How a snapshot holds its files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum SnapshotMode {
    /// Copies the folder into a private folder, which dropping the snapshot removes.
    #[default]
    Copy,
    /// Checks the folder where it is; the caller keeps it, unchanged.
    Borrow,
    /// Checks a private folder where it is; dropping the snapshot removes it.
    Adopt,
}

/// Bounds and placement of a snapshot. Zero bounds mean the defaults: 100,000 files,
/// files under 100 MiB, and 1 GiB in all.
///
/// It is `#[non_exhaustive]`: use [`SnapshotOptions::new`] or `Default`, then assign.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SnapshotOptions {
    /// How the snapshot holds its files.
    pub mode: SnapshotMode,
    /// `Copy`: where the private folder is made; `None`: the system's temporary folder.
    pub temp_dir: Option<PathBuf>,
    /// The most files.
    pub max_files: usize,
    /// The largest file, in bytes.
    pub max_file_bytes: u64,
    /// The most bytes in all.
    pub max_bytes: u64,
    /// Accept a site without `index.html` at its root.
    pub no_index: bool,
}

impl SnapshotOptions {
    /// Default bounds, held as `mode` says.
    pub fn new(mode: SnapshotMode) -> SnapshotOptions {
        SnapshotOptions { mode, ..SnapshotOptions::default() }
    }
}

/// One file of a snapshot: its path from the root with `/` between folders, its size,
/// and the SHA-256 of its contents. Schema 1 publishes every file as a regular,
/// non-executable file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path from the root.
    pub path: String,
    /// The size in bytes.
    pub size: u64,
    /// The SHA-256 of the contents.
    pub digest: [u8; 32],
}

/// A complete, checked site: its folder, its files in the order of their paths'
/// bytes, their total size, and its digest, which names the generation. Dropping a
/// snapshot that is not borrowed removes its private folder.
#[derive(Debug)]
pub struct Snapshot {
    root: PathBuf,
    entries: Vec<Entry>,
    bytes: u64,
    digest: [u8; 32],
    mode: SnapshotMode,
    owns_root: bool,
}

impl Snapshot {
    /// The folder holding the site's files.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The files, sorted by path bytes.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The total size in bytes.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The generation's digest.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// How the snapshot holds its files.
    pub fn mode(&self) -> SnapshotMode {
        self.mode
    }

    /// Reads the folder again and checks it holds exactly these files with these
    /// contents. A frozen site that changed is refused before anything is published.
    pub fn verify(&self) -> Result<(), Problem> {
        let again = snapshot(&self.root, &self.bounds())?;
        if again.digest != self.digest {
            return Err(changed(&self.root));
        }
        Ok(())
    }

    /// Rechecking uses the accepted manifest's bounds rather than the defaults, so a
    /// large file a publisher accepted stays accepted; any change shows in the digest.
    pub(crate) fn bounds(&self) -> SnapshotOptions {
        let largest = self.entries.iter().map(|e| e.size).max().unwrap_or(1).max(1);
        SnapshotOptions {
            mode: SnapshotMode::Borrow,
            temp_dir: None,
            max_files: self.entries.len().max(1),
            max_file_bytes: largest,
            max_bytes: self.bytes.max(1),
            no_index: true,
        }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        if self.owns_root {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

/// A digest in lowercase hexadecimal.
pub fn digest_hex(digest: &[u8; 32]) -> String {
    gd_sha::to_hex(digest)
}

/// Freezes the site in `dir`: every regular file below it, checked and digested, held
/// as `options.mode` says. Refuses a symbolic link or special file, a name Git or a
/// static host cannot serve, two names that differ only in case, a site over the
/// limits, and one without `index.html` (unless `no_index`). A refusal leaves no
/// private folder behind.
pub fn snapshot(dir: impl AsRef<Path>, options: &SnapshotOptions) -> Result<Snapshot, Problem> {
    let dir = dir.as_ref();
    let shown = dir.to_string_lossy();
    let is_dir = fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir());
    if !is_dir {
        return Err(Problem::new(
            "plugins.site",
            "The site is not a folder",
            "There is no folder here to publish.",
        )
        .hint("Build the site first, or name the folder that holds it.")
        .path(&shown)
        .exit(4));
    }
    let mut snap = Snapshot {
        root: dir.to_path_buf(),
        entries: Vec::new(),
        bytes: 0,
        digest: [0; 32],
        mode: options.mode,
        owns_root: false,
    };
    if options.mode == SnapshotMode::Copy {
        let parent = options.temp_dir.clone().unwrap_or_else(std::env::temp_dir);
        snap.root = make_temp_dir(&parent, "guidedog-snapshot")
            .map_err(|e| problem::io(&parent.to_string_lossy(), &e))?;
        snap.owns_root = true;
    }
    let mut walker = Walker::new(&mut snap, dir, options);
    walker.walk("", 0)?;
    finish(&mut snap, &shown, options.no_index)?;
    snap.owns_root = options.mode != SnapshotMode::Borrow;
    Ok(snap)
}

fn finish(snap: &mut Snapshot, shown: &str, no_index: bool) -> Result<(), Problem> {
    snap.entries.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    check_names(&snap.entries)?;
    if !no_index && !snap.entries.iter().any(|e| e.path == "index.html") {
        return Err(Problem::new(
            "plugins.index",
            "The site has no front page",
            "There is no index.html at the root of the site.",
        )
        .hint("Build the HTML site first, or name the folder that holds its index.html.")
        .path(shown));
    }
    snap.digest = manifest_digest(&snap.entries);
    Ok(())
}

/// Reads the folder below `source`, adding its files to the snapshot: digested where
/// they are, or copied below the snapshot's root and digested as they are copied.
struct Walker<'a> {
    snap: &'a mut Snapshot,
    source: &'a Path,
    max_files: usize,
    max_file: u64,
    max_bytes: u64,
}

impl<'a> Walker<'a> {
    fn new(snap: &'a mut Snapshot, source: &'a Path, o: &SnapshotOptions) -> Walker<'a> {
        let or = |v: u64, d: u64| if v > 0 { v } else { d };
        Walker {
            snap,
            source,
            max_files: if o.max_files > 0 { o.max_files } else { DEFAULT_MAX_FILES },
            max_file: or(o.max_file_bytes, DEFAULT_MAX_FILE_BYTES),
            max_bytes: or(o.max_bytes, DEFAULT_MAX_BYTES),
        }
    }

    fn walk(&mut self, rel: &str, depth: usize) -> Result<(), Problem> {
        if depth > MAX_DEPTH {
            return Err(limit(rel, "folders more than 64 deep"));
        }
        let here = self.source.join(rel);
        let shown = here.to_string_lossy().into_owned();
        let list = fs::read_dir(&here).map_err(|e| problem::io(&shown, &e))?;
        for item in list {
            let item = item.map_err(|e| problem::io(&shown, &e))?;
            let full = item.path();
            let Some(name) = item.file_name().to_str().map(str::to_string) else {
                return Err(unservable(&full.to_string_lossy()));
            };
            let path = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            let meta = fs::symlink_metadata(&full)
                .map_err(|e| problem::io(&full.to_string_lossy(), &e))?;
            if meta.is_dir() {
                self.enter(&path, depth)?;
            } else if meta.is_file() {
                self.add(&full, path, meta.len())?;
            } else {
                return Err(special_file(&full.to_string_lossy()));
            }
        }
        Ok(())
    }

    fn enter(&mut self, path: &str, depth: usize) -> Result<(), Problem> {
        if self.snap.mode == SnapshotMode::Copy {
            let copy = self.snap.root.join(path);
            fs::create_dir(&copy).map_err(|e| problem::io(&copy.to_string_lossy(), &e))?;
        }
        self.walk(path, depth + 1)
    }

    fn add(&mut self, full: &Path, path: String, size: u64) -> Result<(), Problem> {
        if self.snap.entries.len() >= self.max_files {
            return Err(limit(&path, "more files than allowed"));
        }
        if size > self.max_file {
            let what = format!("a file of {size} bytes, larger than {}", self.max_file);
            return Err(limit(&path, &what));
        }
        self.snap.bytes += size;
        if self.snap.bytes > self.max_bytes {
            return Err(limit(&path, "more bytes than allowed"));
        }
        let copy = (self.snap.mode == SnapshotMode::Copy).then(|| self.snap.root.join(&path));
        let digest = digest_file(full, copy.as_deref(), size)?;
        self.snap.entries.push(Entry { path, size, digest });
        Ok(())
    }
}

/// Reads the file at `path` a chunk at a time, copying it to `copy` when set, and
/// returns its SHA-256; refuses a file whose size changes while it is read.
fn digest_file(path: &Path, copy: Option<&Path>, size: u64) -> Result<[u8; 32], Problem> {
    let shown = || path.to_string_lossy().into_owned();
    let mut file = fs::File::open(path).map_err(|e| problem::io(&shown(), &e))?;
    let mut out = match copy {
        Some(c) => Some(create_private(c).map_err(|e| problem::io(&c.to_string_lossy(), &e))?),
        None => None,
    };
    let mut hash = Sha256::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let mut read = 0u64;
    loop {
        let n = file.read(&mut chunk).map_err(|e| problem::io(&shown(), &e))?;
        if n == 0 {
            break;
        }
        read += n as u64;
        if read > size {
            break;
        }
        hash.update(&chunk[..n]);
        if let (Some(out), Some(c)) = (out.as_mut(), copy) {
            out.write_all(&chunk[..n]).map_err(|e| problem::io(&c.to_string_lossy(), &e))?;
        }
    }
    if read != size {
        return Err(Problem::new(
            "plugins.snapshot-changed",
            "A file changed while it was read",
            "The file's size changed while it was read.",
        )
        .hint("Run the command again once the program writing the site has finished.")
        .path(&shown()));
    }
    Ok(hash.finish())
}

/// Refuses a path a static host or Git cannot serve, and two paths (or folders above
/// them) that differ only in case, which collide on case-insensitive systems.
pub(crate) fn check_names(entries: &[Entry]) -> Result<(), Problem> {
    let mut seen: HashMap<String, &str> = HashMap::with_capacity(entries.len());
    for e in entries {
        if !is_servable(&e.path) {
            return Err(unservable(&e.path));
        }
        let mut path = e.path.as_str();
        loop {
            let lower = path.to_lowercase();
            match seen.get(&lower) {
                Some(&other) if other != path => {
                    return Err(Problem::new(
                        "plugins.case",
                        "Two names differ only in case",
                        &format!("{other} and {path} collide on systems that ignore case."),
                    )
                    .hint("Rename one of the two files or folders in the site's sources.")
                    .path(&e.path));
                }
                Some(_) => break,
                None => {
                    seen.insert(lower, path);
                }
            }
            match path.rfind('/') {
                Some(slash) => path = &path[..slash],
                None => break,
            }
        }
    }
    Ok(())
}

/// The SHA-256 of the manifest: the line `guidedog-snapshot 1\n`, then for each entry
/// the length of its path (8 bytes, big-endian), the path, its size (8 bytes) and its
/// digest. No two manifests write the same bytes.
fn manifest_digest(entries: &[Entry]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"guidedog-snapshot 1\n");
    for e in entries {
        hash.update(&(e.path.len() as u64).to_be_bytes());
        hash.update(e.path.as_bytes());
        hash.update(&e.size.to_be_bytes());
        hash.update(&e.digest);
    }
    hash.finish()
}

/// Makes a new folder, readable only by the user, named `prefix-...` in `parent`.
pub(crate) fn make_temp_dir(parent: &Path, prefix: &str) -> std::io::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    loop {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("{prefix}-{}-{nanos:x}-{n}", std::process::id()));
        match private_dir_builder().create(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

fn private_dir_builder() -> fs::DirBuilder {
    #[cfg(unix)]
    {
        let mut builder = fs::DirBuilder::new();
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
    }
    #[cfg(not(unix))]
    {
        fs::DirBuilder::new()
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

fn changed(root: &Path) -> Problem {
    Problem::new(
        "plugins.snapshot-changed",
        "The frozen site changed",
        "Files of the site changed after it was frozen.",
    )
    .hint("Run the command again, and let no other program write to its build folder.")
    .path(&root.to_string_lossy())
}

fn unservable(path: &str) -> Problem {
    Problem::new(
        "plugins.path",
        "A file's name cannot be published",
        "The path is too long, is not UTF-8, or holds a control character, a backslash, \
         or a part named .git.",
    )
    .hint("Rename the file in the site's sources.")
    .path(path)
}

fn special_file(path: &str) -> Problem {
    Problem::new(
        "plugins.file-type",
        "The site holds a link or special file",
        "A site may hold only folders and regular files.",
    )
    .hint("Replace the symbolic link with a copy of the file it points to, or remove it.")
    .path(path)
}

fn limit(path: &str, what: &str) -> Problem {
    Problem::new(
        "plugins.limit",
        "The site is over a publishing limit",
        &format!("The site has {what}."),
    )
    .hint("Publish large downloads, such as PDF books, as release assets and link to them.")
    .path(path)
    .exit(3)
}

#[cfg(test)]
mod tests;
