# gd-rustdoc cheat sheet

`gd-rustdoc` documents Rust APIs for Guidedog, as odindoc does for Odin and autodoc for
Python. It reads a crate's source with tree-sitter (nothing is compiled or run), follows
its module tree the way rustc does, and writes reStructuredText of the `rust` domain:
one page per documented module plus an index page. Guidedog's Sphinx reader renders the
domain.

## Use

```toml
[dependencies]
gd-rustdoc = "0.7"
```

We write the reStructuredText pages for the crate in the current folder:

```rust
fn main() -> Result<(), gd_rustdoc::Error> {
    let pages = gd_rustdoc::document_crate(std::path::Path::new("."))?;
    for page in &pages {
        println!("{}: {} bytes", page.path, page.text.len());
    }
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-rustdoc). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
use gd_rustdoc::{Options, document, document_crate, load_crate};
use std::path::Path;

// One call: default options, load warnings on the first page.
let pages = document_crate(Path::new("crates/gd-json"))?;

// Staged, with options.
let options = Options { include_private: true, ..Options::default() };
let krate = load_crate(Path::new("crates/gd-json"), &options)?; // Err: root unreadable
for warning in &krate.warnings { eprintln!("{warning}"); }    // Elm-style Display
for page in document(&krate, &options) {
    std::fs::write(Path::new("api").join(&page.path), &page.text)?; // index.rst, gd_json.rst
}
```

| Item | What it is |
|---|---|
| `load_crate(path, &Options) -> Result<Crate, Error>` | a folder with `Cargo.toml` (`[lib] path`, `src/lib.rs`, `src/main.rs`), a folder with `lib.rs`/`main.rs`/`mod.rs`, a folder of loose `.rs` files (each a public module), or a root `.rs` file |
| `document(&Crate, &Options) -> Vec<Page>` | `index.rst` then one page per module; never fails |
| `document_crate(path) -> Result<Vec<Page>, Error>` | both, with default options |
| `Options` | `include_private`, `document_hidden`, `crate_name`, `max_items` (100 000), `max_files` (4 096), `max_file_bytes` (4 MiB), `max_depth` (512), `member_order` (`Source`/`Alphabetical`), `noindex` |
| `Page` | `path` (`index.rst`, `my_crate.rst`, `my_crate/parser.rst`), `module` (`my_crate::parser`), `text`, `warnings` |
| `Crate` | `name`, `root_dir`, `files`, `modules: Vec<Module>`, `items: Vec<Item>`, `warnings`, `inputs` (every file read or tried, sorted: `Cargo.toml`, module files, `include_str!` docs, so a caller can tell whether earlier pages still hold); ids `ModuleId`, `ItemId`, `FileId` index the vectors |
| `Diagnostic` / `Error` | `code`, `title`, `message`, `hint`, `file`, `position` (line, column, excerpt); `Display` is the Elm layout; `Error` boxes one |

Warning codes: `rust.autodoc.parse` (syntax error, the rest is documented),
`rust.autodoc.module` (no file for `mod x;`), `rust.autodoc.nesting`, `rust.autodoc.size`,
`rust.autodoc.limit` (files or items), `rust.autodoc.confined` (file outside the crate
folder), `rust.autodoc.read`, `rust.autodoc.link` (an intra-doc link into this crate that
names nothing), `rust.autodoc.private-link` (a link to a private or hidden item). Error
codes: `rust.autodoc.crate`, `rust.autodoc.root`, `rust.autodoc.read`,
`rust.autodoc.grammar`.

## What is documented, and where

- Rustdoc's rules: an item is documented in its module when it is `pub` and every module
  on the way is `pub`. `include_private` documents everything (`pub(crate)` and private
  items get `:visibility:`). `#[doc(hidden)]` items need `document_hidden`. `#[cfg(test)]`
  modules and `#[test]` functions are never documented.
