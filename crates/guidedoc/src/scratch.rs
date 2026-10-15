//! Working state that readers and renderers keep in the workspace between conversions.
//!
//! An adapter is a stateless descriptor, but parsing and rendering need working storage:
//! line tables, delimiter stacks, inline item lists, anchor tables. Building those afresh
//! for every conversion allocates each time. Instead an adapter keeps them in a struct of
//! its own, takes it from the workspace's `Scratch` when a conversion starts, clears it
//! (which keeps the capacity), and puts it back when the conversion ends. A worker that
//! converts many documents then stops allocating once its buffers fit the largest one.

use std::any::Any;
use std::fmt;

/// A store of reusable working state: for each Rust type, a pool of values that
/// earlier conversions gave back. The workspace owns one; adapters reach it through
/// `ReadContext::take_scratch` and `RenderContext::take_scratch` (and the matching
/// `put_scratch`).
///
/// Taking a value out moves it, so it borrows nothing and the adapter keeps using its
/// context freely. Nested conversions (a directive body parsed by the same reader) take
/// further values of the same type; every value given back is kept, so each nesting
/// level finds one next time. Once a type's pool exists, taking and putting back do not
/// allocate.
#[derive(Default)]
pub struct Scratch {
    /// One `Vec<T>` per type `T`, boxed so values of any type fit.
    pools: Vec<Box<dyn Any + Send>>,
}

impl fmt::Debug for Scratch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scratch").field("pools", &self.pools.len()).finish()
    }
}

impl Scratch {
    /// Takes a value of type `T` out of the store: one an earlier conversion gave back,
    /// or `T::default()` when there is none (the first conversion, or a nested one while
    /// outer ones hold the values). The value keeps whatever its last user left in it:
    /// clear what you use, keeping the capacity.
    pub fn take<T: Default + Send + 'static>(&mut self) -> T {
        self.pool::<T>().and_then(Vec::pop).unwrap_or_default()
    }

    /// Gives `value` back for a later conversion to take.
    pub fn put<T: Send + 'static>(&mut self, value: T) {
        match self.pool::<T>() {
            Some(pool) => pool.push(value),
            None => self.pools.push(Box::new(vec![value])),
        }
    }

    /// Returns the number of types the store keeps values of.
    pub fn len(&self) -> usize {
        self.pools.len()
    }

    /// Tells whether the store keeps values of no type.
    pub fn is_empty(&self) -> bool {
        self.pools.is_empty()
    }

    /// Drops every stored value, releasing its memory.
    pub fn release(&mut self) {
        self.pools = Vec::new();
    }

    /// Returns the pool of type `T`'s values, if the store has one.
    fn pool<T: Send + 'static>(&mut self) -> Option<&mut Vec<T>> {
        self.pools.iter_mut().find_map(|pool| pool.downcast_mut::<Vec<T>>())
    }
}

/// Empties `buffer` and returns its storage as a vector of another type of the same
/// size and alignment. A reader keeps a buffer of borrowed values (`Vec<Line<'s>>`,
/// borrowing one conversion's source) in its scratch as `Vec<Line<'static>>`, and turns
/// it back with `recycle` in the next conversion: the standard library collects an
/// emptied vector into one of a same-sized type in place, without allocating.
pub fn recycle<T, U>(mut buffer: Vec<T>) -> Vec<U> {
    buffer.clear();
    buffer.into_iter().filter_map(|_| None).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Lines {
        rows: Vec<u32>,
    }

    #[test]
    fn a_value_put_back_is_taken_again_with_its_capacity() {
        let mut store = Scratch::default();
        let mut lines = store.take::<Lines>();
        lines.rows.extend(0..100);
        store.put(lines);
        let mut again = store.take::<Lines>();
        assert_eq!(again.rows.len(), 100);
        again.rows.clear();
        assert!(again.rows.capacity() >= 100);
        store.put(again);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn nested_takes_get_their_own_values() {
        let mut store = Scratch::default();
        let mut outer = store.take::<Lines>();
        outer.rows.push(1);
        let mut inner = store.take::<Lines>();
        inner.rows.push(2);
        store.put(inner);
        store.put(outer);
        assert_eq!(store.take::<Lines>().rows, [1]);
        assert_eq!(store.take::<Lines>().rows, [2]);
        assert!(store.take::<Lines>().rows.is_empty());
        store.release();
        assert!(store.is_empty());
    }

    #[test]
    fn recycled_buffers_keep_their_storage() {
        let text = String::from("borrowed");
        let mut borrowed: Vec<&str> = Vec::with_capacity(32);
        borrowed.push(&text);
        let kept: Vec<&'static str> = recycle(borrowed);
        assert!(kept.is_empty() && kept.capacity() >= 32);
        let again: Vec<&str> = recycle(kept);
        assert!(again.capacity() >= 32);
    }
}
