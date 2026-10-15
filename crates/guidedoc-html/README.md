# guidedoc-html

The HTML renderer of Guidedoc: semantic HTML in Docutils' and Sphinx's element
structure, or the CommonMark reference renderer's bytes for conformance tests.

```rust
let registry = guidedoc::Registry {
    readers: vec![/* a reader */],
    renderers: vec![guidedoc_html::RENDERER],
};
let options = guidedoc::RenderOptions { standalone: false, permalinks: true, ..Default::default() };
let reference = guidedoc::RenderOptions { reference_profile: true, standalone: false, ..Default::default() };
```

| Item | Does |
|---|---|
| `RENDERER` / `renderer()` | the `"html"` descriptor for a `Registry` |
| `render(&mut RenderContext)` | the render function, for tests driving a context directly |
| `RAW_REFUSED`, `RAW_OMITTED`, `RAW_FORMAT`, `URL_UNSAFE`, `URL_OMITTED`, `UNRESOLVED`, `EXPANSION` | the messages it reports |

Options it reads (`guidedoc::RenderOptions`): `standalone`, `reference_profile`, `title`,
`language`, `stylesheet`, `section_numbers`, `anchor_prefix`, `permalinks`,
`permalink_icon`, `secnumber_suffix`, `highlighter`, `labels`, `math`, `math_inline`,
`math_display`. Policy: `raw` and `unsafe_urls` (refuse / omit / allow),
`keep_documents_with_errors` (problematic text instead of errors).

## Redesigns from the Odin package

- **One exhaustive match.** `walk.rs::enter` routes every `NodeKind` to the code that
  writes it; there is no `_ =>` arm and no "unhandled kind" path. `enter` returns a
  `Visit`: skip the children, or descend and close with a `Close` value that the walk
  keeps on its stack, so `leave` matches on `Close`, not on the kind again.
- **Explicit stack, shared.** Nested walks (substitutions, captions, hlist columns)
  share one `Vec<(NodeId, Close)>` with a base index instead of taking scratch per walk.
- **Writer split from state.** `Writer` owns output, the last byte, and reporting; the
  anchor table and the other state are separate fields, so ids borrowed from the table
  are written without copies.
- **Notes** keep per-note reference lists (`Vec<Vec<NodeId>>`) instead of linked
  head/next arrays.
- **No measure pass and no scratch.** The emitter counts past its limit; the renderer
  allocates its small working state (anchors, stack) per render, so the Odin
  zero-allocation tests are not ported.
- Highlighting goes through the `guidedoc::Highlighter` trait.
