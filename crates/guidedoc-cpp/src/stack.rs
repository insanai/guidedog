//! A tiny stack guard, as the engine's: it records the address of a local where a parse
//! starts and compares the address of a local in the current frame with it, so the
//! measure is exact whatever the frame sizes, the compiler, or the optimization level.

/// Measures stack use from where it was made. A guard with no budget is never
/// exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct StackGuard {
    base: usize,
    limit: usize,
}

/// The address of a local of the calling frame. The address is only compared, never
/// dereferenced.
#[inline(never)]
fn frame_address() -> usize {
    let here = 0u8;
    std::hint::black_box(&here) as *const u8 as usize
}

impl StackGuard {
    /// Starts measuring at the caller's frame with a budget of `limit` bytes.
    pub fn new(limit: usize) -> StackGuard {
        StackGuard { base: frame_address(), limit }
    }

    /// Returns the stack used between the guard's start and the caller's frame.
    pub fn used(&self) -> usize {
        self.base.abs_diff(frame_address())
    }

    /// Tells whether the next frame would take the stack past the budget.
    pub fn is_exhausted(&self) -> bool {
        self.base != 0 && self.used() >= self.limit
    }

    /// Returns the budget in bytes.
    pub fn limit(&self) -> usize {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recurse(g: &StackGuard, depth: usize) -> usize {
        if g.is_exhausted() {
            return depth;
        }
        let pad = std::hint::black_box([0u8; 256]);
        recurse(g, depth + 1) + usize::from(pad[0])
    }

    #[test]
    fn guards_stop_recursion() {
        let g = StackGuard::new(64 << 10);
        let depth = recurse(&g, 0);
        assert!(depth > 10 && depth < 64 << 10);
        assert!(!StackGuard::default().is_exhausted());
    }
}
