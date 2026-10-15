//! The raw bytes of an MO file: the checks made once at load, and the move of its
//! strings into one text pool.
//!
//! After the checks, the strings become one `Box<str>` and each table entry a [`Span`]
//! of it. Lookups then slice the pool with no UTF-8 check: the pool was validated as a
//! whole, and every span starts and ends on a character boundary, because each string
//! was validated on its own and is followed by a NUL.

use crate::error::{Error, memory_error};
use crate::mo_read::{MO_MAGIC, mo_error};

const MO_MAGIC_SWAPPED: u32 = 0xde12_0495;

/// A string's place in the pool: start and length in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Span {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

/// A borrowed view of MO bytes whose header has been read.
pub(crate) struct RawMo<'a> {
    data: &'a [u8],
    name: &'a str,
    swapped: bool,
    count: usize,
    tables: [usize; 2], // byte offsets of the original and the translation table.
    hash: Option<(u32, usize)>, // (size, offset), when the file has a table.
}

impl<'a> RawMo<'a> {
    /// Reads the header words: magic (either byte order), revision, count, tables.
    pub(crate) fn new(data: &'a [u8], name: &'a str) -> Result<RawMo<'a>, Error> {
        let swapped = match word_in(data, false, 0) {
            Some(MO_MAGIC) => false,
            Some(MO_MAGIC_SWAPPED) => true,
            _ => return Err(mo_error(name, "The file does not start with the MO magic number.")),
        };
        let words: Vec<u32> = (1..7).filter_map(|k| word_in(data, swapped, 4 * k)).collect();
        let &[revision, count, originals, translations, hash_size, hash_offset] = &words[..]
        else {
            return Err(mo_error(name, "The MO header is truncated."));
        };
        if revision >> 16 > 1 {
            let message =
                format!("The MO file has revision {}, which no reader knows.", revision >> 16);
            return Err(mo_error(name, &message));
        }
        let count = count as usize;
        if count.saturating_mul(16) > data.len() {
            return Err(mo_error(name, "The MO file claims more messages than it can hold."));
        }
        let hash = (hash_size > 2).then_some((hash_size, hash_offset as usize));
        let tables = [originals as usize, translations as usize];
        Ok(RawMo { data, name, swapped, count, tables, hash })
    }

    /// Returns the word at byte `at`; 0 past the end (callers check bounds first).
    fn word(&self, at: usize) -> u32 {
        word_in(self.data, self.swapped, at).unwrap_or(0)
    }

    /// Returns the (offset, length) of entry `i` of table `t` (0 originals, 1 translations).
    fn span(&self, t: usize, i: usize) -> (usize, usize) {
        let at = self.tables[t] + 8 * i;
        (self.word(at + 4) as usize, self.word(at) as usize)
    }

    /// Checks that both tables lie in the file and every string lies in it, ends in NUL,
    /// and is UTF-8.
    pub(crate) fn check_strings(&self) -> Result<(), Error> {
        for t in 0..2 {
            let end = self.count.checked_mul(8).and_then(|n| n.checked_add(self.tables[t]));
            if end.is_none_or(|end| end > self.data.len()) {
                return Err(mo_error(self.name, "A string table lies outside the file."));
            }
            for i in 0..self.count {
                let (offset, length) = self.span(t, i);
                let nul = offset.checked_add(length).and_then(|end| self.data.get(end));
                if nul != Some(&0) {
                    let message = format!("String {i} points outside the file or lacks its NUL.");
                    return Err(mo_error(self.name, &message));
                }
                if std::str::from_utf8(&self.data[offset..offset + length]).is_err() {
                    return Err(mo_error(self.name, &format!("String {i} is not valid UTF-8.")));
                }
            }
        }
        Ok(())
    }

    /// Refuses a file whose strings share bytes. msgfmt and every other compiler write
    /// each string once, apart from the others; a file whose table points many entries
    /// at the same bytes would make each entry check and copy them again, so a small
    /// file could cost as much as a large one. Empty strings hold no bytes and may point
    /// anywhere. Sorting the spans makes the check n log n.
    pub(crate) fn check_apart(&self) -> Result<(), Error> {
        // (offset, length, entry), as u32: the file's own word size keeps this list small.
        let mut spans: Vec<(u32, u32, u32)> = Vec::new();
        spans.try_reserve(2 * self.count).map_err(|_| memory_error(self.name))?;
        for t in 0..2 {
            for i in 0..self.count {
                let (offset, length) = self.span(t, i);
                if length > 0 {
                    spans.push((offset as u32, length as u32, i as u32));
                }
            }
        }
        spans.sort_unstable();
        for pair in spans.windows(2) {
            let ((a_offset, a_length, a), (b_offset, _, b)) = (pair[0], pair[1]);
            if u64::from(b_offset) < u64::from(a_offset) + u64::from(a_length) {
                let message = format!(
                    "Strings {a} and {b} share bytes; a compiled catalog keeps each string apart."
                );
                return Err(mo_error(self.name, &message));
            }
        }
        Ok(())
    }

    /// Copies the hash table's slots out of the file, when it has a table in bounds.
    pub(crate) fn hash_slots(&self) -> Option<Box<[u32]>> {
        let (size, offset) = self.hash?;
        let end = (size as usize).checked_mul(4)?.checked_add(offset)?;
        if end > self.data.len() {
            return None;
        }
        Some((0..size as usize).map(|k| self.word(offset + 4 * k)).collect())
    }

    /// Returns the spans of both tables, originals then translations (`2 * count`),
    /// absolute in the file. Empty strings get (0, 0): they hold no bytes and may point
    /// anywhere.
    pub(crate) fn spans(&self) -> Result<Vec<Span>, Error> {
        let mut spans = Vec::new();
        spans.try_reserve_exact(2 * self.count).map_err(|_| memory_error(self.name))?;
        for t in 0..2 {
            for i in 0..self.count {
                let (offset, length) = self.span(t, i);
                let start = if length == 0 { 0 } else { offset as u32 };
                spans.push(Span { start, len: length as u32 });
            }
        }
        Ok(spans)
    }
}

