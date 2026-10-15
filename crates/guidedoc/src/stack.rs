//! Stack guards. The readers and renderers are recursive descent: a nested construct is a
//! nested call. `Limits::max_depth` bounds how deep content may nest, but not how much
//! machine stack that takes, which depends on the functions on the path, the compiler,
//! and the optimization level. A `StackGuard` bounds the stack itself: it measures the
//! distance from where a component started and turns anything beyond
//! `Limits::stack_bytes` into a `Limit` result (`core.limit.stack`).
//!
//! The thread running a conversion needs its caller's stack, plus `stack_bytes`, plus
//! `STACK_RESERVE` for the leaf work below the deepest guard.

use crate::options::Limits;
use crate::report::{Arg, CORE_STACK, Diagnostic, arg_int, diagnostic};
use crate::types::SourceSpan;

/// The stack a reader or renderer may use when `Limits::stack_bytes` is 0. With
/// `STACK_RESERVE` it fits a 1 MiB thread, the smallest default thread stack of the
/// supported platforms (Windows).
pub const DEFAULT_STACK_BYTES: usize = 512 << 10;

/// The stack a conversion needs below its deepest guard. Debug builds have larger
/// frames, so a debug build should run conversions on a thread with more room.
pub const STACK_RESERVE: usize = 128 << 10;

/// Measures stack use from where it was made. A zero guard is never exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StackGuard {
    base: usize,
    limit: usize,
}

/// The address of a local of the calling frame. The address is only compared, never
/// dereferenced, so the cast from pointer to integer is safe.
#[inline(never)]
fn frame_address() -> usize {
    let here = 0u8;
    std::hint::black_box(&here) as *const u8 as usize
}

/// Starts measuring at the caller's frame, with the limits' stack budget.
#[must_use]
pub fn stack_guard(limits: &Limits) -> StackGuard {
    let budget = if limits.stack_bytes > 0 { limits.stack_bytes } else { DEFAULT_STACK_BYTES };
    StackGuard { base: frame_address(), limit: budget }
}

impl StackGuard {
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

    /// Returns the `Limit` diagnostic a guarded recursion reports at `span`.
    pub fn diagnostic(&self, span: SourceSpan) -> Diagnostic {
        diagnostic(
            CORE_STACK,
            span,
            arg_int(i64::try_from(self.limit).unwrap_or(i64::MAX)),
            Arg::None,
        )
    }
}

/// Tells whether the guard's budget is spent.
pub fn stack_exhausted(g: &StackGuard) -> bool {
    g.is_exhausted()
}

/// Returns the diagnostic a guard's recursion reports.
pub fn stack_diagnostic(g: &StackGuard, span: SourceSpan) -> Diagnostic {
    g.diagnostic(span)
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
        let limits = Limits { stack_bytes: 64 << 10, ..Limits::default() };
        let g = stack_guard(&limits);
        let depth = recurse(&g, 0);
        assert!(depth > 10 && depth < 64 << 10);
        assert!(!StackGuard::default().is_exhausted());
    }
}
