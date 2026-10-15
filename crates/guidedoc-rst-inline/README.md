# guidedoc-rst-inline

**reStructuredText inline markup and reference resolution for Guidedoc.** The block
reader (`guidedoc-rst`) keeps one `InlineState` per read, calls `parse_inline` for every
inline context, and calls `resolve` once when the document is parsed. Sphinx
(`guidedoc-sphinx`) adds roles through a `RoleHook` and default substitutions through
`resolve_with_substitutions`.

## Use

```toml
[dependencies]
guidedoc-rst-inline = "0.7"
guidedoc = "0.7"
```

A reader parses the inline markup of a paragraph it has opened, then resolves references once per document:

```rust
use guidedoc::{LineView, NodeKind, SourceSpan, Workspace, default_request};
use guidedoc_rst_inline::{InlineState, NoRoleHook, Options, parse_inline, resolve};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = "See *this* and `that <https://example.org>`_.";
    let mut ws = Workspace::new();
    let request = default_request("x.rst", text, "rst", "html");
    let mut ctx = ws.begin_read(&request)?;
    let mut state = InlineState::new(Options::default());
    let span = SourceSpan::new(ctx.source, 0, text.len() as u32);
    let paragraph = ctx.b.begin_node(NodeKind::Paragraph, span)?;
    let lines = [LineView { text, offset: 0 }];
    parse_inline(&mut ctx, &mut state, &lines, &mut NoRoleHook)?;
    ctx.b.end_node(paragraph)?;
    resolve(&mut ctx, &state.options)?;
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/guidedoc-rst-inline). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

- Recognition follows the Docutils "Inline markup recognition rules" (contexts from the
  Unicode punctuation categories, quoted start-strings, backslash escapes), with
  standalone URIs and email addresses, implicit PEP/RFC references, and every standard
  and specialized role.
- Resolution: explicit, implicit, indirect, chained, and anonymous targets; embedded URIs
  and aliases; auto-numbered, labelled, and symbol footnotes; citations; substitutions
  (with the cycle and depth checks); then title and subtitle promotion, section
  subtitles, bibliographic fields (15 languages), smart quotes, and tab expansion.
- Coverage ledger: `docs/manual/internals/rst-inline-ledger.rst`.

## API (stable; changes are additive)

The surface the RST block reader (`guidedoc-rst`) and Sphinx (`guidedoc-sphinx`) code
against. It is stable: later changes are additive only. Odin names on the left.

