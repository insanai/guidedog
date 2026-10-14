//! Reading compiled MO catalogs without copying their strings.
//!
//! The GNU MO format: a header of 32-bit words (magic, revision, count, table offsets,
//! hash table size and offset), two tables of (length, offset) pairs for the original
//! and translated strings, a hash table, and the NUL-terminated strings. A key is
//! msgctxt `\x04` msgid, followed for a plural message by NUL and msgid_plural; plural
//! translations are joined by NUL. Both byte orders are read.
//!
//! A [`MoCatalog`] owns the file's bytes in one buffer and checks them once, in linear
//! time (n log n for the overlap check), when it is made. After that every lookup is
//! infallible: it hashes the key (GNU's hashpjw and double hashing) or binary-searches
//! the sorted table, compares bytes, and returns a `&str` borrowed from the buffer.

use std::cmp::Ordering;
use std::path::Path;

use crate::catalog::{charset_in, header_field_in, is_utf8_charset};
use crate::error::{Error, ErrorKind, memory_error};
use crate::load::{LoadOptions, read_file};
use crate::lookup::{Forms, Lookup};
use crate::plural::{PluralRule, parse_plural_forms};

pub(crate) const MO_MAGIC: u32 = 0x9504_12de;
const MO_MAGIC_SWAPPED: u32 = 0xde12_0495;
pub(crate) const HEADER_BYTES: usize = 28;

/// A hash table whose keys take more probes than this is not used: binary search is
/// then as fast, and a hostile table cannot make a lookup slow.
pub(crate) const MAX_HASH_PROBES: u32 = 64;

/// How a [`MoCatalog`] finds a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoSearch {
    /// Through the file's hash table: one to a few probes. `probes` is the most any key
    /// of the file takes, so a lookup of a missing key stops there too.
    Hash {
        /// The table's size (a prime).
        size: u32,
        /// The table's byte offset in the file.
        offset: usize,
        /// The most probes a key of the file takes.
        probes: u32,
    },
    /// Binary search on the original-string table, which is sorted.
    Sorted,
    /// Binary search through an order built at load, for a file whose table is not
    /// sorted (msgfmt always sorts it).
    Permuted(Box<[u32]>),
}

/// One message of a [`MoCatalog`], borrowed from its buffer.
#[derive(Debug, Clone, Copy)]
pub struct MoEntry<'a> {
    /// The msgctxt, if any.
    pub ctx: Option<&'a str>,
    /// The msgid.
    pub id: &'a str,
    /// The msgid_plural of a plural message.
    pub id_plural: Option<&'a str>,
    /// The translation as stored: plural forms joined by NUL.
    pub translation: &'a str,
}

impl<'a> MoEntry<'a> {
    /// Returns the translated forms.
    pub fn forms(&self) -> Forms<'a> {
        Forms::from_joined(self.translation)
    }
}

/// A compiled catalog served straight from its file's bytes.
///
/// It owns one buffer; lookups ([`Lookup`]) borrow from it and allocate nothing. Make one
/// with [`MoCatalog::load`] or [`MoCatalog::from_bytes`]; for an editable catalog, use
/// [`crate::parse_mo`] instead.
#[derive(Debug, Clone)]
pub struct MoCatalog {
    name: String,
    data: Box<[u8]>,
    swapped: bool,
    count: usize,
    originals: usize,
    translations: usize,
    search: MoSearch,
    plural: PluralRule,
}

impl MoCatalog {
    /// Reads and checks a compiled catalog. Fails when the file cannot be read, is
    /// larger than `options.max_bytes`, or as [`MoCatalog::from_bytes`] fails.
    pub fn load(path: impl AsRef<Path>, options: &LoadOptions) -> Result<MoCatalog, Error> {
        let path = path.as_ref();
        let data = read_file(path, options.max_bytes)?;
        MoCatalog::from_bytes(data, &path.display().to_string())
    }

