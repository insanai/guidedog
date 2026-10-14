//! Unit tests, ported from the Odin package's `*_test.odin` files.

mod cost;
mod fuzz;
mod lookup;
mod merge;
mod mo;
mod plural;
mod po;

/// A small deterministic random generator (xorshift64*), so the fuzz and cost tests
/// repeat exactly without an external crate.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    /// Returns a number in `0..n` (`n` > 0).
    pub(crate) fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 33) as usize % n
    }

    /// Returns one item of a non-empty slice.
    pub(crate) fn choice<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}
