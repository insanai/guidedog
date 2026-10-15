# gd-graphdog cheat sheet

Graphviz DOT to SVG, PNG or laid-out DOT with the Graphviz C library in process (bound by
hand in `src/ffi.rs`; links `-lgvc -lcgraph`, or a static Graphviz, see below).

## Use

```toml
[dependencies]
gd-graphdog = "0.7"
```

We render DOT to SVG. The Graphviz libraries `libgvc` and `libcgraph` must be installed (on Debian and Ubuntu, `sudo apt install libgraphviz-dev`):

```rust
fn main() -> Result<(), gd_graphdog::Error> {
    let ctx = gd_graphdog::Context::new()?;
    let source = "digraph { a -> b }";
    match ctx.render(source, gd_graphdog::Options::default()) {
        Ok(rendered) => println!("{} bytes of SVG", rendered.output.len()),
        Err(err) => eprint!("{}", gd_graphdog::format_error(&err, source, "g.dot")),
    }
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-graphdog). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

| Item | Does |
|---|---|
| `Context::new()`, `ctx.version()` | loads plugins once; freed on drop |
| `ctx.render(source, Options) -> Result<Rendered, Error>` | `Rendered { output, warnings }` |
| `Options { engine, format, attributes, files, reserve }` | -K, -T, -G/-N/-E, file confinement, output budget |
| `Engine::{ALL, name, from_name}`, `Format::{ALL, name, from_name}` | names as dot spells them |
| `Error { kind, line, column, near, message, hint }`, `.report(source, name)` | Elm-style via `gd-report`; codes `graphdog.*` |
| `format_error(&err, source, name)` | the report as plain text |

Every Graphviz call runs under one process-wide mutex; renders from many threads are
serialized.

## Static Graphviz

`cargo xtask graphviz` builds Graphviz 16.1.0 (with Expat) as static libraries into
`target/graphviz/16.1.0`. With `GUIDEDOG_GRAPHVIZ_STATIC=<that folder>` (or the
`static-graphviz` feature, which uses the default folder), `build.rs` links them instead
of the shared libraries, `STATIC` is `true`, and the program needs no Graphviz at run
time. The static build has the dot and neato layout plugins (all eight engines) and the
core renderers (SVG, DOT); PNG fails with `MissingFormat`. `src/builtin.rs` hands the
plugins to `gvContextPlugins`, like Graphviz's `dot_static`, plus a text-layout plugin
that sizes non-ASCII labels (CJK at one em) where Graphviz's ASCII-only tables would warn.

## Design notes

- `Context::new`/`Drop` replace `init`/`close`; `version` and `render` are methods.
- The static build is chosen by `build.rs` from an environment variable or a feature,
  not by a compile-time define; the plugin tables are Rust statics, so no C shim is
  compiled.
- Warnings come back in `Rendered::warnings` instead of an `Option<&mut Vec<String>>`
  out-parameter in `Options`.
- `Engine::name`/`from_name`/`ALL`, `Format::...` and `ErrorKind::title`/`code` replace
  the free functions `engine_name`, `parse_engine`, `ENGINES`, `error_title`, ...
- The lock is a type: helpers that call Graphviz take `&Locked`, and the RAII handles
  (`Graph<'l>`, `Layout` borrowing context and graph, the log `Session<'l>`, the
  prototype-defaults guard `Fixed<'l>`) borrow it, so none can outlive the lock or its
  owner. The defaults are restored by `Fixed`'s `Drop` instead of a `fix`/`restore` pair.
- The log's "first error" is an `Option` instead of `""`.
- Reports render through `gd-report` (`2| line` gutter, carets, terminal escaping);
  `format_message` is gone. A known line without a column underlines the whole line.
- `<IMG SRC>` search matches case-insensitively in place instead of lowercasing a copy
  of the label for every image, and attribute names are no longer copied per attribute.
