//! Reading and writing compiled MO catalogs.
//!
//! The GNU MO format: a header of 32-bit words (magic, revision, count, table offsets,
//! hash table), two tables of (length, offset) pairs for original and translated
//! strings, and the NUL-terminated strings. A key is msgctxt `\x04` msgid; plural forms
//! are joined by NUL. Readers accept both byte orders; [`write_mo`] writes little-endian.

use std::path::Path;

use crate::catalog::{Catalog, Message, is_utf8_charset};
use crate::error::{Error, ErrorKind, memory_error, read_error};

const MO_MAGIC: u32 = 0x9504_12de;
const MO_MAGIC_SWAPPED: u32 = 0xde12_0495;
const HEADER_BYTES: usize = 28;

/// Reads a compiled catalog. Its messages have no comments or references.
///
/// Fails when the file cannot be read, or as [`parse_mo`] fails.
pub fn load_mo(path: impl AsRef<Path>) -> Result<Catalog, Error> {
    let path = path.as_ref();
    let name = path.display().to_string();
    let data = std::fs::read(path).map_err(|e| read_error(&name, &e))?;
    parse_mo(&data, &name)
}

/// Reads the words of MO data in its byte order.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MoReader<'a> {
    pub(crate) data: &'a [u8],
    swapped: bool,
}

impl<'a> MoReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> MoReader<'a> {
        MoReader { data, swapped: false }
    }

    /// Returns the word at byte `at`, or `None` past the end.
    pub(crate) fn word(&self, at: usize) -> Option<u32> {
        let bytes: [u8; 4] = self.data.get(at..at.checked_add(4)?)?.try_into().ok()?;
        Some(if self.swapped { u32::from_be_bytes(bytes) } else { u32::from_le_bytes(bytes) })
    }

    /// Returns the (length, offset) of entry `i` of the table at `table`.
    fn span(&self, table: usize, i: usize) -> Option<(u32, u32)> {
        let at = table.checked_add(i.checked_mul(8)?)?;
        Some((self.word(at)?, self.word(at.checked_add(4)?)?))
    }

    /// Returns entry `i` of the table at `table`, which must be followed by its NUL.
    pub(crate) fn string(&self, table: usize, i: usize) -> Option<&'a [u8]> {
        let (length, offset) = self.span(table, i)?;
        let end = offset as usize + length as usize;
        if end >= self.data.len() {
            return None;
        }
        Some(&self.data[offset as usize..end])
    }
}

/// Decodes MO bytes into a catalog that copies what it keeps, so `data` is only
/// borrowed. Every offset is checked: a truncated or damaged file, one whose strings
/// share bytes, or strings that are not UTF-8 give an error of kind
/// [`ErrorKind::MoFormat`]; a header naming another charset gives
/// [`ErrorKind::Charset`], and a bad Plural-Forms header [`ErrorKind::Plural`].
pub fn parse_mo(data: &[u8], name: &str) -> Result<Catalog, Error> {
    let mut r = MoReader::new(data);
    match r.word(0) {
        Some(MO_MAGIC) => {}
        Some(MO_MAGIC_SWAPPED) => r.swapped = true,
        _ => return Err(mo_error(name, "The file does not start with the MO magic number.")),
    }
    let revision = r.word(4).unwrap_or(0);
    let (Some(count), Some(originals), Some(translations)) = (r.word(8), r.word(12), r.word(16))
    else {
        return Err(mo_error(name, "The MO header is truncated or has an unknown revision."));
    };
    if revision >> 16 > 1 {
        return Err(mo_error(name, "The MO header is truncated or has an unknown revision."));
    }
    let (count, originals, translations) =
        (count as usize, originals as usize, translations as usize);
    if count.saturating_mul(8) > data.len() {
        return Err(mo_error(name, "The MO file claims more messages than it can hold."));
    }
    check_apart(&r, count, [originals, translations], name)?;
    let mut cat = Catalog::new(name);
    cat.messages.try_reserve(count).map_err(|_| memory_error(name))?;
    for i in 0..count {
        let (Some(key), Some(value)) = (r.string(originals, i), r.string(translations, i)) else {
            return Err(mo_error(name, &format!("String {i} points outside the file.")));
        };
        let (Ok(key), Ok(value)) = (std::str::from_utf8(key), std::str::from_utf8(value)) else {
            return Err(mo_error(name, &format!("String {i} is not valid UTF-8.")));
        };
        add_entry(&mut cat, key, value);
    }
    mo_header(&mut cat, name)?;
    Ok(cat)
}

