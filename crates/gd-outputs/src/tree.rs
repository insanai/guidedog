//! Removing output trees (remove.odin) and clearing the spare output.

use std::io;
use std::path::{Path, PathBuf};

use gd_host::PathKind;

/// Removes what is at `path`, a whole folder tree included, without following a
/// symbolic link: a link is removed, never what it leads to. A work list replaces
/// recursion, so deep folders cannot exhaust the stack. Nothing at `path` is nothing
/// to remove; the first entry that cannot be removed ends it with its error.
pub(crate) fn remove_tree(path: &Path) -> io::Result<()> {
    // (path, its entries are gone): a folder is visited twice, before and after them.
    let mut pending: Vec<(PathBuf, bool)> = vec![(path.to_path_buf(), false)];
    while let Some((entry, emptied)) = pending.pop() {
        let Some(kind) = gd_host::path_type(&entry)? else { continue };
        if kind != PathKind::Folder || emptied {
            gd_host::remove_path(&entry)?;
            continue;
        }
        let entries = gd_host::list_folder(&entry)?;
        pending.try_reserve(entries.len() + 1).map_err(|_| io::ErrorKind::OutOfMemory)?;
        pending.push((entry.clone(), true));
        pending.extend(entries.into_iter().map(|e| (entry.join(e.name), false)));
    }
    Ok(())
}

/// Removes a file or folder of the spare output. A folder there that a user made
/// read-only, when it was part of the output, is made writable again first: the spare
/// output is Guidedog's own, and its folders are not the output's.
pub(crate) fn remove_spare(path: &Path, kind: PathKind) -> io::Result<()> {
    match remove_tree(path) {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            if let Some(parent) = path.parent() {
                make_writable(parent);
            }
            if kind == PathKind::Folder {
                unlock(path);
            }
            remove_tree(path)
        }
        other => other,
    }
}

/// Makes a folder of the spare output, and every folder below it, writable by this
/// user again. Used below the next folder only.
pub(crate) fn unlock(dir: &Path) {
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        make_writable(&folder);
        let Ok(entries) = gd_host::list_folder(&folder) else { continue };
        let below = entries.into_iter().filter(|e| e.kind == PathKind::Folder);
        pending.extend(below.map(|e| folder.join(e.name)));
    }
}

/// Gives one folder the mode `rwxr-xr-x` (writable by its owner); errors are ignored,
/// the step that needs it fails instead.
#[cfg(unix)]
pub(crate) fn make_writable(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
}

/// Clears a folder's read-only flag; errors are ignored, the step that needs it fails
/// instead.
#[cfg(not(unix))]
pub(crate) fn make_writable(dir: &Path) {
    if let Ok(info) = std::fs::metadata(dir) {
        let mut permissions = info.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(dir, permissions);
    }
}
