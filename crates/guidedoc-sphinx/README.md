# guidedoc-sphinx

**The Sphinx layer of Guidedoc**: Sphinx's directives, roles, and domains (py, c, cpp,
js, rst, std, math, odin, rust) on top of the reStructuredText reader
(`guidedoc-rst`'s `Extension`), and the same handlers for MyST Markdown (the
`guidedoc-commonmark` "myst" reader's `Extension`). Cross-document work (toctrees,
cross-references, numbering) is left to the project host.

## Use

```toml
[dependencies]
guidedoc-sphinx = "0.7"
guidedoc = "0.7"
```

We read a page with a Sphinx directive and print its tree:

```rust
use guidedoc::{Workspace, default_request, dump_to_string, read_document, read_extension};
use guidedoc_sphinx::{Config, reader};

fn main() -> Result<(), guidedoc::Fail> {
    let config = Config { primary_domain: "py".into(), ..Config::default() };
    let mut request = default_request("api.rst", ".. py:function:: f(x)\n", "sphinx", "html");
    request.read.extension = Some(read_extension(&config));
    let mut ws = Workspace::new();
    let doc = read_document(&mut ws, &reader(), &request)?;
    print!("{}", dump_to_string(doc));
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/guidedoc-sphinx). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Overview

- Coverage ledger: `docs/manual/internals/sphinx-ledger.rst`
- MyST conformance with the Sphinx handlers (`tests/myst_conformance/`): fixtures 215 of
  218 match myst-parser 0.16.1 (3 documented differences); Sphinx builds 8 of 10 (2
  documented)

```rust
use guidedoc::{Workspace, default_request, read_document, read_extension};
use guidedoc_sphinx::{Config, reader};

let config = Config { docname: "api".into(), ..Config::default() };
let mut request = default_request("api.rst", ".. py:function:: f(x)\n", "sphinx", "html");
request.read.extension = Some(read_extension(&config));
let mut ws = Workspace::new();
let doc = read_document(&mut ws, &reader(), &request)?;
```

## Cheat sheet

| Need | Call |
|---|---|
| register the reader | `guidedoc_sphinx::READER` / `reader()` (id `sphinx`) |
| configuration | `Config` (`Default` = Sphinx's defaults) via `ReadOptions::extension` |
| MyST with Sphinx | `myst_settings(config, myst)` -> `guidedoc_commonmark::Settings` for `MYST_READER` (id `myst`) |
| autodoc | `impl AutodocProvider` (`generate(&AutodocRequest) -> AutodocResult`), `Config::autodoc: Vec<Autodoc { domain, provider }>` |
| supported extensions | `EXTENSIONS`, `extension_supported(name)` |

## Domain API for domain authors

A domain is a value implementing `domains::Domain` (crate-private), registered in
`domains::DOMAINS`. The odin and rust domains live in `src/domains/odin.rs` and
`src/domains/rust.rs`; each exposes `pub(crate) static DOMAIN` (the `Domain` value) and
a per-document state struct (`OdinState`, `RustState`, `Default`) that `State` holds as
`st.odin` / `st.rust`. Their tests are integration tests, `tests/odin_domain.rs` and
`tests/rust_domain.rs` (the crate's unit tests use the helpers in `src/tests.rs`:
`read_text`, `expect_dump`, `expect_contains`, `expect_codes`, `md_expect`). This surface is stable: changes are additive only.

**Tables.** `fn directives() -> &'static [DomainDirective]` with full names
(`"odin:procedure"`):

- `DomainDirective::object(name, options)`: an object description, run by the shared
  machinery (`domains::object`): Desc{domain, objtype} > Desc_Signature per signature
  line (backslash continuations joined), Index{entries, targets}, Desc_Content.
- `DomainDirective::run(name, tag, (ArgRule, final_ws, ContentRule), options)`: our
  own directive, run by `Domain::run(sx, p, d, tag)`.
- `DomainDirective::autodoc(name, options)`: a generating directive; the host's
  provider for our domain (`Config::autodoc_for(name())`) writes reST that is parsed in
  place; `Domain::autodoc_context(st) -> (module, class)` fills the request.
- Options: `guidedoc_rst::{opt, choice, OptType}`; `object::OPTS_OBJECT` is the common
  set; `object::join_opts(a, b)` joins tables at compile time.
- `fn roles() -> &'static [RoleSpec]`: `RoleSpec::xref(name, XrefFlags::CODE.with(
  XrefFlags::DOTTED))` etc. (flags `CODE PARENS LOWER DOTTED AT SCOPED INDEX`); reftype
  is `"<domain>:<name>"`.
- `fn fields(objtype) -> &'static [DocField]`: info field types
  (`DocField { name, shape: FieldShape::{Plain, Grouped, Typed}, label, names, has_arg,
  role, typenames, typerole, bodyrole, collapse, python, ..DocField::DEFAULT }`).

