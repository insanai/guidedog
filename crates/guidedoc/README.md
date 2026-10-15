# guidedoc

**The conversion engine of Guidedog: reStructuredText and Markdown in, HTML, Typst, and
text out, through one validated document model.** The engine owns the AST, the builder,
validation, diagnostics, the adapter registry, and the pipeline; readers and renderers
live in their own crates (`guidedoc-rst`, `guidedoc-html`, ...) and `guidedoc-defaults`
registers them.

- Node shapes: [docs/manual/internals/ast.rst](../../docs/manual/internals/ast.rst) (normative)
- Every Odin `gd.` name and its Rust form: [docs/manual/internals/engine-api-map.rst](../../docs/manual/internals/engine-api-map.rst)
- Design specification: `../guidedog_old/docs/gds/records/0003-guidedoc-odin.rst`

```rust
use guidedoc::{convert_text, Workspace};

fn main() {
    let registry = guidedoc_defaults::registry();       // the built-in readers and renderers
    let mut ws = Workspace::new();                      // reuse it: buffers are kept
    match convert_text(&mut ws, &registry, "commonmark", "html", "# Hello\n") {
        Ok(html) => println!("{html}"),                 // borrows ws until the next convert
        Err(fail) => eprintln!("{fail}"),               // title, message, and hint
    }
}
```

## Converting

| Call | Does |
|---|---|
| `convert_text(&mut ws, &registry, reader, renderer, text)` | one call, default request, output borrowed from `ws` |
| `convert(&mut ws, &registry, &request)` | the full request: options, policy, limits, passes |
| `read_document(&mut ws, &reader, &request)` | parse and validate; returns the `Document` |
| `transform_document(&mut ws, &passes, &limits)` | run passes on the current document |
| `check_render(&mut ws, &renderer, &request)` | exact output size and support problems, no output |
| `render_document(&mut ws, &renderer, &request)` | write the output |
| `ws.document()`, `ws.reports()`, `ws.source_map()` | the current document, diagnostics, source map |

Build requests with struct-update syntax from the defaults:

```rust
use guidedoc::default_request;

let text = "Title\n=====\n";
let mut request = default_request("intro.rst", text, "rst", "html");
request.policy.keep_documents_with_errors = true;   // Docutils-like: report and carry on
request.limits.max_nodes = 100_000;
```

Every failure is a `Fail { status, need, diagnostic }`: `status` is the class
(`InvalidInput`, `Unsupported`, `Policy`, `Capacity`, `Limit`, `Cancelled`, `Internal`),
`need` names the region a `Capacity` failure lacked (the exact size for output), and the
diagnostic has a stable `code`, a title, a message with its arguments, and a hint.
`ws.reports()` holds every diagnostic of the conversion, bounded by `Limits::max_reports`.

## Writing adapters

A reader is a constant `Reader { id, name, version, conformance, read, read_inline }`
whose `read: fn(&mut ReadContext) -> Result<(), Fail>` builds nodes with `ctx.b`:

```rust
use guidedoc::{Fail, NodeKind, ReadContext, SourceSpan};

fn read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let text = ctx.source_of(ctx.source);                     // keep it while building
    let span = SourceSpan::new(ctx.source, 0, text.len() as u32);
    ctx.b.with_node(NodeKind::Paragraph, span, |b, _| {
        b.add_text_leaf(NodeKind::Text, span, &text).map(|_| ())
    })?;
    Ok(())
}
```

A renderer's `render: fn(&mut RenderContext) -> Result<(), Fail>` walks `ctx.doc`
(`doc.children(id)`, `doc.node_text(id)`, `doc.link_info(id)`, ...) and writes with
`ctx.out.emit(...)`. A pass is a closure `Fn(Document, &mut Builder) -> PassOutcome`
that copies what it keeps (`b.copy_subtree(doc, id)`) and returns `Rebuilt`, or returns
`Unchanged` at no cost.

## Redesigns from the Odin engine

The Odin core is the specification of behaviour; these are the deliberate differences.

- **Memory.** No slabs, plans, banks to size, or scratch region. The `Workspace` owns its
  tables as `Vec`s and `String`s and clears them between conversions, keeping their
  capacity, so a conversion in steady state allocates nothing (a test with a counting
  allocator checks it). Bounds stay as `Limits` (`max_nodes`, `max_text_bytes`,
  `max_output_bytes`, `max_reports`, depth, work, stack, includes), and input-driven
  growth uses `try_reserve`, so running out is a `Capacity` result, not an abort.
- **Documents are borrows.** Generation and revision checks are gone: a `Document<'w>`
  borrows the workspace, so a stale or foreign document does not compile. Stages work
  on the workspace's current document instead of taking a handle.
- **Results.** Every fallible call returns `Result<_, Fail>`; the builder still poisons
  itself on the first failure, and every later call returns that same failure, so `?`
  propagates it and an ignored failure still cannot publish a truncated tree.
- **Option, not sentinels**, at the API: `first_child`, `code_caption`, `parent_of`,
  `Builder::current`, `link_info`, `attribute`, `anchor_of`, `include_source`. The tables
  keep `NONE`/`INVALID_NODE` inside (`Node::payload`, `LinkInfo::resolved`) so the AST
  stays 1:1 with docs/manual/internals/ast.rst.
- **Iterators** replace `children`/`next_child`, `attributes`/`next_attribute`, and
  `anchor_ids`/`next_anchor_id`.
- **Single-pass rendering.** Renderers run once into an emitter with a byte limit that
  keeps counting past it, so an oversized output still reports its exact size; the
  measure-then-write double run is gone. `check_render` runs in check mode.
- **Text pool as bytes, output as bytes.** Readers may push bytes one at a time, as the
  Odin readers do; validation checks that the pool is UTF-8 and that every `Text` lies on
  character boundaries, and the engine checks the output is UTF-8 before returning it.
- **Traits for caller code**: `ResourceProvider`, `Highlighter`, and closures for passes
  and cancellation replace procedure-plus-`rawptr` pairs. Reader settings are a typed
  `&dyn Any` (`read_extension`, `ctx.reader_settings::<T>()`).
- **Sources.** The main source is copied into a reused workspace buffer; included and
  added sources are `Arc<str>` (a host can share its file cache). `check_utf8` is the
  host's UTF-8 check, since a `&str` is UTF-8 by type.
- **Symbol tables** use `HashMap` (collision-resistant hashing) with the same capacity
  bound; `insert` returns `Result<(), SymbolClash>`.
- **Objects** keep the field-by-field little-endian format, now version 2 (adds a
  source's usage); Odin caches are rebuilt.
- **Not ported**, because Rust makes them unnecessary: `Scratch`, `Stack(T)`, `Buffer`,
  `merge_sort` (std's stable sort), the region-overlap check, `init_workspace*`,
  `workspace_size`, `slab_plan`, `Second_Bank`, `document_valid`, `view`.
