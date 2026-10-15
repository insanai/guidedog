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
against. It is stable: later changes are additive only.

| Item | `guidedoc_rst_inline::` |
|---|---|
| lines | `Line<'s>` = `guidedoc::LineView<'s>` (`text`, `offset`); pass source text with tabs **unexpanded** |
| state | `InlineState { source, roles: Vec<Role>, default_role: String, options: Options }` (+ private reusable buffers). The read context and the role hook are call arguments, not fields. `InlineState::new(options)`, `find_role(name) -> Option<usize>`, `define_role(role) -> usize` (replaces same name) |
| roles | `Role { name, base, classes, format, language }` (owned `String`s, `Default`), `RoleBase::{Emphasis, Strong, Literal, Code, Math, Subscript, Superscript, TitleReference, PepReference, RfcReference, Abbreviation, Acronym, Raw, Custom}` |
| standard roles | `standard_role(name) -> Option<RoleBase>` (case-insensitive, aliases included) |
| options | `Options` (`Default`: `docinfo = true`, rest off/empty; `tab_width: usize`, 0 = 8), `SmartQuotes::{Off, On, Alt}` |
| default substitutions | not in `Options`; pass a closure to `resolve_with_substitutions` |
| parsing | `parse_inline(ctx: &mut ReadContext, state: &mut InlineState, lines: &[Line], hook: &mut dyn RoleHook) -> Result<(), Fail>` |
| resolution | `resolve(ctx, &options) -> Result<(), Fail>`; `resolve_with_substitutions(ctx, &options, &\|name\| -> Option<String>)` |
| host roles | `apply_host_role(ctx, &mut state, hook, name, lines, q..j)` |
| role hook | `trait RoleHook { fn role(&mut self, ctx: &mut ReadContext, call: &RoleCall) -> bool }`; closures `FnMut(&mut ReadContext, &RoleCall) -> bool` implement it; `NoRoleHook` knows nothing |
| role call | `RoleCall<'c> { name, marked, text, span, content }` (plain data, constructible by callers, e.g. Sphinx's `field_role`); build nodes with `ctx.b` |
| unescaping | `unescape(&mut String, marked)` (appends) |
| white space | `python_space(c: char) -> bool` |
| footnote symbols | `footnote_symbol(n: i32, &mut String)` (appends) |
| constants | `TARGET_NAME_ATTRIBUTE` ("target-name"), `SECTION_SUBTITLE_CLASS`, `DOCINFO_CLASS`, `FOOTNOTE_SYMBOLS` |
| messages | `MESSAGES: &[Message]` (codes `rst.inline.*`, `rst.role.*`, `rst.reference.*`, `rst.target.*`, `rst.footnote.*`, `rst.substitution.*`, `rst.resolve.*`, `rst.docinfo.*`) |

### Typical use from the block reader

```rust
use guidedoc_rst_inline as inline;

// setup: one state per read
let mut inl = inline::InlineState::new(inline_options(&settings)); // tab_width = p.tab
inl.source = ctx.source;

// every inline context, inside an open node:
inl.source = current_source;
inline::parse_inline(ctx, &mut inl, &views, &mut hook)?;   // hook: &mut dyn RoleHook

// role directive: inl.define_role(role); default-role: inl.default_role = name.to_lowercase();

// after parsing:
inline::resolve(ctx, &inl.options)?;               // or resolve_with_substitutions(...)
```

The hook gets `&mut ReadContext` rather than the block parser: a hook that needs parser
state (Sphinx domains, extension roles) is a separate value holding what it needs, not
the parser that also holds the `InlineState`. Errors are reported to `ctx.reports()`;
`Err` means the builder failed (limit, capacity) and the read must stop.

## Design notes

- **No context inside the state.** The read context and the role hook are call
  arguments (`parse_inline(ctx, state, lines, hook)`), so the block reader can own the
  state and the hook separately without aliasing.
- **Hooks are traits and closures.** Role hooks implement `trait RoleHook` (closures
  do); a default substitution is a `&dyn Fn(&str) -> Option<String>` given to
  `resolve_with_substitutions` rather than an `Options` field, so `Options` stays plain
  `Clone + Eq` data.
- **`RoleCall` is plain data.** It carries name, marked text, cooked text, and spans; the
  builder comes from the `ctx` the hook receives.
- **Roles are a `Vec<Role>`** with owned strings; `define_role` replaces a role of the
  same name. The fixed role table and its capacity error belong to the block reader.
- **`Result` everywhere.** Every builder call returns `Result<_, Fail>`; problems in the
  text are reports, and `Err` means the builder failed (limit or capacity).
- **Reused buffers.** The joined text, line starts, and marks live in
  the `InlineState` and are cleared, not reallocated, between inline contexts.
- **Resolution uses `Vec` and `HashMap`**, which grow as needed, so no counting pass sizes
  tables first and no probe limit applies (`rst.resolve.limit` is not reported). A
  name's state is an enum (`Name::{Record, Ambiguous, AmbiguousExplicit}`); absent
  results are `Option`s.
- **Tree edits** (`rotate`, `remove_range`, `move_out`, ...) use slice `rotate_left` and
  `drain` on the snapshot's `Vec`s.
- **Tests** build documents with a closure fixture (`fixture(source, promote, |f| ...)`)
  over `Workspace::begin_read`; `text_limit_is_a_result` and
  `every_construct_in_one_document` check limits and broad coverage.