- `pub use` of an item that is documented anyway is a re-export line (`rust:use`). A
  `pub use` of an item that is not reachable otherwise (it lives in a private module)
  documents the item in place, under the re-export's name (rustdoc's inlining);
  `#[doc(inline)]` and `#[doc(no_inline)]` force either. `pub use private::*` inlines
  every public item of the module. External re-exports (`pub use std::...`) are lines.
- `#[macro_export]` macros are documented at the crate root.
- `impl` blocks attach to the crate type their self type names (resolved through `use`
  imports): inherent impls first, then trait impls. An impl whose type is not in the
  crate (`impl From<X> for String`) is listed under "Implementations" on its module page.
  Trait-impl members are written `:noindex:` (they are documented on the trait).
- A type's content is grouped as rustdoc groups it, each group under a rubric: "Fields" (or "Variants"), "Implementations" (inherent impls, in source order) and "Trait Implementations" (sorted by trait name). A trait ends with "Implementors": the head of every impl of it found anywhere in the crate, sorted by the implementing type, each with `:module:` set to the impl's module so the paths it writes resolve.
- Tuple fields, and the fields of enum variants, are listed only when one of them has docs (rustdoc's rule); a tuple struct's signature shows fields that are not documented on the page as `_` (with `include_private`, private ones show their types).
- `#[cfg(...)]` and `#[doc(cfg(...))]` become `:cfg:`; several are joined as `all(a, b)`.
- `#[doc = "..."]`, `#[doc = include_str!("file")]` (read relative to the source file,
  only inside the crate folder), `///`, `//!`, `/** */` and `/*! */` docs are joined and
  unindented per fragment, as rustdoc does.
- Signatures are the source on one line: attributes, comments and bodies removed,
  whitespace collapsed, rustfmt's vertical lists folded (`fn f<T>(a: u8) -> X where T:
  Copy`). Constant values longer than 48 characters and every static's value are dropped.

## The `rust` domain (for the Sphinx reader)

The domain mirrors the shape of the Odin domain (GDS 0005): a module directive sets the
context, objects are named by their full path, members nest in their type's content, and
roles link to them with the Python domain's `~` and leading `.`.

### Directives

The argument is the signature as Rust writes the declaration, on one line (visibility
and qualifiers included). The reader finds the name as the identifier after the keyword
(`fn`, `struct`, `enum`, `union`, `trait`, `type`, `const`, `static`, `macro_rules!`,
`mod`), or the first identifier for fields and variants.

| Directive | Signature written | Object named |
|---|---|---|
| `rust:crate` | `my_crate` | `my_crate`; sets the current module |
| `rust:module` | `my_crate::parser` (the full path) | the path; sets the current module |
| `rust:function` | `pub const unsafe fn f<T: Copy>(x: &T) -> Option<T> where T: Send` | `<module>::f` |
| `rust:method` | `pub fn new(x: T) -> Self`, `fn draw(&self) -> u8` (inside a type or impl) | `<module>::Type::new` |
| `rust:struct` | `pub struct Point<T = f32>`, `pub struct Pair(pub i32, _)`, `pub struct Unit` | `<module>::Point` |
| `rust:enum` | `pub enum Kind` | `<module>::Kind` |
| `rust:variant` | `Circle`, `Polygon(u8)`, `Rect { width: f32, height: f32 }`, `Unknown = 9` | `<module>::Kind::Circle` |
| `rust:union` | `pub union Bits` | `<module>::Bits` |
| `rust:trait` | `pub trait Draw: Clone + Send` | `<module>::Draw` |
| `rust:impl` | `impl<T: Copy> Point<T>`, `impl<T> fmt::Display for Point<T>` | never a target (always `:noindex:`) |
| `rust:type` | `pub type Result<T> = std::result::Result<T, Error>` | `<module>::Result` |
| `rust:const` | `pub const MAX: usize = 4_096` | `<module>::MAX` |
| `rust:static` | `pub static mut COUNT: u64` | `<module>::COUNT` |
| `rust:macro` | `macro_rules! shape` (its arms follow in a `code-block` in the content) | `<module>::shape` |
| `rust:field` | `pub x: T`, `0: u8` (tuple fields are named `0`, `1`, ...) | `<module>::Point::x`, `<module>::Kind::Rect::width` |
| `rust:associatedtype` | `type Output: Default`, `type Item = u8` | `<module>::Draw::Output` |
| `rust:associatedconst` | `const PASSES: usize = 1` | `<module>::Draw::PASSES` |
| `rust:use` | `pub use geometry::point::Point`, `pub use std::collections::HashMap` | never a target; links to `:target:` |

Nesting: `rust:field`, `rust:variant`, `rust:method`, `rust:associatedtype`,
`rust:associatedconst` and `rust:impl` appear in the content of `rust:struct`,
`rust:enum`, `rust:union` or `rust:trait`; the members of a `rust:impl` belong to the
type the impl is nested in (its `:for:` when it stands alone at module level); fields of
a variant nest in the `rust:variant`.

### Options

| Option | On | Meaning |
|---|---|---|
| `:synopsis: text` | crate, module | one sentence for the index |
| `:module: path` | any object | describe it in another module than the current one (hand-written docs; generated pages never need it) |
| `:visibility: pub(crate)` | any object | not `pub`: `pub(crate)`, `pub(super)`, `pub(in path)` or `private` (only with `include_private`) |
| `:cfg: predicate` | any object | "Available on `predicate` only." (`feature = "x"`, `all(unix, feature = "x")`) |
| `:unsafe:`, `:async:`, `:const:` | functions, methods, traits | flags, also visible in the signature; for index entries ("f (unsafe function in my_crate)") |
| `:provided:` | trait methods and associated consts | has a default (rustdoc's "Provided methods") |
| `:attributes: #[non_exhaustive] #[repr(C)]` | types, functions | attributes rustdoc shows above the signature |
| `:deprecated: note` | any object | deprecated; the note may be empty |
| `:for: type` | impl | the self type as written |
| `:trait: path` | impl | the trait as written (`!Send` for negative impls) |
| `:target: path` | use | the documented path of what is re-exported, when it is in the crate |
| `:noindex:` | any object | no target, index entry or inventory entry |

### Roles and targets

Targets are full paths with `::`, the crate's name first: `my_crate`,
`my_crate::parser::Lexer`, `my_crate::parser::Lexer::next_token`, `my_crate::Kind::Rect`,
`my_crate::Kind::Rect::width`. Ids are `make_id("rust-" + path)` with `::` written `.`
(`rust-my_crate.parser.Lexer.next_token`); a module's is `rust-module-my_crate.parser`.

| Role | Finds |
|---|---|
| `:rust:mod:` | crates and modules |
| `:rust:fn:` | functions and methods |
| `:rust:struct:`, `:rust:enum:`, `:rust:union:`, `:rust:trait:` | those types |
| `:rust:variant:`, `:rust:field:` | enum variants; struct, union and variant fields |
| `:rust:type:` | type aliases and associated types |
| `:rust:const:` | constants and associated constants |
| `:rust:static:`, `:rust:macro:` | statics; macros |
| `:rust:any:` | any of them; also what generated pages write for links they could not resolve inside the crate (`std::vec::Vec`), for the reader to resolve through inventories or show as code |

Generated roles always carry the full path, with an explicit title when the written text
differs: ``:rust:struct:`Canvas <my_crate::Canvas>` ``. Hand-written ones may use the
Python domain's forms: `~my_crate::parser::Lexer` shows `Lexer`, a leading `.` finds the
name at the end of any target, and a relative name is looked up in the current module
first. Index entries read "Lexer (struct in my_crate::parser)".

## Markdown handling (the choice)

Doc comments are CommonMark; the generated page is reStructuredText. Two designs were
weighed:

1. **reST pages, docs translated** (chosen). Each item's docs are converted to reST in
   its directive's content: paragraphs; `#` headings (`# Examples`, `# Panics`,
   `# Errors`, `# Safety`, setext too) as `.. rubric::`, because sections cannot sit
   inside directives; fenced and indented code as `.. code-block::` with rustdoc's
   language rules (no info, `ignore`, `no_run`, `should_panic`, `compile_fail`,
   `edition2021`, `E0308`... mean `rust`; `text`, `toml`, ... are kept) and rustdoc's
   hidden `# ` lines removed (`##` shows `#`); bullet and ordered lists (nested, start
   number kept); block quotes; pipe tables as `list-table`; HTML blocks as `raw:: html`;
   inline code, emphasis, strong, `~~strike~~` (text kept), links and autolinks as
   anonymous hyperlinks; and intra-doc links (`` [`Foo`] ``, `[Foo]`, `[text](Foo::bar)`,
   `[text][Foo]`, reference definitions, disambiguators `struct@`, `fn@`, `foo()`,
   `foo!`, generics dropped) resolved like rustdoc, relative to the item's module and
   `Self`, as `rust` roles. Everything else reST would read as markup is escaped.
