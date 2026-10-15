//! The folders Typst may read while it typesets a book (typst_root.odin).
//!
//! Typst reads what the book names by paths below its root, and refuses a path that
//! leaves the root. `gd_link::book_root` is the folder the book's paths are relative
//! to: the nearest one that holds the source folder, the include roots and the graphs
//! folder. That folder may hold much else (a home folder), so Typst never reads all of
//! it:
//!
//! - the embedded compiler runs in `book_root` with `read_roots` set to the folders a
//!   build may read (`typst_folders`), checked with every link followed;
//! - the typst program has no such option, so it runs in a private folder that holds,
//!   at the same places, links to only those folders (`TypstRoot::private`).
//!
//! Either way every path the book, its template or its preamble gives reaches those
//! folders or nothing.

use std::path::{Path, PathBuf};

use gd_conf::Config;
use gd_host::Problem;

/// The folder Typst runs in. A private folder of links is removed on drop: its links,
/// never what they lead to, and the folders made to hold them.
#[derive(Debug)]
pub(crate) struct TypstRoot {
    dir: PathBuf,
    private: bool,
}

impl TypstRoot {
    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Prepares the root for the typst program: `book_root` itself when the only
    /// readable folder is it, else a private folder of links. Fails, without falling
    /// back to `book_root` (which would expose files the project never approved), when
    /// the private folder cannot be made.
    pub fn for_program(config: &Config) -> Result<TypstRoot, Problem> {
        let root = gd_link::book_root(config);
        let folders = typst_folders(config);
        if folders.len() == 1 && folders[0] == root {
            return Ok(TypstRoot { dir: root, private: false });
        }
        make_typst_root(&root, &folders)
    }

    /// The root for the embedded compiler, which confines reads itself.
    pub fn shared(config: &Config) -> TypstRoot {
        TypstRoot { dir: gd_link::book_root(config), private: false }
    }
}

impl Drop for TypstRoot {
    fn drop(&mut self) {
        if self.private {
            remove_links(&self.dir);
        }
    }
}

/// The folders Typst may read, outermost only (one inside another is reached through
/// the outer one): the readable roots, and the graphs folder when it exists.
pub(crate) fn typst_folders(config: &Config) -> Vec<PathBuf> {
    let mut all = gd_conf::readable_roots(config);
    let graphs = gd_env::doctrees_dir(config).join("graphs");
    if graphs.is_dir() {
        all.push(graphs);
    }
    // An outer folder is shorter than the folders inside it, so it comes first.
    all.sort_by_key(|f| f.as_os_str().len());
    let mut out: Vec<PathBuf> = Vec::with_capacity(all.len());
    for folder in all {
        if !out.iter().any(|kept| folder.starts_with(kept)) {
            out.push(folder);
        }
    }
    out
}

/// Makes a private folder holding, at their places below `root`, links to `folders`.
pub(crate) fn make_typst_root(root: &Path, folders: &[PathBuf]) -> Result<TypstRoot, Problem> {
    let dir = make_temp_dir().map_err(|e| typst_root_problem(root, &e))?;
    // Dropped on failure, removing what was made.
    let made = TypstRoot { dir, private: true };
    for folder in folders {
        let Ok(relative) = folder.strip_prefix(root) else {
            let outside = std::io::Error::other("the folder is outside the book's root");
            return Err(typst_root_problem(folder, &outside));
        };
        let link = made.dir.join(relative);
        let linked = match link.parent() {
            Some(parent) => {
                std::fs::create_dir_all(parent).and_then(|()| symlink_dir(folder, &link))
            }
            None => symlink_dir(folder, &link),
        };
        linked.map_err(|e| typst_root_problem(folder, &e))?;
    }
    Ok(made)
}

/// A new, empty folder in the system's temporary folder.
fn make_temp_dir() -> std::io::Result<PathBuf> {
    let base = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut last = std::io::Error::other("no free name");
    for attempt in 0..64u32 {
        let name = format!("guidedog-typst-{}-{nanos:x}-{attempt}", std::process::id());
        let dir = base.join(name);
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = e,
            Err(e) => return Err(e),
        }
    }
    Err(last)
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(not(any(unix, windows)))]
fn symlink_dir(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("this system has no symbolic links"))
}

fn typst_root_problem(path: &Path, error: &std::io::Error) -> Problem {
    Problem::new(
        "typst.root",
        "CANNOT PREPARE THE BOOK'S READABLE ROOT",
        &path.display().to_string(),
        format!(
            "The restricted Typst root could not be created: {error}. Typst was not run; \
             the previous PDF is unchanged."
        ),
        "Permit temporary folder and symbolic link creation (on Windows, enable Developer \
         Mode), keep all book inputs inside the source folder and the build folder below \
         it, or use the embedded Typst compiler (typst = \"typst\"). Then build again.",
        4,
    )
}

/// Removes the private folder: its links and the folders made to hold them, never what
/// a link leads to (links are never followed). A work list, no recursion.
fn remove_links(dir: &Path) {
    let mut folders = vec![dir.to_path_buf()];
    let mut i = 0;
    while let Some(folder) = folders.get(i).cloned() {
        i += 1;
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                let _ = remove_link(&entry.path());
            } else if kind.is_dir() {
                folders.push(entry.path());
            }
        }
    }
    // Inner folders were found after outer ones, so they go first.
    for folder in folders.iter().rev() {
        let _ = std::fs::remove_dir(folder);
    }
}

/// Removes a link (on Windows a directory link is removed as a folder).
fn remove_link(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path).or_else(|_| std::fs::remove_dir(path))
}
