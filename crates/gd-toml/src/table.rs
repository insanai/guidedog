//! An ordered TOML table.

use crate::Value;
use std::collections::HashMap;

/// How a table entry came to be, which decides whether later lines may extend it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// A table created only because a header named a table inside it.
    Implicit,
    /// A table defined by a `[header]`.
    Header,
    /// A table created by a dotted key such as `a.b = 1`.
    Dotted,
    /// An array created by `[[header]]`.
    ArrayOfTables,
    /// A complete value: a scalar, an inline array or an inline table.
    Frozen,
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) key: String,
    pub(crate) value: Value,
    pub(crate) origin: Origin,
}

/// Tables with more keys than this keep a hash index; smaller ones search linearly.
const INDEX_FROM: usize = 16;

/// A TOML table: keys in definition order, each with its value.
///
/// Lookups are linear for small tables and hashed for large ones.
#[derive(Debug, Clone, Default)]
pub struct Table {
    entries: Vec<Entry>,
    index: HashMap<String, usize>,
}

impl PartialEq for Table {
    fn eq(&self, other: &Table) -> bool {
        self.entries.len() == other.entries.len()
            && self
                .entries
                .iter()
                .zip(&other.entries)
                .all(|(a, b)| a.key == b.key && a.value == b.value)
    }
}

impl Table {
    /// An empty table.
    pub fn new() -> Table {
        Table::default()
    }

    /// The number of keys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Reports whether the table has no keys.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn position(&self, key: &str) -> Option<usize> {
        if self.entries.len() > INDEX_FROM {
            self.index.get(key).copied()
        } else {
            self.entries.iter().position(|e| e.key == key)
        }
    }

    /// The value of `key`, borrowed from the table; `None` when it is absent.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.position(key).map(|i| &self.entries[i].value)
    }

    /// The value of `key`, mutably borrowed; `None` when it is absent.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.position(key).map(|i| &mut self.entries[i].value)
    }

    /// Follows `path` through nested tables, as `get` does at each step; `None` when a
    /// step is missing or not a table.
    pub fn path(&self, path: &[&str]) -> Option<&Value> {
        let (last, parents) = path.split_last()?;
        let mut table = self;
        for key in parents {
            table = table.get(key)?.as_table()?;
        }
        table.get(last)
    }

    /// Reports whether the table has `key`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    /// Sets `key` to `value`, keeping its place when it already exists, and returns the
    /// value it replaces.
    pub fn insert(&mut self, key: &str, value: impl Into<Value>) -> Option<Value> {
        let value = value.into();
        if let Some(i) = self.position(key) {
            return Some(std::mem::replace(&mut self.entries[i].value, value));
        }
        self.push(key.to_string(), value, Origin::Frozen);
        None
    }

    /// Removes `key` and returns its value; later keys keep their order.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.position(key)?;
        let entry = self.entries.remove(i);
        self.reindex();
        Some(entry.value)
    }

    /// The keys and values in definition order, borrowed.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|e| (e.key.as_str(), &e.value))
    }

    /// The keys in definition order, borrowed.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.key.as_str())
    }

    pub(crate) fn push(&mut self, key: String, value: Value, origin: Origin) -> usize {
        let i = self.entries.len();
        if i >= INDEX_FROM && self.index.len() == i {
            self.index.insert(key.clone(), i);
            self.entries.push(Entry { key, value, origin });
        } else {
            self.entries.push(Entry { key, value, origin });
            self.reindex();
        }
        i
    }

    fn reindex(&mut self) {
        self.index.clear();
        if self.entries.len() > INDEX_FROM {
            self.index.reserve(self.entries.len());
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.key.clone(), i);
            }
        }
    }

    pub(crate) fn entry(&self, i: usize) -> &Entry {
        &self.entries[i]
    }

    pub(crate) fn entry_mut(&mut self, i: usize) -> &mut Entry {
        &mut self.entries[i]
    }
}

impl<'a> IntoIterator for &'a Table {
    type Item = (&'a str, &'a crate::Value);
    type IntoIter = Box<dyn Iterator<Item = (&'a str, &'a crate::Value)> + 'a>;

    /// Iterates keys and values in definition order, as [`Table::iter`] does.
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl std::str::FromStr for Table {
    type Err = crate::Error;

    /// Parses a TOML document, as [`crate::parse`] does.
    fn from_str(source: &str) -> Result<Table, crate::Error> {
        crate::parse(source)
    }
}