/// Reads a word of `data` at byte `at`, or `None` past the end.
fn word_in(data: &[u8], swapped: bool, at: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(if swapped { u32::from_be_bytes(bytes) } else { u32::from_le_bytes(bytes) })
}

/// Turns the file's bytes into the text pool and makes `spans` point into it.
///
/// In a file msgfmt wrote, the strings fill the end of the file, NUL-separated, so the
/// region from the first string to the last NUL is UTF-8 as a whole. That region is
/// moved to the front of the same buffer (no new allocation) and becomes the pool. When
/// the region holds other bytes (a gap that is not UTF-8), the strings are copied one by
/// one into a new pool instead.
pub(crate) fn into_pool(
    mut data: Vec<u8>,
    spans: &mut [Span],
    name: &str,
) -> Result<Box<str>, Error> {
    let used = || spans.iter().filter(|s| s.len > 0);
    let low = used().map(|s| s.start as usize).min().unwrap_or(0);
    let high = used().map(|s| (s.start + s.len) as usize + 1).max().unwrap_or(0);
    if low >= high {
        return Ok(Box::default());
    }
    data.truncate(high);
    data.drain(..low);
    let region = match String::from_utf8(data) {
        Ok(text) => {
            shift_spans(spans, low);
            return Ok(text.into_boxed_str());
        }
        Err(err) => err.into_bytes(),
    };
    copy_strings(&region, spans, low, name)
}

/// Makes non-empty spans relative to a pool that starts at file offset `low`.
fn shift_spans(spans: &mut [Span], low: usize) {
    for span in spans.iter_mut().filter(|s| s.len > 0) {
        span.start -= low as u32;
    }
}

/// Copies every string (each already UTF-8) into a new pool, NUL after each.
fn copy_strings(
    region: &[u8],
    spans: &mut [Span],
    low: usize,
    name: &str,
) -> Result<Box<str>, Error> {
    let total: usize = spans.iter().map(|s| s.len as usize + 1).sum();
    let mut pool = String::new();
    pool.try_reserve_exact(total).map_err(|_| memory_error(name))?;
    for span in spans.iter_mut().filter(|s| s.len > 0) {
        let start = span.start as usize - low;
        let bytes = &region[start..start + span.len as usize];
        let text =
            std::str::from_utf8(bytes).map_err(|_| mo_error(name, "A string is not UTF-8."))?;
        span.start = pool.len() as u32;
        pool.push_str(text);
        pool.push('\0');
    }
    Ok(pool.into_boxed_str())
}
