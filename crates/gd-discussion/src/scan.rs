//! Streaming a record's metadata. A catalog keeps each record's metadata, not its text,
//! so a catalog of any size costs about a kilobyte a record: a record is read through a
//! small buffer, and only its metadata and the previous short line are held.

use crate::metadata::{Sections, is_rst};
use crate::problem::{io_problem, shown};
use crate::rst::is_underline;
use crate::{MAX_RECORD_BYTES, Prefix, Problem, Result, too_large};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// The reading buffer of a record.
pub(crate) const SCAN_CHUNK: usize = 64 * 1024;
/// The longest metadata line.
pub(crate) const MAX_LINE: usize = 4096;
/// The most metadata a record may have.
pub(crate) const MAX_HEADER_BYTES: usize = 64 * 1024;
/// Longer than any section title the scan looks for.
const MAX_TITLE: usize = 64;

/// What a scan keeps between lines.
struct Scan<'p> {
    prefix: &'p Prefix,
    rst: bool,
    in_header: bool,
    header: String,
    sections: Sections,
    previous: String,
    previous_long: bool,
}

/// Reads the record at `path` through a buffer of `chunk` bytes: the metadata at its
/// start, which it returns, and which required sections it has. It stops once the rest
/// of the record cannot change either.
pub(crate) fn scan_record(
    prefix: &Prefix,
    path: &Path,
    chunk: usize,
) -> Result<(String, Sections)> {
    let ns = prefix.as_str();
    let info = std::fs::symlink_metadata(path).map_err(|e| io_problem(ns, path, &e))?;
    if !info.is_file() {
        return Err(Problem::new(
            prefix.code("file-type"),
            "Expected a regular file",
            shown(path),
            "Symbolic links and special files are not accepted.",
            "Use a regular local file.",
        ));
    }
    let large = || too_large(ns, path, "A record may be at most 16 MiB.");
    if info.len() > MAX_RECORD_BYTES {
        return Err(large());
    }
    let file = std::fs::File::open(path).map_err(|e| io_problem(ns, path, &e))?;
    let mut reader = BufReader::with_capacity(chunk.max(1), file);
    let mut s = Scan {
        prefix,
        rst: is_rst(path),
        in_header: true,
        header: String::new(),
        sections: Sections::default(),
        previous: String::new(),
        previous_long: false,
    };
    let mut line = Vec::with_capacity(MAX_LINE);
    let mut total = 0u64;
    loop {
        let (read, long) =
            read_line(&mut reader, &mut line).map_err(|e| io_problem(ns, path, &e))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_RECORD_BYTES {
            return Err(large());
        }
        let text = String::from_utf8_lossy(&line);
        end_line(&mut s, text.strip_suffix('\r').unwrap_or(&text), long)
            .map_err(|_| metadata_size_problem(prefix, path))?;
        if !s.in_header && s.sections.has_abstract && s.sections.has_references {
            break; // the rest is body text.
        }
    }
    Ok((s.header, s.sections))
}

/// Reads one line into `line`, without its `\n`, keeping at most [`MAX_LINE`] + 1 bytes
/// of it. Returns the bytes consumed (0 at the end) and whether the line was longer.
fn read_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> std::io::Result<(usize, bool)> {
    line.clear();
    let (mut total, mut long) = (0, false);
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok((total, long));
        }
        let newline = buffer.iter().position(|&c| c == b'\n');
        let piece = &buffer[..newline.unwrap_or(buffer.len())];
        let room = (MAX_LINE + 1).saturating_sub(line.len());
        long |= piece.len() > room;
        line.extend_from_slice(&piece[..piece.len().min(room)]);
        let used = newline.map_or(buffer.len(), |i| i + 1);
        reader.consume(used);
        total += used;
        if newline.is_some() {
            return Ok((total, long));
        }
    }
}

/// Handles one whole line; `Err` when the metadata is too long. Only a short line is kept
/// as the previous one, since only a short line can be a title the scan looks for.
fn end_line(s: &mut Scan<'_>, line: &str, long: bool) -> Result<(), ()> {
    if s.in_header {
        if metadata_line(s.prefix, s.rst, line) {
            if long || line.len() > MAX_LINE || s.header.len() + line.len() + 1 > MAX_HEADER_BYTES
            {
                return Err(());
            }
            s.header.push_str(line);
            s.header.push('\n');
        } else {
            s.in_header = false;
        }
    }
    if s.rst {
        rst_underline(s, line, long);
    } else {
        typst_sections(s, line);
    }
    s.previous.clear();
    s.previous.push_str(&line[..gd_report::floor_boundary(line, MAX_TITLE)]);
    s.previous_long = long || line.len() > MAX_TITLE;
    Ok(())
}

/// Tells whether `line` belongs to a record's metadata. Streaming reads and whole-record
/// edits share this boundary and its limits.
pub(crate) fn metadata_line(prefix: &Prefix, rst: bool, line: &str) -> bool {
    let trimmed = line.trim();
    if rst {
        trimmed.is_empty()
            || trimmed
                .strip_prefix(':')
                .and_then(|t| t.strip_prefix(prefix.as_str()))
                .is_some_and(|t| t.starts_with('-'))
    } else {
        !trimmed.starts_with("#show:") && !trimmed.starts_with("= ")
    }
}

/// The problem of metadata over 64 KiB, or with a line over 4 KiB.
pub(crate) fn metadata_size_problem(prefix: &Prefix, path: &Path) -> Problem {
    Problem::new(
        prefix.code("metadata"),
        "The metadata is too long",
        shown(path),
        "A record's metadata may be at most 64 KiB, with lines up to 4 KiB.",
        "Shorten the metadata fields at the start of the record.",
    )
}

/// Marks a section whose title is the previous line, as `rst::has_section` finds it.
fn rst_underline(s: &mut Scan<'_>, underline: &str, long: bool) {
    if long || s.previous_long || !is_underline(underline, s.previous.len()) {
        return;
    }
    match s.previous.as_str() {
        "Abstract" => s.sections.has_abstract = true,
        "References" | "Sources and design provenance" => s.sections.has_references = true,
        _ => {}
    }
}

fn typst_sections(s: &mut Scan<'_>, line: &str) {
    if line.contains("= Abstract") {
        s.sections.has_abstract = true;
    }
    if line.contains("= References") || line.contains("= Sources and design provenance") {
        s.sections.has_references = true;
    }
}
