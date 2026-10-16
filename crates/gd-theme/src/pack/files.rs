//! Directory walks reject symlinks and enforce the archive's file and byte bounds.

use std::path::Path;

use super::{
    Error, Files, LOCK_FILE, Lock, MAX_BYTES, MAX_DEPTH, MAX_FILES, MAX_LOCK_BYTES, error,
    tree_digest, validate,
};

/// Reads a theme directory containing only _templates and _static trees.
/// Symlinks, special files, unsafe names and oversized trees return an error.
pub fn read_tree(root: &Path) -> Result<Files, Error> {
    regular_directory(root)?;
    let mut files = Files::new();
    let mut pending = vec![(root.to_path_buf(), String::new(), 0usize)];
    let mut bytes = 0usize;
    let mut entries_seen = 0usize;
    while let Some((dir, prefix, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(error("Theme tree exceeds its directory depth limit."));
        }
        for entry in std::fs::read_dir(&dir).map_err(io)? {
            entries_seen += 1;
            if entries_seen > MAX_FILES * (MAX_DEPTH + 1) {
                return Err(error("Theme tree exceeds its entry limit."));
            }
            let entry = entry.map_err(io)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| error("Theme filenames must be UTF-8."))?;
            let name = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            let kind = entry.file_type().map_err(io)?;
            if kind.is_dir() {
                if !matches!(name.as_str(), "_templates" | "_static") && !super::valid_path(&name)
                {
                    return Err(error("Theme directories must be beneath _templates or _static."));
                }
                pending.push((entry.path(), name, depth + 1));
            } else if kind.is_file() {
                if files.len() >= MAX_FILES {
                    return Err(error("Theme exceeds its file limit."));
                }
                if !super::valid_path(&name) {
                    return Err(error("Unsafe theme filename."));
                }
                let data = read_limited(&entry.path(), MAX_BYTES - bytes)?;
                bytes += data.len();
                files.insert(name, data);
            } else {
                return Err(error("Theme trees cannot contain symlinks or special files."));
            }
        }
    }
    validate(&files)?;
    Ok(files)
}

/// Verifies the project's lock and complete installed tree without accessing the network.
/// A missing file, added file, changed byte or symlink fails verification.
pub fn verify(project: &Path) -> Result<Lock, Error> {
    let lock = read_limited(&project.join(LOCK_FILE), MAX_LOCK_BYTES)?;
    let text = std::str::from_utf8(&lock).map_err(|_| error("Theme lock must be UTF-8."))?;
    let lock = Lock::decode(text)?;
    regular_directory(&project.join("_themes"))?;
    let files = read_tree(&lock.directory(project))?;
    if tree_digest(&files) != lock.tree_sha256 {
        return Err(error("Installed theme does not match its canonical tree checksum."));
    }
    Ok(lock)
}

fn regular_directory(path: &Path) -> Result<(), Error> {
    if !std::fs::symlink_metadata(path).map_err(io)?.is_dir() {
        return Err(error("Theme roots must be regular directories, never symlinks."));
    }
    Ok(())
}

fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    use std::io::Read;
    if !std::fs::symlink_metadata(path).map_err(io)?.is_file() {
        return Err(error("Expected a regular theme file."));
    }
    let mut data = Vec::new();
    std::fs::File::open(path)
        .map_err(io)?
        .take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(io)?;
    if data.len() > limit {
        return Err(error("Theme file exceeds its byte limit."));
    }
    Ok(data)
}

fn io(e: std::io::Error) -> Error {
    Error(e.to_string())
}
