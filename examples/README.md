# Examples

Small programs that embed Guidedog's libraries. Each one is a Cargo example; run it from
the repository root.

## The engine and its libraries (`guidedoc-examples`)

The crate [guidedoc-examples](guidedoc-examples) builds one `Registry` from the reader
and renderer crates (`guidedoc-rst`, `guidedoc-commonmark`, `guidedoc-html`,
`guidedoc-typst`, `guidedoc-text`) in `src/lib.rs`, and every example uses it.

| Example | Shows | Run |
|---|---|---|
| [hello](guidedoc-examples/examples/hello.rs) | Markdown to a standalone HTML page with one call, `convert_text` | `cargo run -p guidedoc-examples --example hello` |
| [reuse_workspace](guidedoc-examples/examples/reuse_workspace.rs) | many conversions with one `Workspace`, whose buffers are reused | `cargo run -p guidedoc-examples --example reuse_workspace` |
| [custom_pass](guidedoc-examples/examples/custom_pass.rs) | a `Pass` that rewrites the document between reading and rendering | `cargo run -p guidedoc-examples --example custom_pass` |
| [plain_text](guidedoc-examples/examples/plain_text.rs) | a registry of our own with the `text` renderer | `cargo run -p guidedoc-examples --example plain_text -- FILE.rst` |
| [graphdog](guidedoc-examples/examples/graphdog.rs) | DOT to SVG with Graphviz linked in, and an Elm-style error | `cargo run -p guidedoc-examples --example graphdog` |

`reuse_workspace` shows bounded memory without storage the caller sizes up front. The
`Workspace` owns its tables and clears them between conversions without giving back
their capacity, so once a document of a given size has been converted, converting
another of that size allocates nothing in the engine. The test
`steady_state_conversion_does_not_allocate` in `crates/guidedoc/tests/services.rs`
checks this with a counting global allocator; the example shows the same effect by
watching the output buffer stay in place. Limits stay explicit: `Limits` bounds nodes,
text, output, depth, and work, and exceeding one is a `Capacity` or `Limit` failure,
never an abort.

## Plugins and publishers

These live next to the libraries they demonstrate:

| Example | Shows | Run |
|---|---|---|
| [build_plugin](../crates/gd-plugins/examples/build_plugin) | the product-badge build plugin: source, HTML, and asset hooks, served over the plugin protocol | `cargo build -p gd-plugins --example build_plugin` |
| [publisher](../crates/gd-plugins/examples/publisher.rs) | a whole publisher that writes a built site to a local folder | `cargo build -p gd-plugins --example publisher` |
| [pages_publisher](../crates/gd-pages/examples/pages_publisher.rs) | the GitHub Pages publisher served through the same SDK | `cargo build -p gd-pages --example pages_publisher` |

Register a built plugin with `guidedog plugin register NAME target/debug/examples/NAME`,
or link it into our own `guidedog` with `gd_cli::run_with`. The
[plugin chapter](../docs/manual/plugins.rst) walks through both.