    /// Takes MO bytes and checks them once, so lookups need no checks.
    ///
    /// Fails with [`ErrorKind::MoFormat`] when the header, a table entry, or a string is
    /// truncated or out of bounds, when strings share bytes or keys repeat (no compiler
    /// writes either), or when a string is not UTF-8; with [`ErrorKind::Charset`] for a
    /// header naming another charset, and [`ErrorKind::Plural`] for a bad Plural-Forms.
    pub fn from_bytes(data: impl Into<Box<[u8]>>, name: &str) -> Result<MoCatalog, Error> {
        let data = data.into();
        let header = read_header(&data, name)?;
        let mut cat = MoCatalog {
            name: name.to_string(),
            data,
            swapped: header.swapped,
            count: header.count,
            originals: header.originals,
            translations: header.translations,
            search: MoSearch::Sorted,
            plural: PluralRule::default(),
        };
        cat.check_strings()?;
        crate::mo_check::check_apart(&cat, name)?;
        cat.search = cat.choose_search(header.hash)?;
        cat.read_header_entry()?;
        Ok(cat)
    }

    /// Returns the file name, or the name given to [`MoCatalog::from_bytes`].
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the number of entries, the header included.
    pub fn entry_count(&self) -> usize {
        self.count
    }

    /// Returns how lookups find keys.
    pub fn search(&self) -> &MoSearch {
        &self.search
    }

    /// Returns the header entry's text (the translation of ""), if there is one.
    pub fn header(&self) -> Option<&str> {
        self.translation("", None)
    }

    /// Returns the value of a header field, compared case-insensitively.
    pub fn header_field(&self, name: &str) -> Option<&str> {
        header_field_in(self.header()?, name)
    }

    /// Returns the entries in file order (sorted by key in a file msgfmt wrote).
    pub fn entries(&self) -> impl Iterator<Item = MoEntry<'_>> + '_ {
        (0..self.count).map(|i| self.entry(i))
    }

    /// Returns entry `i` (`i < count`), split into its parts.
    fn entry(&self, i: usize) -> MoEntry<'_> {
        // Split off the plural first: lookups see the key up to its first NUL.
        let key = self.text(self.originals, i);
        let (singular, id_plural) = match key.split_once('\0') {
            Some((singular, plural)) => (singular, Some(plural)),
            None => (key, None),
        };
        let (ctx, id) = match singular.split_once('\x04') {
            Some((ctx, id)) => (Some(ctx), id),
            None => (None, singular),
        };
        MoEntry { ctx, id, id_plural, translation: self.text(self.translations, i) }
    }

    /// Returns the word at byte `at` in the file's byte order; 0 past the end, which
    /// the checks at load rule out for every word a lookup reads.
    pub(crate) fn word(&self, at: usize) -> u32 {
        word_in(&self.data, self.swapped, at).unwrap_or(0)
    }

    /// Returns the (length, offset) of entry `i` of a table.
    pub(crate) fn span(&self, table: usize, i: usize) -> (usize, usize) {
        let at = table + 8 * i;
        (self.word(at) as usize, self.word(at + 4) as usize)
    }

    /// Returns the bytes of entry `i` of a table, without its NUL.
    pub(crate) fn bytes(&self, table: usize, i: usize) -> &[u8] {
        let (length, offset) = self.span(table, i);
        self.data.get(offset..offset + length).unwrap_or_default()
    }

    /// Returns entry `i` of a table as text; checked as UTF-8 at load, so the
    /// conversion here cannot fail (an empty string would stand in if it did).
    fn text(&self, table: usize, i: usize) -> &str {
        std::str::from_utf8(self.bytes(table, i)).unwrap_or_default()
    }

    /// Returns the key of entry `i` as lookups compare it: up to its first NUL, so a
    /// plural message is found by its singular msgid, as GNU's `strcmp` finds it.
    pub(crate) fn key(&self, i: usize) -> &[u8] {
        let bytes = self.bytes(self.originals, i);
        let end = bytes.iter().position(|&c| c == 0).unwrap_or(bytes.len());
        &bytes[..end]
    }

    pub(crate) fn originals(&self) -> usize {
        self.originals
    }

    pub(crate) fn translations(&self) -> usize {
        self.translations
    }

    pub(crate) fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn data_len(&self) -> usize {
        self.data.len()
    }
}

/// What the header words say.
struct MoHeader {
    swapped: bool,
    count: usize,
    originals: usize,
    translations: usize,
    hash: Option<(u32, usize)>, // (size, offset), when the file has a table.
}

