# guidedoc-defaults

The built-in pieces Guidedog assembles around the engine.

## Use

```toml
[dependencies]
guidedoc-defaults = "1.0.0-beta.10"
guidedoc = "1.0.0-beta.10"
```

Every reader and renderer is in one registry, so one call converts:

```rust
fn main() -> Result<(), guidedoc::Fail> {
    let registry = guidedoc_defaults::registry();
    let mut ws = guidedoc::Workspace::new();
    let html = guidedoc::convert_text(&mut ws, &registry, "myst", "html", "# Hello\n")?;
    println!("{html}");
    Ok(())
}
```

Build the API documentation locally with `cargo doc -p guidedoc-defaults --no-deps` (and on [docs.rs](https://docs.rs/guidedoc-defaults) once published to crates.io). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
let registry = guidedoc_defaults::registry();
let mut ws = guidedoc::Workspace::new();
let html = guidedoc::convert_text(&mut ws, &registry, "myst", "html", "# Hello\n")?;

// Typed settings for one reader; another reader refuses them.
let mut settings = guidedoc_rst::default_settings();
settings.pep_references = true;
let mut request = guidedoc::default_request("page.rst", "See PEP 8.\n", "rst", "html");
request.read = guidedoc_defaults::rst_options(&settings);

// Highlighting for the renderers.
let highlighter = guidedoc_defaults::HtmlHighlighter::new();
let options = guidedoc::RenderOptions { highlighter: Some(&highlighter), ..Default::default() };
```

| Item | Does |
|---|---|
| `registry()` | readers `rst`, `sphinx`, `commonmark`, `myst`; renderers `html`, `typst`, `text`, `rst` |
| `rst_options`, `sphinx_options`, `myst_options`, `ReaderSettings` | `ReadOptions` carrying the reader's own settings by type (`ReaderSettings::{with_config, for_source, options}`) |
| `HtmlHighlighter`, `TypstHighlighter` | `guidedoc::Highlighter` over `gd-highlight`; one per thread, buffer reused |

## Design notes

- The registry is built per call from the crates' `const` descriptors; it is two short
  vectors.
- Typed options use `guidedoc::read_extension`, which records the settings' type name, so a
  mismatch is the diagnostic `core.read.settings_type`.
- Highlighters keep a reusable buffer in a `RefCell`, so a thread highlights every code
  block without allocating a new buffer; the engine renders once.
