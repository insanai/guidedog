//! Pack files: a header, zlib-compressed objects, and the SHA-1 of all that.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::deflate::Deflater;
use crate::sha1::Sha1;
use crate::{Error, ErrorKind, Id, Kind, start};

pub(crate) const CHUNK: usize = 64 * 1024;

/// Writes a pack file. Begin it with [`PackWriter::begin`], add exactly the number of
/// objects it was begun with, and finish it with [`PackWriter::end`]. Dropping an
/// unfinished writer (or calling [`PackWriter::abort`]) closes and removes the file.
#[derive(Debug)]
pub struct PackWriter {
    file: Option<File>,
    path: PathBuf,
    count: u64,
    added: u64,
    hash: Sha1,
    written: u64,
    out: Vec<u8>,
    chunk: Vec<u8>,
    z: Deflater,
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
            file: Some(file),
            path: path.to_path_buf(),
            count: u64::from(count32),
            added: 0,
            hash: Sha1::new(),
            written: 0,
            out: Vec::with_capacity(CHUNK + 1024),
            chunk: Vec::new(),
            z: Deflater::new(),
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
        self.object_header(kind, data.len() as u64)?;
        self.z.reset();
        let mut pieces = data.chunks(CHUNK).peekable();
        if data.is_empty() {
            self.compress(&[], true)?;
        }
        while let Some(piece) = pieces.next() {
            self.compress(piece, pieces.peek().is_none())?;
        }
        self.added += 1;
        Ok(())
    }

    /// Adds the blob holding the file at `path`, which must have `size` bytes and the ID
    /// `id`: a file that changed since its ID was taken is refused
    /// ([`ErrorKind::Changed`]), not packed.
    pub fn add_file(&mut self, path: &Path, size: u64, id: Id) -> Result<(), Error> {
        let mut file = File::open(path).map_err(|e| Error::unreadable(path, &e))?;
        self.object_header(Kind::Blob, size)?;
        self.z.reset();
        let mut check = start(Kind::Blob, size);
        let mut chunk = std::mem::take(&mut self.chunk);
        chunk.resize(CHUNK, 0);
        let result = self.add_stream(&mut file, path, size, &mut check, &mut chunk);
        self.chunk = chunk;
        let read = result?;
        if read != size || Id(check.finish()) != id {
            return Err(Error::changed(path, "changed after it was planned"));
        }
        self.added += 1;
        Ok(())
    }

    /// Streams `file` into the pack a chunk at a time, hashing it into `check`; returns
    /// the bytes read, stopping once more than `size` arrive.
    fn add_stream(
        &mut self,
        file: &mut File,
        path: &Path,
        size: u64,
        check: &mut Sha1,
        chunk: &mut [u8],
    ) -> Result<u64, Error> {
        let mut read = 0u64;
        loop {
            let n = match file.read(chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(Error::unreadable(path, &e)),
            };
            read += n as u64;
            if read > size {
                return Ok(read);
            }
            check.update(&chunk[..n]);
            self.compress(&chunk[..n], n == 0)?;
            if n == 0 {
                return Ok(read);
            }
        }
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
        let sum = std::mem::replace(&mut self.hash, Sha1::new()).finish();
        let unwritable = Error::unwritable(&self.path, "pack");
        let Some(mut file) = self.file.take() else { return Err(unwritable) };
        if file.write_all(&sum).and_then(|()| file.sync_all()).is_err() {
            self.file = Some(file);
            return Err(unwritable);
        }
        self.path = PathBuf::new();
        Ok(self.written + 20)
    }

    /// Closes and removes an unfinished pack.
    pub fn abort(self) {}

    fn object_header(&mut self, kind: Kind, size: u64) -> Result<(), Error> {
        if self.file.is_none() || self.added >= self.count {
            return Err(Error::new(
                ErrorKind::Pack,
                "The pack has no room for the object.",
                "Begin the pack with the number of objects it will hold.",
            ));
        }
        let mut header = [0u8; 16];
        let mut s = size;
        header[0] = (kind as u8) << 4 | (s & 0x0F) as u8;
        s >>= 4;
        let mut n = 1;
        while s > 0 {
            header[n - 1] |= 0x80;
            header[n] = (s & 0x7F) as u8;
            s >>= 7;
            n += 1;
        }
        self.write(&header[..n])
    }

    fn compress(&mut self, data: &[u8], last: bool) -> Result<(), Error> {
        let mut out = std::mem::take(&mut self.out);
        out.clear();
        self.z.push(data, last, &mut out);
        let result = self.write(&out);
        self.out = out;
        result
    }

    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.is_empty() {
            return Ok(());
        }
        self.hash.update(data);
        let ok = self.file.as_mut().is_some_and(|f| f.write_all(data).is_ok());
        if !ok {
            return Err(Error::unwritable(&self.path, "pack"));
        }
        self.written += data.len() as u64;
        Ok(())
    }
}

impl Drop for PackWriter {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.path.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
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
