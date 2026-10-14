//! The lookup index of an editable [`Catalog`].

use std::sync::OnceLock;

use crate::catalog::{Catalog, Message};
use crate::lookup::{Forms, Lookup};
use crate::plural::PluralRule;

/// No entry in a slot of the open-addressing table.
const EMPTY: u32 = u32::MAX;

/// A catalog's lookup table, built once from its messages. The translations it serves
/// are copied into one pool, each form followed by `"\n"`, so every string a lookup
/// returns is a slice of the pool (see [`Catalog::pool`]).
#[derive(Debug, Clone, Default)]
pub(crate) struct Index {
    slots: Vec<u32>,
    entries: Vec<Entry>,
    forms: Vec<(usize, usize)>,
    pool: String,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    message: Option<usize>, // None is the header.
    first: usize,
    count: usize,
}

/// The two lazily built indexes of a catalog: without fuzzy messages, and with them.
/// `OnceLock` keeps a catalog `Sync`, so threads can share one for lookups.
pub(crate) type Indexes = [OnceLock<Index>; 2];

/// FNV-1a over the MO key: msgctxt `\x04` msgid, or msgid alone.
fn key_hash(ctx: Option<&str>, id: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    if let Some(ctx) = ctx {
        ctx.bytes().for_each(&mut eat);
        eat(4);
    }
    id.bytes().for_each(&mut eat);
    h
}

fn indexable(m: &Message, fuzzy: bool) -> bool {
    !m.obsolete && (!m.fuzzy || fuzzy) && m.strs.iter().any(|s| !s.is_empty())
}

fn message_at(cat: &Catalog, k: Option<usize>) -> &Message {
    match (k, &cat.header) {
        (Some(i), _) => &cat.messages[i],
        (None, Some(h)) => h,
        (None, None) => unreachable!("an index entry names the header of a catalog without one"),
    }
}

/// The header and the messages a lookup may find, with their message numbers.
fn indexed(cat: &Catalog, fuzzy: bool) -> impl Iterator<Item = (Option<usize>, &Message)> {
    let header = cat.header.iter().map(|h| (None, h));
    let messages = cat.messages.iter().enumerate().filter(move |(_, m)| indexable(m, fuzzy));
    header.chain(messages.map(|(i, m)| (Some(i), m)))
}

fn build(cat: &Catalog, fuzzy: bool) -> Index {
    let (mut count, mut forms, mut bytes) = (0usize, 0usize, 0usize);
    for (_, m) in indexed(cat, fuzzy) {
        count += 1;
        forms += m.strs.len();
        bytes += m.strs.iter().map(|s| s.len() + 1).sum::<usize>();
    }
    let mut idx = Index {
        slots: vec![EMPTY; (2 * count).next_power_of_two().max(16)],
        entries: Vec::with_capacity(count),
        forms: Vec::with_capacity(forms),
        pool: String::with_capacity(bytes),
    };
    for (k, m) in indexed(cat, fuzzy) {
        let first = idx.forms.len();
        for s in &m.strs {
            idx.forms.push((idx.pool.len(), s.len()));
            idx.pool.push_str(s);
            idx.pool.push('\n');
        }
        idx.entries.push(Entry { message: k, first, count: m.strs.len() });
        insert_slot(&mut idx, cat, m);
    }
    idx
}

fn insert_slot(idx: &mut Index, cat: &Catalog, m: &Message) {
    let entry = idx.entries.len() - 1;
    let mask = idx.slots.len() as u64 - 1;
    let mut at = key_hash(m.ctx.as_deref(), &m.id) & mask;
    while idx.slots[at as usize] != EMPTY {
        let other = message_at(cat, idx.entries[idx.slots[at as usize] as usize].message);
        if other.same_key(m) {
            return; // the first definition wins.
        }
        at = (at + 1) & mask;
    }
    idx.slots[at as usize] = entry as u32;
}

impl Catalog {
    /// (Re)builds the lookup index now, so the first lookup does not pay for it. Call it
    /// after editing `messages` or `header` directly.
    pub fn build_index(&mut self) {
        self.drop_index();
        let fuzzy = self.allow_fuzzy;
        let idx = build(self, fuzzy);
        let _ = self.index[usize::from(fuzzy)].set(idx);
    }

    fn index(&self) -> &Index {
        let fuzzy = self.allow_fuzzy;
        self.index[usize::from(fuzzy)].get_or_init(|| build(self, fuzzy))
    }

    /// Returns the text every translation of the catalog is a slice of, so a caller can
    /// register it once as a source and give parsed translations exact spans.
    pub fn pool(&self) -> &str {
        &self.index().pool
    }
}

impl Lookup for Catalog {
    /// Finds a message in the index. A fuzzy message is found only when `allow_fuzzy` is
    /// set; obsolete and untranslated messages are never found.
    fn find(&self, id: &str, ctx: Option<&str>) -> Option<Forms<'_>> {
        let idx = self.index();
        let mask = idx.slots.len() as u64 - 1;
        let mut at = key_hash(ctx, id) & mask;
        while idx.slots[at as usize] != EMPTY {
            let e = idx.entries[idx.slots[at as usize] as usize];
            let m = message_at(self, e.message);
            if m.ctx.as_deref() == ctx && m.id == id {
                let ranges = &idx.forms[e.first..e.first + e.count];
                return Some(Forms::from_pool(&idx.pool, ranges));
            }
            at = (at + 1) & mask;
        }
        None
    }

    fn plural_rule(&self) -> &PluralRule {
        &self.plural
    }
}
