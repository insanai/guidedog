//! Reading inputs within limits: whole files charged to a ledger, standard input probed
//! one byte past its limit, and chunked comparison without holding a file.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use guidedoc::SourceId;

use crate::budget::{Ledger, budget_problem};
use crate::problem::{Problem, io_problem, memory_problem, too_large};

/// The default size limit of one input file (32 MiB).
pub const MAX_SOURCE_BYTES: usize = 32 << 20;

/// The buffer `same_contents` and `each_chunk` read through, on the stack.
const CHUNK_BYTES: usize = 16 << 10;

/// The UTF-8 byte order mark, which `read_source` and the include provider drop.
const BOM: &str = "\u{feff}";

/// An input read by [`read_source`]: its text and the name the conversion knows it by.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFile {
    /// The text, UTF-8 checked, without a byte order mark.
    pub text: String,
    /// The absolute, resolved path, or `"-"` for standard input.
    pub canonical: String,
}

/// Reads a named input, or standard input for `"-"`, of at most `limit` bytes, checks
/// that it is UTF-8 and drops a leading byte order mark. Fails with `host.io`,
/// `host.file-type`, `host.limit.source`, or `host.utf8` (with the line and column of the
/// first bad byte).
pub fn read_source(path: &str, limit: usize) -> Result<SourceFile, Problem> {
    if path == "-" {
        let data = read_stream(&mut io::stdin().lock(), "standard input", limit)?;
        let text = source_text(data, "standard input")?;
        return Ok(SourceFile { text, canonical: "-".to_string() });
    }
    let data = read_file(Path::new(path), None, limit)?;
    let canonical = std::fs::canonicalize(path).map_err(|e| io_problem(Path::new(path), &e))?;
    let Some(canonical) = canonical.to_str() else {
        return Err(Problem::new(
            "host.path",
            "PATH IS NOT UTF-8",
            path,
            "The resolved path of this file is not valid UTF-8, so includes cannot be \
             resolved against it.",
            "Rename the file or a folder above it to a UTF-8 name.",
            2,
        ));
    };
    Ok(SourceFile { text: source_text(data, path)?, canonical: canonical.to_string() })
}

/// Turns file bytes into source text: UTF-8 checked, byte order mark dropped.
pub(crate) fn source_text(data: Vec<u8>, path: &str) -> Result<String, Problem> {
    let mut text = match String::from_utf8(data) {
        Ok(text) => text,
        Err(e) => return Err(utf8_problem(path, e.as_bytes())),
    };
    if text.starts_with(BOM) {
        text.drain(..BOM.len());
    }
    Ok(text)
}

fn utf8_problem(path: &str, bytes: &[u8]) -> Problem {
    let at = match guidedoc::check_utf8(SourceId::MAIN, bytes) {
        Ok(_) => 0,
        Err(fail) => fail.diagnostic.span.start as usize,
    };
    let head = String::from_utf8_lossy(&bytes[..at.min(bytes.len())]);
    let (line, column) = gd_report::line_column(&head, head.len());
    Problem::new(
        "host.utf8",
        "SOURCE IS NOT UTF-8",
        path,
        format!("Byte {at} (line {line}, column {column}) is not valid UTF-8."),
        "Save the file as UTF-8; most editors offer it under \"Save as\" or \"Encoding\".",
        1,
    )
}

/// Reads a regular file of at most `limit` bytes whole, charging its size to `ledger`
/// before allocating; the ledger keeps the charge until the caller discharges or drops
/// it. A file that grows while it is read is refused; one that shrinks keeps its full
/// charge (fewer bytes read do not mean fewer bytes held).
pub fn read_file(
    path: &Path,
    ledger: Option<&Ledger<'_>>,
    limit: usize,
) -> Result<Vec<u8>, Problem> {
    let shown = path.display().to_string();
    let info = std::fs::metadata(path).map_err(|e| io_problem(path, &e))?;
    if !info.is_file() {
        return Err(Problem::new(
            "host.file-type",
            "EXPECTED A REGULAR FILE",
            &shown,
            "Only regular files are read.",
            "Name a file, not a folder or device.",
            2,
        ));
    }
    let size = usize::try_from(info.len()).unwrap_or(usize::MAX);
    if size > limit {
        return Err(too_large(&shown, limit));
    }
    if let Some(l) = ledger
        && !l.charge(size)
    {
        return Err(budget_problem("Reading this file", &shown, size, l.budget()));
    }
    let read = read_charged(path, size);
    if read.is_err()
        && let Some(l) = ledger
    {
        l.discharge(size);
    }
    read
}

fn read_charged(path: &Path, size: usize) -> Result<Vec<u8>, Problem> {
    let shown = path.display().to_string();
    let mut file = File::open(path).map_err(|e| io_problem(path, &e))?;
    let mut data = Vec::new();
    if data.try_reserve_exact(size).is_err() {
        return Err(memory_problem("Reading this file", &shown, size));
    }
    // Read at most `size` bytes, then probe one more on the stack to detect growth.
    (&mut file).take(size as u64).read_to_end(&mut data).map_err(|e| io_problem(path, &e))?;
    let mut extra = [0u8; 1];
    let grew = file.read(&mut extra).map_err(|e| io_problem(path, &e))? > 0;
    if grew {
        return Err(Problem::new(
            "host.io",
            "FILE CHANGED WHILE READ",
            &shown,
            "The file grew while it was being read.",
            "Wait until the program writing the file has finished, then run the command again.",
            4,
        ));
    }
    Ok(data)
}

/// Reads a stream of unknown length (a pipe, standard input) within `limit` bytes,
/// probing one byte past the limit: neither an endless pipe nor a growing file can force
/// an unbounded allocation. `name` names the stream in problems.
pub fn read_stream(stream: &mut impl Read, name: &str, limit: usize) -> Result<Vec<u8>, Problem> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let room = (limit - data.len()).min(chunk.len());
        let n = match stream.read(&mut chunk[..room.max(1)]) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_problem(Path::new(name), &e)),
        };
        if n > room {
            return Err(too_large(name, limit));
        }
        if n == 0 {
            return Ok(data);
        }
        if data.try_reserve(n).is_err() {
            return Err(memory_problem("Reading this input", name, data.len() + n));
        }
        data.extend_from_slice(&chunk[..n]);
    }
}

/// Reports whether the file at `path` holds exactly `data`, reading it in chunks and
/// stopping at the first difference; false when it cannot be read.
pub fn same_contents(path: &Path, data: &[u8]) -> bool {
    let mut at = 0;
    let read_all = each_chunk(path, |chunk| {
        let Some(expected) = data.get(at..at + chunk.len()) else { return false };
        at += chunk.len();
        expected == chunk
    });
    read_all && at == data.len()
}

/// Reads a file through a stack buffer, giving each chunk to `each` until it returns
/// false. Returns whether the whole file was read and accepted.
pub fn each_chunk(path: &Path, mut each: impl FnMut(&[u8]) -> bool) -> bool {
    let Ok(mut file) = File::open(path) else { return false };
    let mut buffer = [0u8; CHUNK_BYTES];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => return true,
            Ok(n) if !each(&buffer[..n]) => return false,
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
}
