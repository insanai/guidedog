//! An ordered JSON object.

use crate::Value;
use std::collections::HashMap;

/// Objects with more keys than this keep a hash index; smaller ones search linearly,
/// which is faster for the few keys most objects have.
const INDEX_FROM: usize = 16;

/// A JSON object: keys in insertion order, each with its value.
#[derive(Debug, Clone, Default)]
pub struct Object {
    entries: Vec<(String, Value)>,
    index: HashMap<String, usize>,
}

impl PartialEq for Object {
    /// Objects are equal when they hold the same keys and values in the same order.
    fn eq(&self, other: &Object) -> bool {
        self.entries == other.entries
    }
}

impl Object {
    /// An empty object.
    pub fn new() -> Object {
        Object::default()
    }

    /// An empty object with room for `n` keys.
    pub fn with_capacity(n: usize) -> Object {
        Object { entries: Vec::with_capacity(n), index: HashMap::new() }
    }

    /// The number of keys.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Reports whether the object has no keys.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn position(&self, key: &str) -> Option<usize> {
        if self.entries.len() > INDEX_FROM {
            self.index.get(key).copied()
        } else {
            self.entries.iter().position(|(k, _)| k == key)
        }
    }

    /// The value of `key`, borrowed; `None` when it is absent.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.position(key).map(|i| &self.entries[i].1)
    }

    /// The value of `key`, mutably borrowed; `None` when it is absent.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.position(key).map(|i| &mut self.entries[i].1)
    }

    /// Reports whether the object has `key`.
    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    /// Sets `key` to `value`, keeping its place when it already exists, and returns the
    /// value it replaces.
    pub fn insert(&mut self, key: &str, value: impl Into<Value>) -> Option<Value> {
        let value = value.into();
        match self.position(key) {
            Some(i) => Some(std::mem::replace(&mut self.entries[i].1, value)),
            None => {
                self.push(key.to_string(), value);
                None
            }
        }
    }

    /// Appends `key` without checking whether it exists; the parser uses it after its
    /// own duplicate check. A repeated key is found by lookups at its first place.
    pub(crate) fn push(&mut self, key: String, value: Value) {
        let i = self.entries.len();
        if i >= INDEX_FROM && self.index.len() == i {
            self.index.entry(key.clone()).or_insert(i);
            self.entries.push((key, value));
        } else {
            self.entries.push((key, value));
            self.reindex();
        }
    }

    /// Removes `key` and returns its value; later keys keep their order.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.position(key)?;
        let (_, value) = self.entries.remove(i);
        self.reindex();
        Some(value)
    }

    /// Rebuilds the hash index after keys moved, or clears it for a small object.
    fn reindex(&mut self) {
        self.index.clear();
        if self.entries.len() > INDEX_FROM {
            self.index.reserve(self.entries.len());
            for (i, (key, _)) in self.entries.iter().enumerate() {
                self.index.entry(key.clone()).or_insert(i);
            }
        }
    }

    /// The keys and values in insertion order, borrowed.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// The keys in insertion order, borrowed.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }
}

impl<K: Into<String>, V: Into<Value>> FromIterator<(K, V)> for Object {
    /// Collects pairs in order; a repeated key keeps its first place and its last value.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(pairs: I) -> Object {
        let mut object = Object::new();
        for (key, value) in pairs {
            let key = key.into();
            object.insert(&key, value);
        }
        object
    }
}

impl<'a> IntoIterator for &'a Object {
    type Item = (&'a str, &'a Value);
    type IntoIter = Box<dyn Iterator<Item = (&'a str, &'a Value)> + 'a>;

    /// Iterates keys and values in insertion order, as [`Object::iter`] does.
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}
