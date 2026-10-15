# guidedoc-defaults

The built-in pieces Guidedog assembles around the engine.

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
| `registry()` | readers `rst`, `sphinx`, `commonmark`, `myst`; renderers `html`, `typst`, `text` |
| `rst_options`, `sphinx_options`, `myst_options` | `ReadOptions` carrying the reader's own settings by type |
| `HtmlHighlighter`, `TypstHighlighter` | `guidedoc::Highlighter` over `gd-highlight`; one per thread, buffer reused |

## Design notes (from Odin `lib/defaults` and `lib/highlight/guidedoc`)

- The registry is built per call from the crates' `const` descriptors instead of
  static tables; it is two short vectors.
- Typed options use `guidedoc::read_extension`, which records the settings' type name, so a
  mismatch is `core.read.settings_type` exactly as in Odin.
- Highlighters keep a reusable buffer in a `RefCell` instead of writing straight into the
  emitter in two passes; the engine now renders once.
