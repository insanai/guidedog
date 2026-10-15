//! Repeated conversion in a reused `Workspace` is bounded: converting the same text
//! again, with every built-in reader and renderer, allocates no more than the time before
//! (the workspace's node tables, text pool and output buffer are reused, so nothing grows
//! with the number of conversions), and no more than the ceiling recorded for the pair.
//!
//! This is the Rust form of the Odin suite's allocation tests, which ran each conversion
//! with no allocator at all. Rust has no per-call allocator to take away, so a counting
//! global allocator observes the conversions instead. The engine core alone converts
//! without allocating (`guidedoc`'s `steady_state_conversion_does_not_allocate`); the
//! readers and renderers still build short-lived scratch vectors per conversion (block
//! and inline item lists, anchor tables, renderer state). `CEILINGS` records today's
//! counts as a ratchet: lower an entry when a change removes allocations, and never
//! raise one without a reason.
//!
//! The allocator counts per thread, so the test harness's other threads do not disturb
//! the count. It is the only global allocator in this test binary, which is why this
//! file is a test binary of its own.

// The counting global allocator below forwards to the system allocator.
#![allow(unsafe_code)]
#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use guidedoc::{Workspace, convert_text};

/// Counts the allocations and reallocations made on the current thread.
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: every call is forwarded unchanged to the system allocator; the counter is a
// thread-local cell with a const initializer, so counting never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, passed on as is.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.with(|c| c.set(c.get() + 1));
        // SAFETY: as for `dealloc`; `size` is the caller's valid new size.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// A reStructuredText document with the common block and inline constructs.
const RST: &str = "Title\n=====\n\nA paragraph with *emphasis*, **strong**, ``code`` and a \
`link <https://example.org>`_.\n\nSection\n-------\n\n- one\n- two\n\n  nested paragraph\n\n\
1. first\n2. second\n\nterm\n   Definition.\n\n::\n\n   literal block\n\n\
.. note::\n\n   An admonition.\n\n\
+-----+-----+\n| a   | b   |\n+=====+=====+\n| 1   | 2   |\n+-----+-----+\n\n\
.. code-block:: python\n\n   print(\"hi\")\n\n> quoted\n\nA footnote [#f]_.\n\n.. [#f] Note.\n";

/// A Markdown document with the common block and inline constructs.
const MARKDOWN: &str = "# Title\n\nA paragraph with *emphasis*, **strong**, `code` and a \
[link](https://example.org).\n\n## Section\n\n- one\n- two\n\n  nested paragraph\n\n\
1. first\n2. second\n\n> quoted\n\n```python\nprint(\"hi\")\n```\n\n    indented code\n\n\
---\n\nText with an ![image](a.png) and a hard  \nbreak.\n";

/// The sample text for a reader: Markdown for the Markdown readers, RST otherwise.
fn sample(reader: &str) -> &'static str {
    match reader {
        "commonmark" | "myst" => MARKDOWN,
        _ => RST,
    }
}

/// The most allocations a repeated conversion of the samples may make, per reader and
/// renderer: a ratchet (see the module documentation).
const CEILINGS: &[(&str, &str, usize)] = &[
    ("rst", "html", 128),
    ("rst", "typst", 126),
    ("rst", "text", 138),
    ("rst", "rst", 301),
    ("sphinx", "html", 244),
    ("sphinx", "typst", 242),
    ("sphinx", "text", 254),
    ("sphinx", "rst", 418),
    ("commonmark", "html", 50),
    ("commonmark", "typst", 50),
    ("commonmark", "text", 56),
    ("commonmark", "rst", 155),
    ("myst", "html", 53),
    ("myst", "typst", 52),
    ("myst", "text", 58),
    ("myst", "rst", 157),
];

/// The allocations `f` makes on this thread.
fn allocations_of(f: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.with(Cell::get);
    f();
    ALLOCATIONS.with(Cell::get) - before
}

/// The allocations of converting `text` again in `ws`, which has converted it before;
/// the output must have `len` bytes.
fn repeat_count(
    ws: &mut Workspace,
    registry: &guidedoc::Registry,
    pair: (&str, &str),
    text: &str,
    len: usize,
) -> usize {
    allocations_of(|| {
        let out = convert_text(ws, registry, pair.0, pair.1, text);
        // Compared by length inside the measured call: a copy would allocate.
        assert_eq!(out.map(str::len).ok(), Some(len), "{pair:?} changed its output");
    })
}

#[test]
fn every_reader_and_renderer_converts_again_within_its_ceiling() {
    let registry = guidedoc_defaults::registry();
    let mut checked = 0;
    let mut failures = Vec::new();
    for reader in &registry.readers {
        for renderer in &registry.renderers {
            let pair = (reader.id, renderer.id);
            let text = sample(pair.0);
            let mut ws = Workspace::new();
            let len = convert_text(&mut ws, &registry, pair.0, pair.1, text)
                .map(str::len)
                .unwrap_or_else(|f| panic!("{pair:?} failed: {f:?}"));
            let second = repeat_count(&mut ws, &registry, pair, text, len);
            let third = repeat_count(&mut ws, &registry, pair, text, len);
            if third > second {
                failures.push(format!("{pair:?} grows: {second} then {third} allocation(s)"));
            }
            // A new pair reports its count, so its ceiling can be recorded as measured.
            let ceiling = CEILINGS.iter().find(|c| (c.0, c.1) == pair).map(|c| c.2);
            let Some(ceiling) = ceiling else {
                failures.push(format!("{pair:?} has no ceiling: record {second} in CEILINGS"));
                continue;
            };
            if second > ceiling {
                failures.push(format!("{pair:?}: {second} allocation(s), ceiling {ceiling}"));
            }
            checked += 1;
        }
    }
    assert!(checked >= 16, "expected 4 readers x 4 renderers, checked {checked}");
    assert!(failures.is_empty(), "repeated conversions:\n{}", failures.join("\n"));
}
