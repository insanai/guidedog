//! Projects: the folders that hold packages, how their files are read, and the
//! packages loaded from them.

use std::collections::HashMap;

use crate::Package;

/// Reads the files the analysis needs. A host confines both methods to its readable
/// roots; `FsReader` reads the file system. It is `Send` so a `Project` is, and a host
/// may share one project behind a `Mutex`.
pub trait FileReader: Send {
    /// The text of the file at `path`, or `None` when it cannot be read.
    fn read(&mut self, path: &str) -> Option<String>;
    /// The entries of the folder at `dir`, sorted, folders ending with "/"; `None` when
    /// it cannot be listed.
    fn list(&mut self, dir: &str) -> Option<Vec<String>>;
}

/// A `FileReader` over the file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsReader;

impl FileReader for FsReader {
    fn read(&mut self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn list(&mut self, dir: &str) -> Option<Vec<String>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).ok()? {
            let entry = entry.ok()?;
            let mut name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().ok()?.is_dir() {
                name.push('/');
            }
            out.push(name);
        }
        out.sort();
        Some(out)
    }
}

/// A `FileReader` over files held in memory, by path: for tests, and for hosts that
/// read sources some other way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryReader {
    /// File texts by path.
    pub files: std::collections::BTreeMap<String, String>,
}

impl MemoryReader {
    /// A reader over `(path, text)` pairs.
    pub fn new<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let files = files.into_iter().map(|(p, t)| (p.to_string(), t.to_string())).collect();
        MemoryReader { files }
    }
}

impl FileReader for MemoryReader {
    fn read(&mut self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }

    fn list(&mut self, dir: &str) -> Option<Vec<String>> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        let mut out: Vec<String> = Vec::new();
        for path in self.files.keys() {
            let Some(rest) = path.strip_prefix(&prefix) else {
                continue;
            };
            let name = match rest.find('/') {
                Some(k) => format!("{}/", &rest[..k]),
                None => rest.to_string(),
            };
            if out.last() != Some(&name) && !out.contains(&name) {
                out.push(name);
            }
        }
        out.sort();
        if out.is_empty() { None } else { Some(out) }
    }
}

/// A folder holding packages. Without a collection, a package's id is its path relative
/// to `dir` ("readers/sphinx"), and `dir`'s own package is named by its base name; with
/// one, ids are import paths in that collection ("core:fmt").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Root {
    /// The folder.
    pub dir: String,
    /// The collection it stands for ("core"), or empty for plain paths.
    pub collection: String,
}

/// A file the analysis read, for a host's dependency tracking.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileRead {
    /// The path as the reader was asked for it.
    pub path: String,
    /// What it held.
    pub text: String,
}

/// A package below a root: its id and folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageRef {
    /// The package's id (see `Root`).
    pub id: String,
    /// Its folder.
    pub dir: String,
}

/// Bounds on the work one file may cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Files larger than this many bytes are left out (`Package::too_large`).
    pub max_file_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_file_bytes: 64 << 20 }
    }
}

/// A set of roots and the packages loaded from them. Packages are analysed once, on
/// first use, and kept. It is `Send`.
pub struct Project {
    /// The folders packages are looked up in, in order.
    pub roots: Vec<Root>,
    /// Bounds on the work one file may cause.
    pub limits: Limits,
    /// Every file read, in order.
    pub files: Vec<FileRead>,
    reader: Box<dyn FileReader>,
    /// Loaded packages by id; `None` for an id no root holds.
    packages: HashMap<String, Option<Package>>,
}

impl std::fmt::Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("roots", &self.roots)
            .field("limits", &self.limits)
            .field("packages", &self.packages.len())
            .finish()
    }
}

impl Project {
    /// A project over `roots`, reading through `reader`.
    pub fn new(roots: Vec<Root>, reader: Box<dyn FileReader>) -> Project {
        Project {
            roots,
            limits: Limits::default(),
            files: Vec::new(),
            reader,
            packages: HashMap::new(),
        }
    }

    /// A project over `roots` on the file system.
    pub fn from_dirs(roots: Vec<Root>) -> Project {
        Project::new(roots, Box::new(FsReader))
    }

    /// Reads a file through the reader and remembers it.
    /// Returns its index in `files`.
    pub(crate) fn read_file(&mut self, path: &str) -> Option<usize> {
        let text = self.reader.read(path)?;
        self.files.push(FileRead { path: path.to_string(), text });
        Some(self.files.len() - 1)
    }

    /// A folder's `.odin` files that belong to its package: not tests, not hidden.
    pub(crate) fn source_files(&mut self, dir: &str) -> Vec<String> {
        let names = self.reader.list(dir).unwrap_or_default();
        names
            .into_iter()
            .filter(|n| !n.ends_with('/') && n.ends_with(".odin"))
            .filter(|n| !n.ends_with("_test.odin") && !n.starts_with('.'))
            .collect()
    }