/// Refuses a file whose strings share bytes. msgfmt and every other compiler write each
/// string once, apart from the others; a file whose table points many entries at the
/// same bytes would make each entry check and copy them again, so a small file could
/// cost as much as a large one. Empty strings hold no bytes and may point anywhere.
/// Sorting the spans makes the check n log n; afterwards reading costs the file's size.
fn check_apart(
    r: &MoReader<'_>,
    count: usize,
    tables: [usize; 2],
    name: &str,
) -> Result<(), Error> {
    let mut spans: Vec<(u32, u32, usize)> = Vec::new();
    spans.try_reserve(2 * count).map_err(|_| memory_error(name))?;
    for i in 0..count {
        for table in tables {
            let Some((length, offset)) = r.span(table, i) else {
                return Err(mo_error(name, &format!("String {i} points outside the file.")));
            };
            if length > 0 {
                spans.push((offset, length, i));
            }
        }
    }
    spans.sort_unstable_by_key(|&(offset, _, _)| offset);
    for pair in spans.windows(2) {
        let ((a_offset, a_length, a), (b_offset, _, b)) = (pair[0], pair[1]);
        if u64::from(b_offset) < u64::from(a_offset) + u64::from(a_length) {
            let message = format!(
                "Strings {a} and {b} share bytes; a compiled catalog keeps each string apart."
            );
            return Err(mo_error(name, &message));
        }
    }
    Ok(())
}

fn add_entry(cat: &mut Catalog, key: &str, value: &str) {
    let mut m = Message::default();
    let mut id = key;
    if let Some((ctx, rest)) = key.split_once('\x04') {
        m.ctx = Some(ctx.to_string());
        id = rest;
    }
    if let Some((singular, plural)) = id.split_once('\0') {
        m.id_plural = Some(plural.to_string());
        id = singular;
    }
    m.id = id.to_string();
    if m.id_plural.is_some() {
        m.strs.extend(value.split('\0').map(str::to_string));
    } else {
        m.strs.push(value.to_string());
    }
    if m.id.is_empty() && m.ctx.is_none() && cat.header.is_none() {
        cat.header = Some(m);
    } else {
        cat.messages.push(m);
    }
}

fn mo_header(cat: &mut Catalog, name: &str) -> Result<(), Error> {
    let set = cat.charset();
    if !is_utf8_charset(set) {
        let message = format!("The catalog is encoded in {set}; Guidedog reads UTF-8 only.");
        let hint = "Recompile it from a UTF-8 PO file (msgconv --to-code=UTF-8, then msgfmt), \
                    or let Guidedog read the .po.";
        return Err(Error::new(ErrorKind::Charset, name, &message, hint));
    }
    let Some(forms) = cat.header_field("Plural-Forms") else { return Ok(()) };
    match crate::parse_plural_forms(forms) {
        Ok(rule) => {
            cat.plural = rule;
            Ok(())
        }
        Err(mut err) => {
            name.clone_into(&mut err.file);
            Err(err)
        }
    }
}

fn mo_error(name: &str, message: &str) -> Error {
    let hint = "Recompile the catalog from its .po file (msgfmt), or point Guidedog at the .po.";
    Error::new(ErrorKind::MoFormat, name, message, hint)
}