**Hooks** (all have defaults):

| Hook | Called | The odin domain's use |
|---|---|---|
| `signature(cx, o, sig, span)` | per signature of an object directive | reads one declaration line |
| `content_begin(cx, o)` | inside Desc_Content, before the content | the `:availability:` paragraph |
| `before_content(cx, o) -> u32` | when a signature gave a name; returns what to pop | pushes the type, its polymorphic names and a `:package:` |
| `after_content(st, o, pushed)` | after every object's content | pops what `before_content` pushed |
| `toc_name(config, st, o, TocNames{fullname, module, sig}) -> Option<String>` | from `cx.toc_entry_name` | the name as `toc_object_entries_show_parents` asks |
| `map_target(st, target) -> Option<String>` | a role of the domain rewrites its target | follows an import alias |
| `xref_context(b, st, id, &XrefInfo{reftype, target, any})` | an Xref of the domain or `any` | records `odin:package` and the enclosing type |
| `run(sx, p, d, tag)` | `DomainDirective::run` directives | `odin:package`, ... |
| `autodoc_context(st)` | autodoc request | the current package and top type |

**Contexts.** `Cx { p: &mut Parser, st: &mut State, config: &Config }` holds the state
borrowed: inside a `Cx` hook never parse nested content (`p.parse_body`, `p.inline`),
because roles borrow the state too. `run` gets `sx: &Sphinx` instead: borrow with
`sx.state()` / `sx.with_cx(p, |cx| ..)` only between nested parses.

**Helpers.** `Cx`: `b()`, `report(msg, span)`, `leaf_in(kind, span, text)`,
`text_leaf(span, text)`, `custom(id, name, value)`, `add_ids(id, prefix, name, alias)
-> made id` (Sphinx make_id, unique; sets `st.object_id`, the entries' target),
`plain_signature(sig, span)`, `toc_entry_name(o, id, TocNames)` (call before the
signature's first child), `xref(span, reftype, domain, &XrefParts{title, target,
explicit, specific}, code)`, `node_count()` + `generated(first)`; `cx.config.label(
Label::Availability)`. `Object`: `d`, `domain`, `objtype`, `noindex`, `noentry`,
`fullname`, `prefix`, `signatures` (increment when a signature gives a name), `extra`
(free text for our before_content, e.g. polymorphic names), `entry(kind, &[parts])` /
`entry_begin` + `entry_add`, `flag(name)`, `option(name)`. `State`: `NameStack`
(`push -> bool` at most 32 deep, `pop`, `top`, `items`), `make_object_id`, `id_taken`,
`note_id`, `index_id`. Free functions: `domains::{leaf_in, text_leaf, generated,
emit_xref}`, `python::{module_target, pseudo_arglist, dotted_toc_name,
annotation_xrefs}`, `text::{trim_space, has_at, last_part, is_identifier_byte, ...}`.
Messages: `messages::SIGNATURE`, `NESTING`, ... (we add our own `Message` consts in our
module; every message has a hint).

## Design notes

- **One `Domain` trait.** Each domain is a value with its tables and hooks; the shared
  object description, roles, info fields, and toc names call the hooks, so no code
  switches on the domain. The odin and rust domains keep their own state (`OdinState`,
  `RustState`) beside the shared `State`.
- **Owned state.** `State` owns `String`s and `Vec`s
  (`NameStack` is a bounded `Vec<String>`); index entries and ids are built in
  `String`s and copied into the pool once.
- **State in a `RefCell` behind `&self`**, as `guidedoc-rst`'s `Extension` takes
  `&self` and handlers run re-entrantly. Roles receive the `ReadContext`, not the
  parser, so role code is written against `RoleCx { ctx, st, config }`.
- **Directives dispatch on tags** (`(group << 16) | index`), so a handler is
  found by a number; core directives are an enum (`Core`).
- **Autodoc providers are a trait** (`AutodocProvider`) per domain in
  `Config::autodoc`; messages are static `guidedoc::Message`s with arguments (Elm
  style), not strings built at run time.
- **Config is owned data** (`String`, `Vec`, `Labels`), `Default` = Sphinx's defaults.
- **MyST without aliasing.** The Markdown host and the embedded parser never share one
  read context at the same time: a directive detaches the Markdown reader's state from the context
  (`guidedoc_commonmark::Host::detach`), lends the context to the embedded
  reStructuredText parser, and nested Markdown attaches it again
  (`Detached::attach`) after moving the parser's state where nested directives find it
  (`guidedoc_rst::Parser::lend`). Both are small additive APIs of those crates.
- **The C++ parser is its own crate** (`guidedoc-cpp`): this crate keeps the glue
  (signatures, describing, scopes, xref lookup data). Its ids follow Sphinx exactly,
  each id piece written once (`PFidE`, not `PFiddE`).
- **Reads allocate by design**, so no test counts allocator calls here.
