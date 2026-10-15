//! Bounded symbol tables. A table never holds more than the capacity it was made with,
//! so hostile input costs bounded memory and ends in a `Full` status rather than
//! unbounded growth. The standard hasher resists collision attacks, so lookups need no
//! probe limit.

use std::collections::HashMap;

/// Why `SymbolTable::insert` left the table unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolClash {
    /// The key was present, with this value.
    Duplicate(u32),
    /// The table holds its capacity.
    Full,
}

/// A bounded map from names to `u32` values.
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    map: HashMap<String, u32>,
    capacity: usize,
}

impl SymbolTable {
    /// Makes a table for about `capacity` names. It holds at least 8 and rounds
    /// `capacity` up to a power of two, so the bound a caller sees is predictable.
    pub fn with_capacity(capacity: usize) -> SymbolTable {
        let mut slots = 16usize;
        while slots < capacity.saturating_mul(2) {
            slots = slots.saturating_mul(2);
        }
        let capacity = slots / 2;
        SymbolTable { map: HashMap::with_capacity(capacity), capacity }
    }

    /// Inserts `key` with `value`. A duplicate or a full table leaves the table as it is.
    pub fn insert(&mut self, key: &str, value: u32) -> Result<(), SymbolClash> {
        if let Some(&existing) = self.map.get(key) {
            return Err(SymbolClash::Duplicate(existing));
        }
        if self.map.len() >= self.capacity {
            return Err(SymbolClash::Full);
        }
        self.map.insert(key.to_owned(), value);
        Ok(())
    }

    /// Returns the value stored for `key`.
    pub fn get(&self, key: &str) -> Option<u32> {
        self.map.get(key).copied()
    }

    /// Replaces the value of an existing key; false when it is absent.
    pub fn set(&mut self, key: &str, value: u32) -> bool {
        match self.map.get_mut(key) {
            Some(v) => {
                *v = value;
                true
            }
            None => false,
        }
    }

    /// Returns the number of names held.
    pub fn count(&self) -> usize {
        self.map.len()
    }

    /// Empties the table, keeping its storage.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_table_is_bounded() {
        let mut table = SymbolTable::with_capacity(4);
        assert_eq!(table.insert("a", 1), Ok(()));
        assert_eq!(table.insert("a", 2), Err(SymbolClash::Duplicate(1)));
        for key in ["b", "c", "d", "e", "f", "g", "h"] {
            assert_eq!(table.insert(key, 3), Ok(()));
        }
        assert_eq!(table.insert("z", 9), Err(SymbolClash::Full));
        assert_eq!(table.get("c"), Some(3));
        assert!(table.set("c", 4));
        assert_eq!(table.get("c"), Some(4));
        assert!(!table.set("zz", 1));
    }
}