/// Compiles a catalog as msgfmt does: the header and every translated message, sorted
/// by key, with a hash table, little-endian. Fuzzy messages are left out unless
/// `include_fuzzy`; obsolete and untranslated ones always are.
///
/// Fails with [`ErrorKind::Memory`] when the output cannot be allocated, and with
/// [`ErrorKind::MoFormat`] when it would pass the format's 4 GiB of offsets.
pub fn write_mo(cat: &Catalog, include_fuzzy: bool) -> Result<Vec<u8>, Error> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(text) = cat.header.as_ref().and_then(|h| h.strs.first()) {
        pairs.push((String::new(), text.clone()));
    }
    for m in &cat.messages {
        if m.obsolete || (m.fuzzy && !include_fuzzy) || !m.is_translated() {
            continue;
        }
        let mut key = crate::catalog::message_key(m);
        if let Some(plural) = &m.id_plural {
            key.push('\0');
            key.push_str(plural);
        }
        pairs.push((key, m.strs.join("\0")));
    }
    pairs.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    encode(&pairs, &cat.name)
}

fn encode(pairs: &[(String, String)], name: &str) -> Result<Vec<u8>, Error> {
    let n = pairs.len();
    let hash_size = hash_table_size(n);
    let originals = HEADER_BYTES;
    let translations = originals + 8 * n;
    let hash_at = translations + 8 * n;
    let strings_at = hash_at + 4 * hash_size;
    let total = strings_at + pairs.iter().map(|(k, v)| k.len() + v.len() + 2).sum::<usize>();
    let too_large = || mo_error(name, "The compiled catalog would pass 4 GiB.");
    let word = |v: usize| u32::try_from(v).map_err(|_| too_large());
    word(total)?;
    let mut out = Vec::new();
    out.try_reserve_exact(total).map_err(|_| memory_error(name))?;
    out.resize(total, 0);
    let header = [MO_MAGIC, 0, n as u32, originals as u32, translations as u32];
    for (k, v) in header.into_iter().chain([hash_size as u32, hash_at as u32]).enumerate() {
        put(&mut out, 4 * k, v);
    }
    let mut at = strings_at;
    let keys = pairs.iter().map(|(k, _)| k);
    let values = pairs.iter().map(|(_, v)| v);
    for (i, s) in keys.chain(values).enumerate() {
        put(&mut out, originals + 8 * i, s.len() as u32);
        put(&mut out, originals + 8 * i + 4, at as u32);
        out[at..at + s.len()].copy_from_slice(s.as_bytes());
        at += s.len() + 1;
    }
    let size = hash_size as u32;
    for (i, (key, _)) in pairs.iter().enumerate() {
        let h = hashpjw(key.as_bytes());
        let (mut slot, step) = (h % size, 1 + h % (size - 2));
        while read_word(&out, hash_at + 4 * slot as usize) != 0 {
            slot = (slot + step) % size;
        }
        put(&mut out, hash_at + 4 * slot as usize, i as u32 + 1);
    }
    Ok(out)
}

fn put(out: &mut [u8], at: usize, v: u32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn read_word(out: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([out[at], out[at + 1], out[at + 2], out[at + 3]])
}

/// msgfmt's hash table size: the next odd prime above 4n/3, at least 3.
fn hash_table_size(n: usize) -> usize {
    let mut size = (n * 4).div_ceil(3).max(3);
    if size.is_multiple_of(2) {
        size += 1;
    }
    while !is_prime(size) {
        size += 2;
    }
    size
}

fn is_prime(n: usize) -> bool {
    n >= 2 && (2..).take_while(|d| d * d <= n).all(|d| !n.is_multiple_of(d))
}

/// The hash function of GNU gettext's MO hash table.
pub(crate) fn hashpjw(s: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &c in s {
        h = (h << 4).wrapping_add(u32::from(c));
        let g = h & 0xf000_0000;
        if g != 0 {
            h ^= g >> 24;
            h ^= g;
        }
    }
    h
}
