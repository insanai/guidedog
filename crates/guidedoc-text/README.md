# guidedoc-text

The plain-text renderer of Guidedoc, as Sphinx's text builder writes text: paragraphs
wrapped at 70 columns, underlined titles, hanging list markers, three-space indents,
grid tables with spans, and `Note:` admonitions.

```rust
let registry = guidedoc::Registry {
    readers: vec![/* a reader */],
    renderers: vec![guidedoc_text::RENDERER],
};
```

| Item | Does |
|---|---|
| `RENDERER` / `renderer()` | the `"text"` descriptor for a `Registry` |
| `render(&mut RenderContext)` | the render function |
| `RAW_REFUSED`, `RAW_OMITTED`, `RAW_FORMAT`, `UNRESOLVED`, `EXPANSION`, `DEPTH_LIMIT` | the messages it reports |

Only raw content with format `text` is written, and only when the policy allows it.

## Redesigns from the Odin package

- **One exhaustive match.** `blocks.rs::render_node` routes every `NodeKind` (blocks and
  inlines; a `Context` says which, for images). Structure parts met on their own are
  reported as `core.unhandled_kind` by explicit arms, never by a wildcard.
- **Strings instead of measure-then-write.** Inline layouts and table cells are laid
  out once into `String`s taken from a small pool of reused buffers; the whole output is
  built in a `String` and emitted once. The Odin two-pass layout and scratch
  bookkeeping are gone.
- **Fill as an enum.** What a layout writes (`Fill::Inlines`, `Fill::Term`, ...) is a
  plain enum instead of a procedure pointer.
- Frames keep their marker strings and are reused; table cells own their laid-out text.
- Recursion is bounded by `Limits::max_depth` (`text.limit.depth`) and the stack guard
  (`core.stack`), as in Odin.
