//! The file system operations a publication that replaces a whole folder needs: it
//! builds the next folder beside the current one, fills it with links (`link_file`),
//! and switches the two (`exchange`); `identity` tells them apart after a switch, and
//! `list_folder` lists a folder without opening each entry. None of these guesses: a
//! question the system cannot answer is an error, never a "no".

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::publish::is_not_found;
use crate::sys;

/// What a path names, never following a symbolic link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathKind {
    /// A regular file.
    File,
    /// A folder.
    Folder,
    /// A symbolic link, whatever it leads to.
    Link,
    /// Anything else (a device, a socket, a pipe).
    Other,
}

impl PathKind {
    fn of(kind: fs::FileType) -> PathKind {
        if kind.is_symlink() {
            PathKind::Link
        } else if kind.is_dir() {
            PathKind::Folder
        } else if kind.is_file() {
            PathKind::File
        } else {
            PathKind::Other
        }
    }
}

/// A file's or a folder's device and inode, which a rename keeps: two paths with the
/// same identity name the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Identity {
    /// The device.
    pub device: u64,
    /// The inode.
    pub inode: u64,
}

/// A name in a folder, what it names, and its inode as the listing gives it (0 where
/// the system gives none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderEntry {
    /// The entry's name.
    pub name: OsString,
    /// What it names; a symbolic link is `PathKind::Link`.
    pub kind: PathKind,
    /// The inode, or 0.
    pub inode: u64,
}

/// What is at `path`, not following a symbolic link; `None` when nothing is.
pub fn path_type(path: &Path) -> io::Result<Option<PathKind>> {
    match fs::symlink_metadata(path) {
        Ok(info) => Ok(Some(PathKind::of(info.file_type()))),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The identity of what is at `path` (not following a link); `None` when nothing is, or
/// where the system gives no inode (Windows).
pub fn identity(path: &Path) -> io::Result<Option<Identity>> {
    match fs::symlink_metadata(path) {
        Ok(info) => Ok(identity_of(&info)),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(unix)]
fn identity_of(info: &fs::Metadata) -> Option<Identity> {
    use std::os::unix::fs::MetadataExt;
    Some(Identity { device: info.dev(), inode: info.ino() })
}

#[cfg(not(unix))]
fn identity_of(_info: &fs::Metadata) -> Option<Identity> {
    None
}

/// Lists a folder's entries in the order the system gives them, each with its kind from
/// the listing itself (no entry is opened, which matters for thousands of files).
pub fn list_folder(dir: &Path) -> io::Result<Vec<FolderEntry>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = match entry.file_type() {
            Ok(kind) => PathKind::of(kind),
            Err(e) if is_not_found(&e) => continue, // removed while listed.
            Err(e) => return Err(e),
        };
        out.push(FolderEntry { name: entry.file_name(), kind, inode: entry_inode(&entry) });
    }
    Ok(out)
}

#[cfg(unix)]
fn entry_inode(entry: &fs::DirEntry) -> u64 {
    use std::os::unix::fs::DirEntryExt;
    entry.ino()
}

#[cfg(not(unix))]
fn entry_inode(_entry: &fs::DirEntry) -> u64 {
    0
}

/// Makes `to` name the same file as `from`: a hard link, or a copy where the file
/// system cannot link (FAT, some network shares), which `Ok(true)` reports and the
/// caller must flush. A symbolic link (`kind` is `PathKind::Link`) is made again, to
/// the same target. An existing `to` is an error, never replaced.
pub fn link_file(from: &Path, to: &Path, kind: PathKind) -> io::Result<bool> {
    if kind == PathKind::Link {
        let target = fs::read_link(from)?;
        make_symlink(&target, to)?;
        return Ok(false);
    }
    let Err(link_error) = fs::hard_link(from, to) else { return Ok(false) };
    if path_type(to)?.is_some() {
        return Err(link_error);
    }
    copy_new(from, to)?;
    Ok(true)
}

/// Copies `from` to a new file `to`; an existing `to` is an error.
fn copy_new(from: &Path, to: &Path) -> io::Result<()> {
    let mut source = fs::File::open(from)?;
    let mut target = fs::OpenOptions::new().write(true).create_new(true).open(to)?;
    io::copy(&mut source, &mut target)?;
    Ok(())
}

#[cfg(unix)]
fn make_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_symlink(target: &Path, link: &Path) -> io::Result<()> {
    let resolved = link.parent().unwrap_or(Path::new("")).join(target);
    if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

#[cfg(not(any(unix, windows)))]
fn make_symlink(_target: &Path, _link: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

/// Swaps the names of two existing paths in one step where the system can (Linux's
/// `renameat2` with `RENAME_EXCHANGE`, macOS's `renamex_np` with `RENAME_SWAP`), so a
/// reader sees one folder or the other, never neither. `Ok(false)`, changing nothing,
/// where the system or the file system cannot: the caller then renames in two steps.
pub fn exchange(a: &Path, b: &Path) -> io::Result<bool> {
    sys::exchange(a, b)
}

/// Removes a file, a link, or an empty folder.
pub fn remove_path(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() { fs::remove_dir(path) } else { fs::remove_file(path) }
}

/// Removes what is at `path`, a whole folder tree included, without following a
/// symbolic link: a link is removed, never what it leads to. A work list replaces
/// recursion, so deep folders cannot exhaust the stack. Nothing at `path` is nothing
/// to remove; the first entry that cannot be removed ends it with its error.
pub fn remove_tree(path: &Path) -> io::Result<()> {
    // (path, its entries are gone): a folder is visited twice, before and after them.
    let mut pending: Vec<(PathBuf, bool)> = vec![(path.to_path_buf(), false)];
    while let Some((entry, emptied)) = pending.pop() {
        let Some(kind) = path_type(&entry)? else { continue };
        if kind != PathKind::Folder || emptied {
            remove_path(&entry)?;
            continue;
        }
        let entries = list_folder(&entry)?;
        pending.try_reserve(entries.len() + 1).map_err(|_| io::ErrorKind::OutOfMemory)?;
        pending.push((entry.clone(), true));
        pending.extend(entries.into_iter().map(|e| (entry.join(e.name), false)));
    }
    Ok(())
}
