# guidedoc-commonmark

**Markdown for Guidedoc: pure CommonMark 0.31.2 (`READER`, id `commonmark`, 652/652 spec
examples) and the MyST Markdown Sphinx projects write (`MYST_READER`, id `myst`,
myst-parser 0.16.1).** Node shapes: `crates/guidedoc/AST.md`; coverage and conformance:
`docs/manual/internals/commonmark-ledger.rst` and `commonmark-conformance.rst`.

```rust
use guidedoc::{Registry, Workspace, convert, default_request, read_extension};
use guidedoc_commonmark::{MYST_READER, READER, Settings, SyntaxExtension};

let registry = Registry { readers: vec![READER, MYST_READER], renderers: vec![/* html */] };
let mut settings = Settings::default();                   // myst-parser's defaults
settings.enable_extensions.insert(SyntaxExtension::Deflist);
settings.heading_anchors = 2;
let mut request = default_request("intro.md", "# Hi\n", "myst", "html");
request.read.extension = Some(read_extension(&settings)); // the "commonmark" reader refuses settings
let mut ws = Workspace::new();
let _ = convert(&mut ws, &registry, &request);
```

## Extending (the Sphinx layer)

Implement `Extension` and put it in `Settings::extension` (`Arc<dyn Extension>`):

| Method | Called |
|---|---|
| `shape(h, name) -> Option<DirectiveShape>` | before a directive's options are read; `None`: unknown |
| `directive(h, &Directive) -> Result<DirectiveResult, Fail>` | for every `{name}` fence and math block |
| `role(h, &Role) -> Result<bool, Fail>` | for every `{name}`content``; `false`: unknown |
| `table(h, node, span, columns)` | after a GFM Table opens |
| `begin(h)`, `end(h)` | around the document (begin also before inline reads) |
| `resolve(h) -> Result<bool, Fail>` | instead of the reader's resolution when it returns true |

Handlers use the `Host`: `parse_body(lines, source, sections, classes)`,
`parse_inline(lines, source)`, `builder()`, `context()`, `list_items(lines, source)`,
`include_paths(images, docs)`, `set_classes_for_next(text)`, `name_text(s)`,
`set_extension_state(T)` / `extension_state::<T>()`.

## Redesigns from the Odin package

- **No scratch slabs.** The skeleton (blocks, arena, line segments) is a `Skeleton`
  owned by one parse and only read during emission, so directive lines borrow it while
  handlers get `&mut Host`. Inline items keep their text in the inline parser's own
  buffer; only emitted text reaches the document's pool (no garbage from discarded
  delimiter runs or from slug and image probes).
- **`Result` everywhere**: every builder-touching function returns `Result<_, Fail>`
  and propagates with `?`, instead of checking `b.status` after each call.
- **Extension as a trait** with default methods, replacing tables of procedures plus a
  `user` pointer; per-read state lives in the host (`set_extension_state`). A handler's
  "classes for the next element" output became `Host::set_classes_for_next`.
- **Owned settings** (`Vec<String>`, `Option<Vec<String>>` for url_schemes) with
  `Default` = myst-parser's defaults; `Settings::default()` replaces
  `default_settings()`. Readers are `const`s (`READER`, `MYST_READER`).
- **Entities**: the full WHATWG HTML5 table (2125 names, generated from Python's
  `html.entities.html5`), replacing Odin's XML table minus a deny-list.
- **Reference definitions** in a `HashMap<String, Ref>` of decoded strings; footnote
  lines and slugs are owned copies, so nothing outlives the source it came from.
- **YAML** nodes own their text; nesting is bounded (64) so hostile flow collections
  cannot overflow the stack. Tree walks of inline items are iterative.
- **Not ported**: the allocator-refusal tests (Rust readers allocate per read by
  design) and the `REFERENCES` "label table full" message (the map is unbounded,
  growing with the input).
