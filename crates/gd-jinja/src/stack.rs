//! The stack guard.
//!
//! The parser and the renderer are recursive descent: a nested bracket, operator chain or
//! tag is a nested call, and so is a macro call, an include or a recursive loop. Three
//! limits keep hostile templates from overflowing the thread's stack:
//!
//! - `Options::max_nesting` bounds how deep a template's syntax nests (brackets, operator
//!   chains, filters, attribute chains, elif chains and nested tags), so every tree the
//!   parser builds, and every walk over it, has a bounded depth.
//! - `Options::max_recursion` bounds how deep macro calls, includes, blocks and recursive
//!   loops nest while rendering.
//! - `Options::stack_bytes` bounds the machine stack itself. The first public call on an
//!   environment records the address of a local; the parser and the evaluator stop with a
//!   Limit error once the stack has grown more than `stack_bytes` below it. This is the
//!   backstop for what depth counts cannot see, such as every level of a recursion
//!   evaluating a deeply nested expression.
//!
//! Values have no syntax, so walks over nested lists and dicts (repr, equality, tojson)
//! stop at `MAX_VALUE_DEPTH` instead.

/// How deep a template's syntax may nest by default.
pub const DEFAULT_MAX_NESTING: usize = 100;

/// How deep macros, includes, blocks and recursive loops may nest by default.
pub const DEFAULT_MAX_RECURSION: usize = 200;

/// The stack parsing and rendering may use by default. It fits the 2 MiB stack of a
/// thread Rust spawns, with room for the caller, and holds `DEFAULT_MAX_RECURSION`
/// levels of recursion and `DEFAULT_MAX_NESTING` levels of syntax with room to spare.
/// Unoptimized builds have larger frames (about 6 KiB per level of macro recursion and
/// 3.5 KiB per bracket, against 2 KiB and 1.5 KiB optimized) and get a larger budget.
pub const DEFAULT_STACK_BYTES: usize = if cfg!(debug_assertions) { 3 << 19 } else { 1 << 20 };

/// Where the outermost call on an environment started, and how far the stack may grow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StackGuard {
    pub base: usize,
    pub limit: usize,
}

/// The address of a local of a fresh frame: where the stack is now.
#[inline(never)]
pub(crate) fn stack_position() -> usize {
    let here = 0u8;
    std::hint::black_box(&here) as *const u8 as usize
}

impl StackGuard {
    /// Whether the stack has grown past the budget since the guard started. A zero guard
    /// is never exhausted.
    pub(crate) fn exhausted(self) -> bool {
        self.base != 0 && self.base.abs_diff(stack_position()) >= self.limit
    }
}
