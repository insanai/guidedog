//! Writing compiled MO catalogs as msgfmt does: sorted keys and a hash table, so GNU
//! gettext, musl, and [`crate::MoCatalog`] all find keys quickly.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::catalog::{Catalog, Message};
use crate::error::{Error, ErrorKind, memory_error};
use crate::mo_read::{HEADER_BYTES, MO_MAGIC, hashpjw, mo_error};

/// One entry to write: its stored key, a range of [`Compiled::keys`], and its
/// translated forms, borrowed from the catalog.
struct Entry<'a> {
    key_start: usize,
    /// The length of the key as lookups hash it: `ctx \x04 id`.
    lookup_len: usize,
    /// The length of the stored key: the lookup key, then NUL and msgid_plural.
    key_len: usize,
    strs: &'a [String],
}

impl Entry<'_> {
    fn value_len(&self) -> usize {
        self.strs.iter().map(String::len).sum::<usize>() + self.strs.len().saturating_sub(1)
    }

    fn write_value(&self, out: &mut impl Write) -> io::Result<()> {
        for (k, s) in self.strs.iter().enumerate() {
            if k > 0 {
                out.write_all(b"\0")?;
            }
            out.write_all(s.as_bytes())?;
        }
        out.write_all(b"\0")
    }
}

/// The entries to write, sorted, with every stored key in one buffer: sorting compares
/// contiguous bytes (memcmp), and the translations are never copied.
struct Compiled<'a> {
    keys: Vec<u8>,
    entries: Vec<Entry<'a>>,
}

