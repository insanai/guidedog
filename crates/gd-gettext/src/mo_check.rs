//! The checks a [`MoCatalog`] makes on its keys once at load, so its lookups can trust
//! the table order and the hash table, and the copy into an editable [`Catalog`].

use std::cmp::Ordering;

use crate::catalog::{Catalog, Message};
use crate::error::{Error, memory_error};
use crate::mo_read::{MAX_HASH_PROBES, MoCatalog, hashpjw, mo_error};

/// Checks the keys are sorted and distinct, as binary search needs. Returns `None` for
/// a sorted table, or the sorted order of an unsorted one. Fails when two entries have
/// the same key: a lookup could not tell which one is meant.
pub(crate) fn sorted_order(mo: &MoCatalog) -> Result<Option<Box<[u32]>>, Error> {
    let ascending = (1..mo.count()).all(|i| mo.key(i - 1) < mo.key(i));
    if ascending {
        return Ok(None);
    }
    let mut order: Vec<u32> = Vec::new();
    order.try_reserve_exact(mo.count()).map_err(|_| memory_error(mo.name()))?;
    order.extend(0..mo.count() as u32);
    order.sort_unstable_by(|&a, &b| mo.key(a as usize).cmp(mo.key(b as usize)));
    for pair in order.windows(2) {
        if mo.key(pair[0] as usize).cmp(mo.key(pair[1] as usize)) == Ordering::Equal {
            let message = format!("Strings {} and {} have the same key.", pair[0], pair[1]);
            return Err(mo_error(mo.name(), &message));
        }
    }
    Ok(Some(order.into_boxed_slice()))
}

/// Checks a hash table the way a lookup will use it: every key must be found by
/// probing from its hash, within `MAX_HASH_PROBES` probes and before an empty slot (so
/// checking costs at most that many probes per key). Returns
/// the most probes any key took, or `None` when the table cannot be trusted (it is
/// then ignored; binary search serves instead).
pub(crate) fn hash_probes(mo: &MoCatalog, slots: &[u32]) -> Option<u32> {
    let size = u32::try_from(slots.len()).ok().filter(|&size| size > 2)?;
    let mut most = 0;
    for i in 0..mo.count() {
        let h = hashpjw(mo.key(i));
        let (mut slot, step) = (h % size, 1 + h % (size - 2));
        let mut probes = 1;
        loop {
            match slots[slot as usize] as usize {
                0 => return None,
                entry if entry == i + 1 => break,
                _ if probes >= MAX_HASH_PROBES.min(size) => return None,
                _ => {}
            }
            slot = (slot + step) % size;
            probes += 1;
        }
        most = most.max(probes);
    }
    Some(most)
}

impl From<&MoCatalog> for Catalog {
    /// Copies a compiled catalog into an editable one. Its messages have no comments or
    /// references; the first entry with an empty key is the header.
    fn from(mo: &MoCatalog) -> Catalog {
        let mut cat = Catalog::new(mo.name());
        cat.plural = crate::Lookup::plural_rule(mo).clone();
        cat.messages.reserve(mo.entry_count());
        for entry in mo.entries() {
            let strs: Vec<String> = match entry.id_plural {
                Some(_) => entry.forms().iter().map(str::to_string).collect(),
                None => vec![entry.translation.to_string()],
            };
            let m = Message {
                ctx: entry.ctx.map(str::to_string),
                id: entry.id.to_string(),
                id_plural: entry.id_plural.map(str::to_string),
                strs,
                ..Message::default()
            };
            if m.id.is_empty() && m.ctx.is_none() && cat.header.is_none() {
                cat.header = Some(m);
            } else {
                cat.messages.push(m);
            }
        }
        cat
    }
}
