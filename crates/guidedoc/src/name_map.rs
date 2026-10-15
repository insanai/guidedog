//! A map from names to values that keeps its storage when cleared. Readers and
//! renderers index targets, labels, and slugs by name in every conversion; a
//! `HashMap<String, V>` frees its keys when cleared and allocates each one again in the
//! next conversion. `NameMap` copies every key into one shared `String` instead, so a
//! cleared map refills without allocating once it has held as many names before.

use std::fmt;
use std::hash::{BuildHasher, RandomState};

use crate::types::NONE;

/// One entry: its key's byte range in the key text, and its value.
#[derive(Debug, Clone)]
struct Entry<V> {
    start: u32,
    len: u32,
    value: V,
}

/// A map from `&str` names to values `V`, iterated in insertion order. Clearing it
/// keeps its storage. Names are hashed with the standard library's randomly keyed
/// hasher, which resists collision attacks from hostile input.
#[derive(Clone)]
pub struct NameMap<V> {
    /// Every key, one after another.
    keys: String,
    /// The entries, in insertion order.
    entries: Vec<Entry<V>>,
    /// Entry indexes found by their key's hash: open addressing with linear probing;
    /// NONE marks a free slot. Its length is zero or a power of two, at most half used.
    slots: Vec<u32>,
    hasher: RandomState,
}

impl<V> Default for NameMap<V> {
    fn default() -> Self {
        NameMap {
            keys: String::new(),
            entries: Vec::new(),
            slots: Vec::new(),
            hasher: RandomState::new(),
        }
    }
}

impl<V: fmt::Debug> fmt::Debug for NameMap<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<V> NameMap<V> {
    /// Makes an empty map; it allocates nothing until a name is inserted.
    pub fn new() -> Self {
        NameMap::default()
    }

    /// Removes every entry, keeping the storage for the next names.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.entries.clear();
        self.slots.fill(NONE);
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Tells whether the map has no entry.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Tells whether `name` has an entry.
    pub fn contains_key(&self, name: &str) -> bool {
        self.find(name).1.is_some()
    }

    /// Returns the value of `name`.
    pub fn get(&self, name: &str) -> Option<&V> {
        let index = self.find(name).1?;
        Some(&self.entries[index].value)
    }

    /// Returns the value of `name`, to change it.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut V> {
        let index = self.find(name).1?;
        Some(&mut self.entries[index].value)
    }

    /// Sets the value of `name` and returns the value it replaces, if any.
    pub fn insert(&mut self, name: &str, value: V) -> Option<V> {
        match self.find(name).1 {
            Some(index) => Some(std::mem::replace(&mut self.entries[index].value, value)),
            None => {
                self.push(name, value);
                None
            }
        }
    }

    /// Returns the value of `name`, inserting `make()` first when it has none.
    pub fn get_or_insert_with(&mut self, name: &str, make: impl FnOnce() -> V) -> &mut V {
        let index = match self.find(name).1 {
            Some(index) => index,
            None => self.push(name, make()),
        };
        &mut self.entries[index].value
    }

    /// Iterates the entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> + '_ {
        self.entries.iter().map(|e| (self.key(e), &e.value))
    }

    /// Iterates the values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.entries.iter().map(|e| &e.value)
    }

    fn key(&self, e: &Entry<V>) -> &str {
        let start = e.start as usize;
        self.keys.get(start..start + e.len as usize).unwrap_or("")
    }

    /// Finds `name`: its slot and entry index, or the free slot it would take and
    /// `None`. An empty table has no slot (`usize::MAX`).
    fn find(&self, name: &str) -> (usize, Option<usize>) {
        if self.slots.is_empty() {
            return (usize::MAX, None);
        }
        let mask = self.slots.len() - 1;
        // Truncating the hash is fine: only its low bits pick the slot.
        let mut at = self.hasher.hash_one(name) as usize & mask;
        loop {
            match self.slots[at] {
                NONE => return (at, None),
                index if self.key(&self.entries[index as usize]) == name => {
                    return (at, Some(index as usize));
                }
                _ => at = (at + 1) & mask,
            }
        }
    }

    /// Appends a new entry for `name` and returns its index.
    fn push(&mut self, name: &str, value: V) -> usize {
        let index = self.entries.len();
        let start = u32::try_from(self.keys.len()).unwrap_or(u32::MAX);
        self.keys.push_str(name);
        let len = u32::try_from(name.len()).unwrap_or(0);
        self.entries.push(Entry { start, len, value });
        if self.entries.len() * 2 > self.slots.len() {
            self.grow();
        } else {
            self.place(index);
        }
        index
    }

    /// Doubles the slots and places every entry again.
    fn grow(&mut self) {
        let size = (self.slots.len() * 2).max(16);
        self.slots.clear();
        self.slots.resize(size, NONE);
        for index in 0..self.entries.len() {
            self.place(index);
        }
    }

    /// Puts entry `index` in the free slot its key hashes to.
    fn place(&mut self, index: usize) {
        let (at, _) = self.find(self.key(&self.entries[index]));
        self.slots[at] = u32::try_from(index).unwrap_or(NONE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_values_in_insertion_order() {
        let mut map = NameMap::new();
        assert_eq!(map.insert("b", 1), None);
        assert_eq!(map.insert("a", 2), None);
        assert_eq!(map.insert("b", 3), Some(1));
        *map.get_or_insert_with("c", || 0) += 5;
        assert_eq!(map.get("b"), Some(&3));
        assert_eq!(map.get("c"), Some(&5));
        assert!(!map.contains_key("d"));
        let keys: Vec<&str> = map.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["b", "a", "c"]);
    }

    #[test]
    fn a_cleared_map_refills_within_its_storage() {
        let mut map = NameMap::new();
        for i in 0..100 {
            map.insert(&format!("name-{i}"), i);
        }
        assert_eq!(map.len(), 100);
        let capacity = (map.keys.capacity(), map.entries.capacity(), map.slots.len());
        map.clear();
        assert!(map.is_empty() && map.get("name-1").is_none());
        for i in 0..100 {
            map.insert(&format!("name-{i}"), i);
        }
        assert_eq!(map.get("name-42"), Some(&42));
        assert_eq!((map.keys.capacity(), map.entries.capacity(), map.slots.len()), capacity);
    }
}