impl<'a> Compiled<'a> {
    fn new(cat: &'a Catalog, include_fuzzy: bool) -> Compiled<'a> {
        let mut compiled =
            Compiled { keys: Vec::new(), entries: Vec::with_capacity(cat.messages.len() + 1) };
        if let Some(header) = cat.header.as_ref().filter(|h| !h.strs.is_empty()) {
            compiled.push(None, "", None, &header.strs[..1]);
        }
        let wanted = |m: &&Message| {
            !m.obsolete && (!m.fuzzy || include_fuzzy) && m.is_translated() && !is_header_key(m)
        };
        for m in cat.messages.iter().filter(wanted) {
            compiled.push(m.ctx.as_deref(), &m.id, m.id_plural.as_deref(), &m.strs);
        }
        let keys = &compiled.keys;
        // A stable sort keeps the first of equal keys first, so dedup keeps it.
        compiled.entries.sort_by(|a, b| stored_key(keys, a).cmp(stored_key(keys, b)));
        compiled.entries.dedup_by(|b, a| lookup_key(keys, a) == lookup_key(keys, b));
        compiled
    }

    fn push(&mut self, ctx: Option<&str>, id: &str, plural: Option<&str>, strs: &'a [String]) {
        let key_start = self.keys.len();
        if let Some(ctx) = ctx {
            self.keys.extend_from_slice(ctx.as_bytes());
            self.keys.push(4);
        }
        self.keys.extend_from_slice(id.as_bytes());
        let lookup_len = self.keys.len() - key_start;
        if let Some(plural) = plural {
            self.keys.push(0);
            self.keys.extend_from_slice(plural.as_bytes());
        }
        let key_len = self.keys.len() - key_start;
        self.entries.push(Entry { key_start, lookup_len, key_len, strs });
    }

    fn strings_len(&self) -> usize {
        self.entries.iter().map(|e| e.key_len + e.value_len() + 2).sum()
    }
}

fn stored_key<'k>(keys: &'k [u8], e: &Entry<'_>) -> &'k [u8] {
    &keys[e.key_start..e.key_start + e.key_len]
}

fn lookup_key<'k>(keys: &'k [u8], e: &Entry<'_>) -> &'k [u8] {
    &keys[e.key_start..e.key_start + e.lookup_len]
}

/// Compiles a catalog as msgfmt does and returns the bytes. See [`write_mo_to`].
pub fn write_mo(cat: &Catalog, include_fuzzy: bool) -> Result<Vec<u8>, Error> {
    let compiled = Compiled::new(cat, include_fuzzy);
    let tables = build_tables(&compiled, &cat.name)?;
    let mut out = Vec::new();
    let size = tables.len() + compiled.strings_len();
    out.try_reserve_exact(size).map_err(|_| memory_error(&cat.name))?;
    write_parts(&mut out, &compiled, &tables).map_err(|err| write_error(&cat.name, &err))?;
    Ok(out)
}

/// Compiles a catalog into a file, through a buffer. See [`write_mo_to`].
pub fn save_mo(cat: &Catalog, path: impl AsRef<Path>, include_fuzzy: bool) -> Result<(), Error> {
    let path = path.as_ref();
    let name = path.display().to_string();
    let file = File::create(path).map_err(|err| write_error(&name, &err))?;
    let mut out = BufWriter::new(file);
    write_mo_to(&mut out, cat, include_fuzzy)?;
    out.flush().map_err(|err| write_error(&name, &err))
}

/// Compiles a catalog into `out` as msgfmt does: the header and every translated
/// message, sorted by key, with a hash table, little-endian. Fuzzy messages are left
/// out unless `include_fuzzy`; obsolete and untranslated ones always are; of messages
/// with the same key, the first is written. The output depends only on the catalog.
///
/// The strings are written straight from the catalog; only the tables are built in
/// memory. Pass a buffered writer: entries are written piece by piece. Fails with
/// [`ErrorKind::Io`] when writing fails, and [`ErrorKind::MoFormat`] when the output
/// would pass the format's 4 GiB of offsets.
pub fn write_mo_to(out: &mut impl Write, cat: &Catalog, include_fuzzy: bool) -> Result<(), Error> {
    let compiled = Compiled::new(cat, include_fuzzy);
    let tables = build_tables(&compiled, &cat.name)?;
    write_parts(out, &compiled, &tables).map_err(|err| write_error(&cat.name, &err))
}

/// Writes the tables, then every key, then every translation.
fn write_parts(out: &mut impl Write, compiled: &Compiled<'_>, tables: &[u8]) -> io::Result<()> {
    out.write_all(tables)?;
    for e in &compiled.entries {
        out.write_all(stored_key(&compiled.keys, e))?;
        out.write_all(b"\0")?;
    }
    for e in &compiled.entries {
        e.write_value(out)?;
    }
    Ok(())
}

/// A message whose key is the header's: written only as the header.
fn is_header_key(m: &Message) -> bool {
    m.id.is_empty() && m.ctx.is_none()
}

/// Builds the header words, the two string tables, and the hash table.
fn build_tables(compiled: &Compiled<'_>, name: &str) -> Result<Vec<u8>, Error> {
    let entries = &compiled.entries;
    let n = entries.len();
    let hash_size = hash_table_size(n);
    let originals = HEADER_BYTES;
    let translations = originals + 8 * n;
    let hash_at = translations + 8 * n;
    let strings_at = hash_at + 4 * hash_size;
    let strings = compiled.strings_len();
    let word = |v: usize| {
        u32::try_from(v).map_err(|_| mo_error(name, "The compiled catalog would pass 4 GiB."))
    };
    word(strings_at + strings)?;
    let mut out = Vec::with_capacity(strings_at);
    out.extend_from_slice(&MO_MAGIC.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // revision 0
    for v in [n, originals, translations, hash_size, hash_at] {
        out.extend_from_slice(&word(v)?.to_le_bytes());
    }
    let mut at = strings_at;
    let keys = entries.iter().map(|e| e.key_len);
    let values = entries.iter().map(Entry::value_len);
    for length in keys.chain(values) {
        out.extend_from_slice(&word(length)?.to_le_bytes());
        out.extend_from_slice(&word(at)?.to_le_bytes());
        at += length + 1;
    }
    for slot in hash_table(compiled, hash_size) {
        out.extend_from_slice(&slot.to_le_bytes());
    }
    Ok(out)
}

/// GNU's hash table: entry `i` stored as `i + 1` at the slot its lookup key's hashpjw
/// names, double hashing on collisions.
fn hash_table(compiled: &Compiled<'_>, size: usize) -> Vec<u32> {
    let mut slots = vec![0u32; size];
    let size = size as u32;
    for (i, e) in compiled.entries.iter().enumerate() {
        let h = hashpjw(lookup_key(&compiled.keys, e));
        let (mut slot, step) = (h % size, 1 + h % (size - 2));
        while slots[slot as usize] != 0 {
            slot = (slot + step) % size;
        }
        slots[slot as usize] = i as u32 + 1;
    }
    slots
}

/// msgfmt's hash table size: the next odd prime above 4n/3, at least 3. The table is
/// then at most three quarters full, so it always has empty slots.
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

fn write_error(name: &str, err: &io::Error) -> Error {
    let message = format!("Cannot write {name}: {err}.");
    Error::new(ErrorKind::Io, name, &message, "Check that the directory exists and is writable.")
}
