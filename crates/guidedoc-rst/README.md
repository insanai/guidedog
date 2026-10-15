# guidedoc-rst

**The reStructuredText reader of Guidedoc**: lines, block structure, explicit markup,
every standard Docutils directive, include, and the interfaces the Sphinx layer and the
MyST reader build on. Inline markup and resolution are `guidedoc-rst-inline`.

## Use

```toml
[dependencies]
guidedoc-rst = "0.7"
guidedoc = "0.7"
guidedoc-html = "0.7"
```

We convert reStructuredText to HTML:

```rust
use guidedoc::{Registry, Workspace, convert_text};

fn main() -> Result<(), guidedoc::Fail> {
    let registry = Registry {
        readers: vec![guidedoc_rst::reader()],
        renderers: vec![guidedoc_html::RENDERER],
    };
    let mut ws = Workspace::new();
    let source = "Title\n=====\n\nHello *world*.\n";
    let html = convert_text(&mut ws, &registry, "rst", "html", source)?;
    println!("{html}");
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/guidedoc-rst). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Overview

- Coverage ledger: `docs/manual/internals/rst-ledger.rst`
- Conformance with Docutils 0.22 (98 of 98 corpus sources match):
  `docs/manual/internals/rst-conformance.rst`, `tests/conformance.rs`

```rust
use guidedoc::{Workspace, default_request, read_document, read_extension};
use guidedoc_rst::{Settings, reader};

let settings = Settings { sectsubtitle: true, ..Settings::default() };
let mut request = default_request("intro.rst", "Hello *world*.\n", "rst", "html");
request.read.extension = Some(read_extension(&settings));
let mut ws = Workspace::new();
let doc = read_document(&mut ws, &reader(), &request)?;
```

## Cheat sheet

| Need | Call |
|---|---|
| register the reader | `guidedoc_rst::reader()` (`read`, `read_inline`) |
| settings | `Settings` (`Default` = Docutils' defaults) via `ReadOptions::extension` |
| a dialect (Sphinx) | `impl Extension`, then `read_with(ctx, &settings, Some(&ext))` / `read_inline_with` |
| dialect directive | `Extension::directive(name) -> Option<DirectiveHandler>` (rules + `tag`), `Extension::run_directive(p, d)` |
| dialect role | `Extension::role(ctx, &RoleCall) -> bool` |
| hooks | `begin`, `has_prologue` + `prologue` (after leading field lines), `end`, `substitution(name)` |
| directive parts | `d.arg`, `d.content`, `d.option(name)`, `d.option_value(name)`, `d.option_text`, `d.option_lines`, `d.has_option`, `d.handler()` |
| nested content | `p.parse_body(lines, sections, each)`, `p.inline(lines)`, `p.inline_node(kind, lines)`, `p.parse_blocks`, `p.parse_titled`, `p.place_titled`, `p.parse_source(id, sections)` |
| nodes | `p.open`/`p.close` (pending classes applied), `p.ctx.b`, `p.add_common`, `p.directive_node`, `p.custom` |
| text | `p.code_text`, `p.pool_lines`, `p.push_lines`, `p.pool_classes`, `p.pool_lower`, `join_lines`, `path_of` |
| spans | `p.span_of`, `p.line_span`, `p.lines_span`; sources: `p.enter_source`/`p.leave_source` |
| roles state | `p.find_role`, `p.inline_state()` (default role), `standard_role` |
| a host markup (MyST) | `impl Host` (`body`, `inline`, `items`); `embed_begin(ctx, &settings, ext) -> ReaderState`; per call `state.parser(ctx, ext, Some(&host))`; `p.run_hosted(&HostDirective)`, `p.directive_shape(name)`, `p.apply_host_role`, `p.host_lines`/`p.host_views`, `p.take_table_extra` + `p.table_decorate`, `p.take_pending_classes`; end with `p.embed_end()`; `p.lend(\|ctx, st\| ..)` hands a host the context and state for nested content |
| messages | `guidedoc_rst::messages::*` (every one has a hint) |

## Redesigns from the Odin reader

- **No scratch arena.** Lines are `Copy` views (`Line<'s>`) of the source; lines with
  tabs, form feeds, or vertical tabs are expanded once into a `String` the lines borrow
  (`split_lines`, `expand_views`). Derived line arrays are `Vec`s; there is no
  `take`/`mark`/`release`. The Odin guarantee of zero allocator calls per read is not
  kept: a read allocates small vectors per block (the workspace's tables are still
  reused).
- **Errors with `?`.** Every parsing function returns `Result<_, Fail>`; problems are
  reports, only builder failures (limits, capacity) are `Err`. The `alive(p)` checks and
  status returns are gone.
- **Parser = view over state.** `ReaderState` owns the per-document state (sections,
  adornment styles, pending classes, inline state); `Parser<'p, 'a>` borrows it with the
  `ReadContext`, the dialect's `&dyn Extension`, and the `&dyn Host`. A host keeps the
  state between its own calls and builds a parser view per call, so it never gives its
  context away (Odin stored the parser inside the host).
- **Traits instead of procedure + `rawptr` tables.** `Extension` (directive lookup,
  `run_directive`, `role`, hooks, `substitution`) and `Host` take `&self`; dialect state
  that changes lives in cells, because handlers run re-entrantly. A `DirectiveHandler`
  carries the rules and a `tag` instead of a procedure; the extension dispatches its own
  handlers in `run_directive`. Role handlers receive the `ReadContext` (the inline crate's
  `RoleHook`), not the parser.
- **Settings are plain data** (`String`s, `'static`), so they travel through
  `ReadOptions::extension`; the extension and host are passed to `read_with` /
  `embed_begin` instead of living inside `Settings`.
- **Directive** owns its parts (`Vec<Line>` argument, options, content) and has
  accessor methods (`option_value` replaces `opt_value(p, d, name)`).
- `TableExtra` stores the table title as source ranges, so the hosted table directive
  can hand it to the host's table across calls; the first table takes it
  (`take_table_extra`).
- The list-table walks its rows from one collected list (`bullet_items`) instead of a
  primed iterator; enumerator ordinals are `Option<i64>` instead of -1.
- East Asian widths use the Unicode 15.1 table Odin's `core:unicode` ships, and
  combining marks the blocks Odin's `is_combining` names.
- Deep-nesting tests run on a large-stack thread with a raised stack budget in debug
  builds, whose frames are larger; in release builds the default 512 KiB budget holds
  the default depth limit (`default_stack_budget_holds_the_default_depth`).
