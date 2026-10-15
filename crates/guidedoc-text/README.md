# guidedoc-text

The plain-text renderer of Guidedoc, as Sphinx's text builder writes text: paragraphs
wrapped at 70 columns, underlined titles, hanging list markers, three-space indents,
grid tables with spans, and `Note:` admonitions.

## Use

```toml
[dependencies]
guidedoc-text = "0.7"
guidedoc = "0.7"
guidedoc-commonmark = "0.7"
```

We render CommonMark as plain text:

```rust
use guidedoc::{Registry, Workspace, convert_text};

fn main() -> Result<(), guidedoc::Fail> {
    let registry = Registry {
        readers: vec![guidedoc_commonmark::READER],
        renderers: vec![guidedoc_text::RENDERER],
    };
    let mut ws = Workspace::new();
    let out = convert_text(&mut ws, &registry, "commonmark", "text", "# Hi *there*\n")?;
    println!("{out}");
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/guidedoc-text). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

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

## Design notes

- **One exhaustive match.** `blocks.rs::render_node` routes every `NodeKind` (blocks and
  inlines; a `Context` says which, for images). Structure parts met on their own are
  reported as `core.unhandled_kind` by explicit arms, never by a wildcard.
- **One pass into strings.** Inline layouts and table cells are laid out once into
  `String`s taken from a small pool of reused buffers; the whole output is built in a
  `String` and emitted once, so no layout is measured before it is written.
- **Fill as an enum.** What a layout writes (`Fill::Inlines`, `Fill::Term`, ...) is a
  plain enum.
- Frames keep their marker strings and are reused; table cells own their laid-out text.
- Recursion is bounded by `Limits::max_depth` (`text.limit.depth`) and the stack guard
  (`core.stack`), so deep input fails with a diagnostic, never a stack overflow.