fn read_header(data: &[u8], name: &str) -> Result<MoHeader, Error> {
    let swapped = match word_in(data, false, 0) {
        Some(MO_MAGIC) => false,
        Some(MO_MAGIC_SWAPPED) => true,
        _ => return Err(mo_error(name, "The file does not start with the MO magic number.")),
    };
    let words: Vec<u32> = (1..7).filter_map(|k| word_in(data, swapped, 4 * k)).collect();
    let &[revision, count, originals, translations, hash_size, hash_offset] = &words[..] else {
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
    Ok(MoHeader {
        swapped,
        count,
        originals: originals as usize,
        translations: translations as usize,
        hash,
    })
}

/// Reads a word of `data` at byte `at`, or `None` past the end.
pub(crate) fn word_in(data: &[u8], swapped: bool, at: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(if swapped { u32::from_be_bytes(bytes) } else { u32::from_le_bytes(bytes) })
}

pub(crate) fn mo_error(name: &str, message: &str) -> Error {
    let hint = "Recompile the catalog from its .po file (msgfmt), or point Guidedog at the .po.";
    Error::new(ErrorKind::MoFormat, name, message, hint)
}

impl MoCatalog {
    /// Checks that every table entry lies in the file, ends in NUL, and is UTF-8.
    fn check_strings(&self) -> Result<(), Error> {
        for table in [self.originals, self.translations] {
            let table_end = self.count.checked_mul(8).and_then(|n| n.checked_add(table));
            if table_end.is_none_or(|end| end > self.data.len()) {
                return Err(mo_error(&self.name, "A string table lies outside the file."));
            }
            for i in 0..self.count {
                let (length, offset) = self.span(table, i);
                let nul = offset.checked_add(length).and_then(|end| self.data.get(end));
                if nul != Some(&0) {
                    let message = format!("String {i} points outside the file or lacks its NUL.");
                    return Err(mo_error(&self.name, &message));
                }
                if std::str::from_utf8(self.bytes(table, i)).is_err() {
                    return Err(mo_error(&self.name, &format!("String {i} is not valid UTF-8.")));
                }
            }
        }
        Ok(())
    }

    /// Picks how lookups find keys: the hash table when it is sound, else binary search
    /// on the table (or on an order built here when the table is not sorted).
    fn choose_search(&self, hash: Option<(u32, usize)>) -> Result<MoSearch, Error> {
        let order = crate::mo_check::sorted_order(self)?;
        if let Some((size, offset)) = hash
            && let Some(probes) = crate::mo_check::hash_probes(self, size, offset)
        {
            return Ok(MoSearch::Hash { size, offset, probes });
        }
        Ok(match order {
            None => MoSearch::Sorted,
            Some(order) => MoSearch::Permuted(order),
        })
    }

    /// Applies the header entry: its charset must be UTF-8, and a Plural-Forms field
    /// replaces the default rule.
    fn read_header_entry(&mut self) -> Result<(), Error> {
        let Some(header) = self.header() else { return Ok(()) };
        let set = charset_in(header);
        if !is_utf8_charset(set) {
            let message = format!("The catalog is encoded in {set}; Guidedog reads UTF-8 only.");
            let hint = "Recompile it from a UTF-8 PO file (msgconv --to-code=UTF-8, then \
                        msgfmt), or let Guidedog read the .po.";
            return Err(Error::new(ErrorKind::Charset, &self.name, &message, hint));
        }
        let Some(forms) = header_field_in(header, "Plural-Forms") else { return Ok(()) };
        let rule = parse_plural_forms(forms).map_err(|mut err| {
            self.name.clone_into(&mut err.file);
            err
        })?;
        self.plural = rule;
        Ok(())
    }

    /// Finds the entry whose key is `ctx \x04 id` (or `id`).
    pub(crate) fn find_entry(&self, id: &str, ctx: Option<&str>) -> Option<usize> {
        match &self.search {
            MoSearch::Hash { size, offset, probes } => {
                self.find_by_hash(*size, *offset, *probes, id, ctx)
            }
            MoSearch::Sorted => self.find_sorted(id, ctx, |i| i),
            MoSearch::Permuted(order) => self.find_sorted(id, ctx, |i| order[i] as usize),
        }
    }

    /// Probes the hash table as GNU gettext does, at most `probes` times.
    pub(crate) fn find_by_hash(
        &self,
        size: u32,
        offset: usize,
        probes: u32,
        id: &str,
        ctx: Option<&str>,
    ) -> Option<usize> {
        let h = hash_query(id, ctx);
        let (mut slot, step) = (h % size, 1 + h % (size - 2));
        for _ in 0..probes {
            let entry = self.word(offset + 4 * slot as usize) as usize;
            if entry == 0 {
                return None;
            }
            if entry <= self.count && compare_key(self.key(entry - 1), id, ctx).is_eq() {
                return Some(entry - 1);
            }
            slot = (slot + step) % size;
        }
        None
    }

    /// Binary search over the keys in the order `at(0..count)` gives.
    pub(crate) fn find_sorted(
        &self,
        id: &str,
        ctx: Option<&str>,
        at: impl Fn(usize) -> usize,
    ) -> Option<usize> {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let mid = low + (high - low) / 2;
            match compare_key(self.key(at(mid)), id, ctx) {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Some(at(mid)),
            }
        }
        None
    }
}