    /// Every folder below the roots that holds a package, by id.
    pub fn list_packages(&mut self) -> Vec<PackageRef> {
        let mut out = Vec::new();
        for root in self.roots.clone() {
            self.walk_packages(&root, &root.dir, &mut out, 0);
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    fn walk_packages(&mut self, root: &Root, dir: &str, out: &mut Vec<PackageRef>, depth: u32) {
        if depth > 32 {
            return;
        }
        if !self.source_files(dir).is_empty() {
            out.push(PackageRef { id: package_id(root, dir), dir: dir.to_string() });
        }
        for name in self.reader.list(dir).unwrap_or_default() {
            let Some(sub) = name.strip_suffix('/') else {
                continue;
            };
            if !sub.starts_with('.') {
                self.walk_packages(root, &join(dir, sub), out, depth + 1);
            }
        }
    }

    /// The folder of a package id: a collection's path, or a path relative to a root
    /// without one (or the root's own base name).
    fn package_dir(&mut self, id: &str) -> Option<String> {
        let (collection, rest) = match id.split_once(':') {
            Some((c, r)) => (Some(c), r),
            None => (None, id),
        };
        for root in self.roots.clone() {
            let dir = match collection {
                Some(c) if root.collection != c => continue,
                Some(_) => join(&root.dir, rest),
                None if !root.collection.is_empty() => continue,
                None if id == base(&clean(&root.dir)) => root.dir.clone(),
                None => join(&root.dir, id),
            };
            if !self.source_files(&dir).is_empty() {
                return Some(dir);
            }
        }
        None
    }

    /// The id of the package in a folder, from the root it is under.
    pub(crate) fn dir_package_id(&self, dir: &str) -> Option<String> {
        self.roots.iter().find(|r| under(&r.dir, dir)).map(|r| package_id(r, dir))
    }

    /// Analyses the package with an id, once; later calls return the same package.
    /// `None` when no root holds a package with that id.
    pub fn load_package(&mut self, id: &str) -> Option<&Package> {
        if !self.packages.contains_key(id) {
            let package = self.package_dir(id).map(|dir| crate::analyse::analyse(self, id, &dir));
            self.packages.insert(id.to_string(), package);
        }
        self.packages.get(id).and_then(Option::as_ref)
    }
}

/// The id of the package in `dir` below `root`.
fn package_id(root: &Root, dir: &str) -> String {
    let rel = relative(&root.dir, dir);
    if !root.collection.is_empty() {
        return format!("{}:{rel}", root.collection);
    }
    if rel.is_empty() {
        return base(&clean(&root.dir)).to_string();
    }
    rel
}

/// `path` cleaned lexically: no empty, "." or resolvable ".." parts, "/" separators.
pub(crate) fn clean(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let rooted = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|&p| p != "..") => {
                parts.pop();
            }
            ".." if rooted => {}
            _ => parts.push(part),
        }
    }
    let body = parts.join("/");
    match (rooted, body.is_empty()) {
        (true, _) => format!("/{body}"),
        (false, true) => ".".to_string(),
        (false, false) => body,
    }
}

/// `a` and `b` joined and cleaned; an empty part is left out.
pub(crate) fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => String::new(),
        (true, false) => clean(b),
        (false, true) => clean(a),
        (false, false) => clean(&format!("{a}/{b}")),
    }
}

/// The last part of a path.
pub(crate) fn base(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() { "." } else { "/" };
    }
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// The folder a path is in.
pub(crate) fn dir_of(path: &str) -> String {
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(k) => clean(&path[..k]),
        None => ".".to_string(),
    }
}

/// `path` relative to `base_dir` with "/" separators, "" for the folder itself. Paths
/// unlike each other (one absolute) are related by their absolute forms; `path` cleaned
/// when they cannot be.
pub(crate) fn relative(base_dir: &str, path: &str) -> String {
    let (b, t) = (clean(base_dir), clean(path));
    if b == t {
        return String::new();
    }
    if let Some(rel) = lexical_relative(&b, &t) {
        return rel;
    }
    let absolute = |p: &str| -> Option<String> {
        if p.starts_with('/') {
            return Some(p.to_string());
        }
        let cwd = std::env::current_dir().ok()?;
        Some(join(&cwd.to_string_lossy(), p))
    };
    match (absolute(&b), absolute(&t)) {
        (Some(ab), Some(at)) => lexical_relative(&ab, &at).unwrap_or(t),
        _ => t,
    }
}

/// `target` relative to `base_dir`, both clean and alike (both absolute or both
/// relative); `None` when the base climbs out of what it shares with the target.
fn lexical_relative(base_dir: &str, target: &str) -> Option<String> {
    if base_dir.starts_with('/') != target.starts_with('/') {
        return None;
    }
    let parts = |p: &'_ str| -> Vec<String> {
        p.split('/').filter(|s| !s.is_empty() && *s != ".").map(str::to_string).collect()
    };
    let (bp, tp) = (parts(base_dir), parts(target));
    let shared = bp.iter().zip(&tp).take_while(|(a, b)| a == b).count();
    if bp[shared..].iter().any(|p| p == "..") {
        return None;
    }
    let mut out: Vec<&str> = vec![".."; bp.len() - shared];
    out.extend(tp[shared..].iter().map(String::as_str));
    if out.is_empty() {
        return Some(".".to_string());
    }
    Some(out.join("/"))
}

/// Whether `path` is `dir` or below it.
fn under(dir: &str, path: &str) -> bool {
    let (d, t) = (clean(dir), clean(path));
    d == t || (t.len() > d.len() && t.starts_with(&d) && t.as_bytes()[d.len()] == b'/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_relate_lexically() {
        assert_eq!(clean("/src//a/./b/../c/"), "/src/a/c");
        assert_eq!(clean("../x"), "../x");
        assert_eq!(join("/src/geo", "../mathx"), "/src/mathx");
        assert_eq!(relative("/src/lib/core", "/src/other/pkg"), "../../other/pkg");
        assert_eq!(relative("/src", "/src/geo"), "geo");
        assert_eq!(base("/src"), "src");
        assert!(under("/src", "/src/a") && !under("/src", "/srcx"));
    }

    // A host may keep one project behind a Mutex for many directives.
    #[test]
    fn projects_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Project>();
    }
}
