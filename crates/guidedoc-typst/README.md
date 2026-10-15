# guidedoc-typst

**The Typst-source renderer of Guidedoc.** A validated document becomes one Typst source
file: a fixed, trusted preamble of `gd-` helpers, then the body. It writes text only and
depends on nothing but `guidedoc`; `gd-typst` compiles the result to PDF or HTML.

```rust
use guidedoc::{Registry, Workspace, convert_text};

let registry = Registry { readers: vec![/* a reader */], renderers: vec![guidedoc_typst::RENDERER] };
let mut ws = Workspace::new();
let typst = convert_text(&mut ws, &registry, "rst", "typst", "Title\n=====\n");
```

| Item | Does |
|---|---|
| `RENDERER`, `renderer()` | the "typst" descriptor for a `guidedoc::Registry` |
| `render(ctx)` | the render function (preamble and settings only when `standalone`) |
| `PREAMBLE`, `emit_preamble(out)` | the helper definitions; a book writes them once before body-only chapters |
| `translate_math(latex, &mut String)` | the bounded LaTeX math adapter; `Err(MathError { construct })` names the first unsupported command or construct |
| `write_markup_text`, `write_string_literal`, `is_label_safe` | the encoders: inert markup text, string literals for code, URLs, labels, and paths |
| `MATH_UNSUPPORTED`, `RAW_REFUSED`, ... | the diagnostics, every one with a hint |

Policies: raw Typst is written only under `ContentPolicy::Allow` (raw content for any
other format never is); unsafe URLs keep their text under omit; untranslatable math is
an error (refuse), left out (omit), or shown as LaTeX source (allow), always reported.
Under `keep_documents_with_errors` content errors become warnings. Nesting is bounded by
`Limits::max_depth` (+64) and the stack guard (`Limits::stack_bytes`).

Source maps: with `Request::source_map`, each block, item, row, and part records the
output offset where it starts (`ws.source_map()`), so a Typst error position maps back to
its source span.

## Layout

`dispatch.rs` (the exhaustive kind routing), `blocks.rs`, `structure.rs` (lists),
`inlines.rs`, `tables.rs` (tables, figures, images), `notes.rs` (footnotes, citations,
substitutions), `sphinx.rs`, `settings.rs` (standalone set rules), `escape.rs`,
`math/` (translator, special commands and environments, the command table),
`preamble.typ`.

## Redesigns from the Odin package

- **One pass.** The Odin renderer ran twice (measure, write) and translated every
  formula twice (a probe, then the output). The Rust renderer runs once; a formula is
  translated into a reused scratch `String` and copied out only when it succeeded.
- **The math adapter is public** and writes to a `String`, returning
  `Result<(), MathError>` instead of a `(bad, ok)` pair; the construct borrows the
  formula. Multi-byte characters after a backslash, as delimiters, or in `\text` are
  handled whole (the Odin code sliced bytes).
- **Exhaustive dispatch.** One `route` and one `render_node` match over every
  `NodeKind`, with no catch-all; a kind met where its handler cannot write it fails the
  render with `core.render.kind`.
- **Errors.** `render` returns `Err(Fail)` with the first error the renderer reported,
  instead of a status field the engine inspects.
- **Anchor lookup.** The core keeps its name table private, so footnote and citation
  lookup compares a node's anchor ids with the derived key over a pre-collected list of
  notes (same result, no symbol table access).
- **The preamble** is a `.typ` file included at compile time (`include_str!`).
- **Allocation.** The Odin test that rendering makes no allocator calls is not ported:
  `build_anchors` builds its table per render, and the per-node marks are a `Vec`
  (allocated with `try_reserve`, so failure is a `Capacity` result).
- **Version.** The descriptor carries the project version, not a renderer version.

The Odin package has no parse-based tests; every test builds its document with the
builder, and the tests that compile with the typst CLI skip when `typst` is not on PATH.