impl Lookup for MoCatalog {
    fn find(&self, id: &str, ctx: Option<&str>) -> Option<Forms<'_>> {
        let i = self.find_entry(id, ctx)?;
        Some(Forms::from_joined(self.text(self.translations, i)))
    }

    fn plural_rule(&self) -> &PluralRule {
        &self.plural
    }
}

/// Compares a stored key with the key a lookup asks for, `ctx \x04 id`, as bytes. It
/// compares slices (memcmp) rather than a chain of bytes: this is the inner loop of
/// every binary search.
pub(crate) fn compare_key(key: &[u8], id: &str, ctx: Option<&str>) -> Ordering {
    let Some(ctx) = ctx else { return key.cmp(id.as_bytes()) };
    let ctx = ctx.as_bytes();
    let head = &key[..key.len().min(ctx.len())];
    match head.cmp(ctx) {
        Ordering::Equal if key.len() == ctx.len() => Ordering::Less, // key ends first.
        Ordering::Equal => {
            let rest = &key[ctx.len()..];
            rest[0].cmp(&4).then_with(|| rest[1..].cmp(id.as_bytes()))
        }
        unequal => unequal,
    }
}

/// hashpjw of the lookup key `ctx \x04 id`, hashed slice by slice.
pub(crate) fn hash_query(id: &str, ctx: Option<&str>) -> u32 {
    let mut h = 0;
    if let Some(ctx) = ctx {
        h = hashpjw_from(h, ctx.as_bytes());
        h = hashpjw_from(h, &[4]);
    }
    hashpjw_from(h, id.as_bytes())
}

/// The hash function of GNU gettext's MO hash table (hashpjw over the key's bytes).
pub(crate) fn hashpjw(bytes: &[u8]) -> u32 {
    hashpjw_from(0, bytes)
}

/// Continues a hashpjw over more bytes.
fn hashpjw_from(mut h: u32, bytes: &[u8]) -> u32 {
    for &c in bytes {
        h = (h << 4).wrapping_add(u32::from(c));
        let g = h & 0xf000_0000;
        if g != 0 {
            h ^= g >> 24;
            h ^= g;
        }
    }
    h
}

/// Reads a compiled catalog into an editable [`crate::Catalog`] (strings copied). Use
/// [`MoCatalog::load`] to look up without copying.
///
/// Fails as [`MoCatalog::load`] fails.
pub fn load_mo(path: impl AsRef<Path>, options: &LoadOptions) -> Result<crate::Catalog, Error> {
    let mo = MoCatalog::load(path, options)?;
    let mut cat = crate::Catalog::from(&mo);
    cat.allow_fuzzy = options.allow_fuzzy;
    Ok(cat)
}

/// Decodes MO bytes into an editable [`crate::Catalog`] that copies what it keeps, so
/// `data` is only borrowed. Fails as [`MoCatalog::from_bytes`] fails, or with
/// [`ErrorKind::Memory`] when the copy cannot be allocated.
pub fn parse_mo(data: &[u8], name: &str) -> Result<crate::Catalog, Error> {
    let mut owned = Vec::new();
    owned.try_reserve_exact(data.len()).map_err(|_| memory_error(name))?;
    owned.extend_from_slice(data);
    Ok(crate::Catalog::from(&MoCatalog::from_bytes(owned, name)?))
}
