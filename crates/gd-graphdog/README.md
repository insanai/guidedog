# gd-graphdog cheat sheet

Graphviz DOT to SVG, PNG or laid-out DOT with the Graphviz C library in process (bound by
hand in `src/ffi.rs`; links `-lgvc -lcgraph`).

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

## Design notes

- `Context::new`/`Drop` replace `init`/`close`; `version` and `render` are methods.
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