2. **MyST pages with `{rust:function}` fences.** The Markdown would pass through
   untouched, but rustdoc's dialect still needs rewriting (hidden `# ` lines, untyped
   fences meaning Rust, intra-doc links that are not MyST links), headings inside a
   directive would still be invalid, and the reader would need MyST directive fences
   for every object.

The first is more faithful: every construct rustdoc renders gets a deliberate reST form,
and the reader's existing reST machinery (rubrics, code blocks, roles) does the rest.

Footnotes (`[^1]`) become auto-numbered footnotes (`[#d3-f1]_`) defined at the end of the item's docs; `d3` numbers the doc comments of a page, so labels never clash. An image paragraph becomes `.. image::` with `:alt:` (and `:target:` for a linked badge), an image in running text a substitution defined after the docs; images with a relative path keep only their alt text, since nothing places them next to the generated page. A paragraph with a hard line break (two trailing spaces or a backslash) becomes a line block. Not translated: nested emphasis inside strong (reST cannot nest; the inner text is kept).

Links into the standard library (primitives `[u8]`, `prim@str`, `[u8::MAX]`, paths starting `std::`, `core::` or `alloc::`, and prelude names such as `[Vec]` or `[Some]` that the crate does not define) become hyperlinks to doc.rust-lang.org. The standard library publishes no inventory a reader could use, so a stable URL is the practical choice: primitives and a table of common items get their exact pages (`struct.Vec.html`, `#method.new`), a disambiguator gives the page kind for the rest (`macro@`, `mod@`, `struct@`), and anything else opens rustdoc's search for the path. The shown text drops a written disambiguator (`[`prim@u8`]` shows `u8`), as rustdoc does.

## Design notes

- gd-rustdoc is two plain values: `load_crate` builds an owned `Crate` (flat `Vec`s of
  modules and items addressed by newtype ids) and `document` borrows it to write owned
  `Page`s. Nothing needs freeing or initialising.
- Problems are values: `Result<Crate, Error>` only for an unusable root, `Diagnostic`s
  on the crate and on each page for everything else, with `Option` for an absent file or
  position rather than sentinel zeros.
- Bounds: files are read only below the crate folder, within `max_file_bytes` and
  `max_files`; trees deeper than `max_depth` (measured with a cursor walk) are skipped;
  item creation stops at `max_items`. All tree walks use cursors or explicit stacks;
  the Markdown converter recurses only for nested lists and quotes, at most
  `MAX_NESTING` (12) levels deep.

Source map: `load.rs` (finding files), `parse.rs` (syntax to items), `attrs.rs` (docs
and attributes), `sig.rs` (signatures), `resolve.rs` (paths, impl attachment),
`plan.rs` (what is documented where), `emit.rs` (pages), `markdown.rs` and `inline.rs`
(doc comments to reST), `std_links.rs` (doc.rust-lang.org URLs), `diag.rs` (diagnostics), `model.rs` (the model).
