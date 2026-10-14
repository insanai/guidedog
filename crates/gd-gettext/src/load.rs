//! Reading catalog files under a size limit.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::{Error, ErrorKind, memory_error, read_error};

/// How catalogs are loaded. Fill it with struct-update syntax:
/// `LoadOptions { allow_fuzzy: true, ..LoadOptions::default() }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadOptions {
    /// The largest file accepted, in bytes; 1 GiB by default. A larger file is refused
    /// with an error of kind [`ErrorKind::TooLarge`] before anything is read.
    pub max_bytes: u64,
    /// Look fuzzy translations up too (Sphinx's `gettext_allow_fuzzy_translations`).
    pub allow_fuzzy: bool,
}

impl Default for LoadOptions {
    fn default() -> LoadOptions {
        LoadOptions { max_bytes: 1 << 30, allow_fuzzy: false }
    }
}

/// Reads a whole file into one buffer, refusing one larger than `max_bytes`.
///
/// The size is checked before reading, and again while reading (a file may grow), so a
/// limit holds even for a pipe. The buffer is reserved with `try_reserve`, so a size the
/// allocator refuses is an error, not an abort.
pub(crate) fn read_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, Error> {
    let name = path.display().to_string();
    let mut file = File::open(path).map_err(|e| read_error(&name, &e))?;
    let size = file.metadata().map_err(|e| read_error(&name, &e))?.len();
    if size > max_bytes {
        return Err(too_large(&name, size, max_bytes));
    }
    let mut data = Vec::new();
    data.try_reserve_exact(size as usize).map_err(|_| memory_error(&name))?;
    let limit = max_bytes.saturating_add(1);
    file.by_ref().take(limit).read_to_end(&mut data).map_err(|e| read_error(&name, &e))?;
    if data.len() as u64 > max_bytes {
        return Err(too_large(&name, data.len() as u64, max_bytes));
    }
    Ok(data)
}

/// Checks a size (of a file, or of what was read so far) against the limit.
pub(crate) fn check_size(name: &str, size: u64, max_bytes: u64) -> Result<(), Error> {
    if size > max_bytes {
        return Err(too_large(name, size, max_bytes));
    }
    Ok(())
}

fn too_large(name: &str, size: u64, max_bytes: u64) -> Error {
    let message =
        format!("The catalog is {size} bytes or more, past the limit of {max_bytes} bytes.");
    let hint = "Raise LoadOptions::max_bytes if the file is meant to be this large, or split \
                the catalog into one file per text domain.";
    Error::new(ErrorKind::TooLarge, name, &message, hint)
}
