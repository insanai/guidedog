//! Pack files: a header, zlib-compressed objects, and the SHA-1 of all that.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use gd_flate::Level;
use gd_sha::Sha1;

use crate::{Error, ErrorKind, Id, Kind, start};

pub(crate) const CHUNK: usize = 64 * 1024;

/// Writes a pack file. Begin it with [`PackWriter::begin`], add exactly the number of
/// objects it was begun with, and finish it with [`PackWriter::end`]. Dropping an
/// unfinished writer closes and removes the file.
///
/// Each object is compressed whole, so memory holds one object (and its compressed form)
/// at a time, never the pack. The writer keeps both buffers between objects.
#[derive(Debug)]
pub struct PackWriter {
    /// Declared before `pending`, so the file is closed before an unfinished one is
    /// removed (Windows cannot remove an open file).
    file: File,
    pending: Pending,
    count: u64,
    added: u64,
    hash: Sha1,
    written: u64,
    /// A file's contents, read by `add_file`.
    contents: Vec<u8>,
    /// An object's zlib stream.
    compressed: Vec<u8>,
}

impl PackWriter {
    /// Creates the pack file `path`, which must not exist, for `count` objects.
    ///
    /// Fails with [`ErrorKind::TooLarge`] for more than `u32::MAX` objects and
    /// [`ErrorKind::Io`] when the file cannot be created or written.
    pub fn begin(path: &Path, count: usize) -> Result<PackWriter, Error> {
        let Ok(count32) = u32::try_from(count) else {
            return Err(Error::new(
                ErrorKind::TooLarge,
                "A pack holds up to 4,294,967,295 objects.",
                "Publish the site in smaller parts.",
            ));
        };
        let file = create_private(path).map_err(|e| Error::uncreatable(path, &e))?;
        let mut w = PackWriter {
            file,
            pending: Pending { path: path.to_path_buf(), keep: false },
            count: u64::from(count32),
            added: 0,
            hash: Sha1::new(),
            written: 0,
            contents: Vec::new(),
            compressed: Vec::with_capacity(CHUNK),
        };
        let mut header = [b'P', b'A', b'C', b'K', 0, 0, 0, 2, 0, 0, 0, 0];
        header[8..].copy_from_slice(&count32.to_be_bytes());
        w.write(&header)?;
        Ok(w)
    }

    /// Adds the object of `kind` with contents `data`.
    ///
    /// Fails with [`ErrorKind::Pack`] when the pack already has all its objects and
    /// [`ErrorKind::Io`] when the file cannot be written.
    pub fn add(&mut self, kind: Kind, data: &[u8]) -> Result<(), Error> {
        self.check_room()?;
        self.add_object(kind, data)
    }

    /// Adds the blob holding the file at `path`, which must have `size` bytes and the ID
    /// `id`: a file that changed since its ID was taken is refused
    /// ([`ErrorKind::Changed`]), not packed.
    ///
    /// Fails with [`ErrorKind::TooLarge`] when memory for the file cannot be reserved,
    /// and like [`PackWriter::add`] otherwise.
    pub fn add_file(&mut self, path: &Path, size: u64, id: Id) -> Result<(), Error> {
        self.check_room()?;
        let file = File::open(path).map_err(|e| Error::unreadable(path, &e))?;
        let mut contents = std::mem::take(&mut self.contents);
        let result = read_file(file, path, size, &mut contents);
        let result = result.and_then(|()| {
            let mut check = start(Kind::Blob, size);
            check.update(&contents);
            if contents.len() as u64 != size || Id(check.finish()) != id {
                return Err(Error::changed(path, "changed after it was planned"));
            }
            self.add_object(Kind::Blob, &contents)
        });
        self.contents = contents;
        result
    }

    /// Writes the pack's checksum, flushes it to disk and closes it; returns the pack's
    /// size in bytes.
    ///
    /// Fails with [`ErrorKind::Pack`] unless every object `begin` was given has been
    /// added, and with [`ErrorKind::Io`] when the file cannot be written. The file is
    /// removed on failure.
    pub fn end(mut self) -> Result<u64, Error> {
        if self.added != self.count {
            return Err(Error::new(
                ErrorKind::Pack,
                format!("The pack has {} of its {} objects.", self.added, self.count),
                "Add exactly as many objects as the pack was begun with.",
            ));
        }
        let sum = std::mem::take(&mut self.hash).finish();
        if self.file.write_all(&sum).and_then(|()| self.file.sync_all()).is_err() {
            return Err(Error::unwritable(&self.pending.path, "pack"));
        }
        self.pending.keep = true;
        Ok(self.written + 20)
    }

    fn check_room(&self) -> Result<(), Error> {
        if self.added < self.count {
            return Ok(());
        }
        Err(Error::new(
            ErrorKind::Pack,
            "The pack has no room for the object.",
            "Begin the pack with the number of objects it will hold.",
        ))
    }

    /// Writes the object's header and its zlib stream.
    fn add_object(&mut self, kind: Kind, data: &[u8]) -> Result<(), Error> {
        let mut header = [0u8; 16];
        let used = object_header(&mut header, kind, data.len() as u64);
        self.write(&header[..used])?;
        let mut compressed = std::mem::take(&mut self.compressed);
        compressed.clear();
        gd_flate::zlib_compress_into(&mut compressed, data, Level::Default);
        let result = self.write(&compressed);
        self.compressed = compressed;
        result?;
        self.added += 1;
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        self.hash.update(data);
        if self.file.write_all(data).is_err() {
            return Err(Error::unwritable(&self.pending.path, "pack"));
        }
        self.written += data.len() as u64;
        Ok(())
    }
}

/// The pack's path, removed when dropped unless the pack was finished.
#[derive(Debug)]
struct Pending {
    path: PathBuf,
    keep: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Writes the pack header of an object of `kind` and `size` bytes into `out`: the type
/// and the size in little-endian groups of 7 bits. Returns the bytes used.
fn object_header(out: &mut [u8; 16], kind: Kind, size: u64) -> usize {
    let mut rest = size >> 4;
    out[0] = (kind as u8) << 4 | (size & 0x0F) as u8;
    let mut used = 1;
    while rest > 0 {
        out[used - 1] |= 0x80;
        out[used] = (rest & 0x7F) as u8;
        rest >>= 7;
        used += 1;
    }
    used
}

/// Reads all of `file` into `contents` (cleared first), stopping one byte past `size` so a
/// growing file is noticed without reading it all.
fn read_file(file: File, path: &Path, size: u64, contents: &mut Vec<u8>) -> Result<(), Error> {
    contents.clear();
    let expected = usize::try_from(size).unwrap_or(usize::MAX);
    if contents.try_reserve_exact(expected).is_err() {
        return Err(Error::new(
            ErrorKind::TooLarge,
            format!("{} is too large to pack ({size} bytes).", path.display()),
            "Leave very large files out of the site, or publish from a machine with more \
             memory.",
        ));
    }
    let mut limited = file.take(size.saturating_add(1));
    limited.read_to_end(contents).map_err(|e| Error::unreadable(path, &e))?;
    Ok(())
}

/// Creates `path`, which must not exist, readable and writable by its owner only.
pub(crate) fn create_private(path: &Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