| Odin `rst_inline.` | Rust `guidedoc_rst_inline::` |
|---|---|
| `Line` (`gd.Line_View`) | `Line<'s>` = `guidedoc::LineView<'s>` (`text`, `offset`); pass source text with tabs **unexpanded** (Odin `inline_view`) |
| `Inline_State{ctx, source, roles, roles_used, default_role, hook, options}` | `InlineState { source, roles: Vec<Role>, default_role: String, options: Options }` (+ private reusable buffers). No `ctx` and no `hook` inside: both are call arguments. `InlineState::new(options)`, `find_role(name) -> Option<usize>`, `define_role(role) -> usize` (replaces same name) |
| `Role`, `Role_Base` | `Role { name, base, classes, format, language }` (owned `String`s, `Default`), `RoleBase::{Emphasis, Strong, Literal, Code, Math, Subscript, Superscript, TitleReference, PepReference, RfcReference, Abbreviation, Acronym, Raw, Custom}` |
| `standard_role(name)` (private in inline; the block reader had its own) | `standard_role(name) -> Option<RoleBase>` (case-insensitive, aliases included) |
| `Options`, `Smart_Quotes` | `Options` (`Default`: `docinfo = true`, rest off/empty; `tab_width: usize`, 0 = 8), `SmartQuotes::{Off, On, Alt}` |
| `Options.default_substitution` (`Substitution_Hook`) | not in `Options`; pass a closure to `resolve_with_substitutions` |
| `parse_inline(&state, lines) -> Status` | `parse_inline(ctx: &mut ReadContext, state: &mut InlineState, lines: &[Line], hook: &mut dyn RoleHook) -> Result<(), Fail>` |
| `resolve(ctx, options) -> Status` | `resolve(ctx, &options) -> Result<(), Fail>`; `resolve_with_substitutions(ctx, &options, &\|name\| -> Option<String>)` |
| `apply_host_role(&state, name, lines, q, j)` | `apply_host_role(ctx, &mut state, hook, name, lines, q..j)` |
| `Role_Hook{procedure, user}` | `trait RoleHook { fn role(&mut self, ctx: &mut ReadContext, call: &RoleCall) -> bool }`; closures `FnMut(&mut ReadContext, &RoleCall) -> bool` implement it; `NoRoleHook` knows nothing |
| `Role_Call{state, b, name, marked, text, span, content}` | `RoleCall<'c> { name, marked, text, span, content }` (plain data, constructible by callers, e.g. Sphinx's `field_role`); build nodes with `ctx.b` |
| `unescape(&buf, marked)` | `unescape(&mut String, marked)` (appends) |
| `python_space(r)` | `python_space(c: char) -> bool` |
| `footnote_symbol(n, &buf)` | `footnote_symbol(n: i32, &mut String)` (appends) |
| `TARGET_NAME_ATTRIBUTE` | `TARGET_NAME_ATTRIBUTE` ("target-name"); also `SECTION_SUBTITLE_CLASS`, `DOCINFO_CLASS`, `FOOTNOTE_SYMBOLS` |
| messages | `MESSAGES: &[Message]` (codes `rst.inline.*`, `rst.role.*`, `rst.reference.*`, `rst.target.*`, `rst.footnote.*`, `rst.substitution.*`, `rst.resolve.*`, `rst.docinfo.*`) |

### Typical use from the block reader

```rust
use guidedoc_rst_inline as inline;

// setup: one state per read
let mut inl = inline::InlineState::new(inline_options(&settings)); // tab_width = p.tab
inl.source = ctx.source;

// every inline context (Odin `inline(p, lines)`), inside an open node:
inl.source = current_source;
inline::parse_inline(ctx, &mut inl, &views, &mut hook)?;   // hook: &mut dyn RoleHook

// role directive: inl.define_role(role); default-role: inl.default_role = name.to_lowercase();

// after parsing (Odin `rst_inline.resolve(ctx, p.inl.options)`):
inline::resolve(ctx, &inl.options)?;               // or resolve_with_substitutions(...)
```

The hook gets `&mut ReadContext` rather than the block parser: a hook that needs parser
state (Sphinx domains, extension roles) is a separate value holding what it needs, not
the parser that also holds the `InlineState`. Errors are reported to `ctx.reports()`;
`Err` means the builder failed (limit, capacity) and the read must stop.

## Redesigns from the Odin package

- **No context inside the state.** Odin's `Inline_State` held `ctx` and the role hook;
  here both are call arguments (`parse_inline(ctx, state, lines, hook)`), so the block
  reader can own the state and the hook separately without aliasing.
- **Hooks are traits and closures.** `Role_Hook{procedure, user}` is `trait RoleHook`
  (closures implement it); `Substitution_Hook` is a `&dyn Fn(&str) -> Option<String>`
  given to `resolve_with_substitutions` instead of an `Options` field, so `Options` stays
  plain `Clone + Eq` data.
- **`RoleCall` is plain data.** It carries name, marked text, cooked text, and spans; the
  builder comes from the `ctx` the hook receives.
- **Roles are a `Vec<Role>`** with owned strings; `define_role` replaces a role of the
  same name. The fixed role table and its capacity error belong to the block reader now.
- **`Result` everywhere.** Every builder call returns `Result<_, Fail>`; problems in the
  text are reports, and `Err` means the builder failed (limit or capacity).
- **Reused buffers instead of scratch.** The joined text, line starts, and marks live in
  the `InlineState` and are cleared, not reallocated, between inline contexts.
- **Resolution uses `Vec` and `HashMap`.** The counting pass that sized scratch tables is
  gone, and so is the probe limit (`rst.resolve.limit` is no longer reported). The name
  table's `AMBIGUOUS` sentinels are an enum (`Name::{Record, Ambiguous,
  AmbiguousExplicit}`); `Option` replaces `-1`/`INVALID_NODE` results.
- **Tree edits** (`rotate`, `remove_range`, `move_out`, ...) use slice `rotate_left` and
  `drain` on the snapshot's `Vec`s.
- **Tests** build documents with a closure fixture (`fixture(source, promote, |f| ...)`)
  over `Workspace::begin_read`. The Odin tests for a full scratch region
  (`scratch_capacity_is_a_result`, `resolution_capacity_is_a_result`) and the "no
  allocator calls" test have no Rust counterpart; `text_limit_is_a_result` and
  `every_construct_in_one_document` take their place.

