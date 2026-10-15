# guidedoc-rst-writer

The reStructuredText writer of Guidedoc: renderer id `rst`. It writes a document tree
(docs/manual/internals/ast.rst) back out as clean reStructuredText that reads back into
the same tree, so Guidedog converts MyST/Markdown and reStructuredText to
reStructuredText itself (`guidedog convert page.md --from myst --to rst -o page.rst`).
Std-only; it has no Odin counterpart (the Odin project used pandoc).

## Use

```toml
[dependencies]
guidedoc-rst-writer = "0.7"
guidedoc = "0.7"
guidedoc-commonmark = "0.7"
```

We render CommonMark as reStructuredText:

```rust
use guidedoc::{Registry, Workspace, convert_text};

fn main() -> Result<(), guidedoc::Fail> {
    let registry = Registry {
        readers: vec![guidedoc_commonmark::READER],
        renderers: vec![guidedoc_rst_writer::RENDERER],
    };
    let mut ws = Workspace::new();
    let out = convert_text(&mut ws, &registry, "commonmark", "rst", "# Hi *there*\n")?;
    println!("{out}");
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/guidedoc-rst-writer). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
use guidedoc_rst_writer::{RENDERER, Settings, render_with};

let registry = guidedoc::Registry { renderers: vec![RENDERER], ..Default::default() };
// With a RenderContext `ctx` (the engine makes one per render):
// render_with(&mut ctx, Settings { width: 79 })?;   // the registry uses width 92
```

- `RENDERER` / `renderer()`, `render` (default settings), `render_with(ctx, Settings)`.
- `Settings { width }`: paragraphs wrap at `width` columns (default `DEFAULT_WIDTH` = 92),
  never inside inline markup or URLs.
- `LOSSY` (`rst.write.lossy`, a Note): something reStructuredText cannot express exactly;
  `DEPTH_LIMIT` (`rst.limit.depth`, Error/Limit): nesting past the limits.
- `walk.rs` holds the one exhaustive match over `NodeKind`.

## Output conventions

Section adornments `=`, `-`, `~`, `^`, `"`, `'`, ... by level (a promoted title takes
`=`); `-` bullets unless the tree says otherwise; enumerators as read; literal blocks
announced by `::` on the paragraph before them; `code-block` (Sphinx) or `code`
(Docutils) with `:linenos:`, `:lineno-start:`, `:caption:`, `:name:`; grid tables for
tables with spans or read from a grid (exact column widths), simple tables when a grid
cannot keep a simple table's widths, list tables for block content or tables too wide
for a grid; `.. role::` declarations at the top for custom roles and for markup with
classes, a language, or a raw format; `rst-class` (Sphinx) or `class` (Docutils) for
classes of plain blocks; domain objects rebuilt as `.. py:function:: f(a, b=1) -> int`,
info fields as `:param int a: text`.

## Written approximately (a Note each)

- Hard line breaks: a space (reStructuredText paragraphs have none; use a line block).
- Markup nested in markup (`**bold *em***`, code inside a link): the inner markup's text.
- An image inside text: a substitution reference and an `image` definition after the
  block. An image or link title: dropped. An empty image source.
- A link to a Markdown heading anchor: a reference to the section title.
- An anchor in a link to a document (`[x](page.md#a)`): `:doc:` without the anchor.
- Several definitions of one term (MyST): one definition. A term with no definition.
- A block quote with a class and no comment before it: the class, an empty comment, the
  quote. A directive the writer does not know: its name, Custom attributes, content.
- Code or a parsed literal whose every line is indented (the reader dedents it); an empty
  code block; the words after a fence's language; a backquote in inline math.
- White space at the ends of `:math:` or raw roles: moved outside (elsewhere it is kept
  with escaped spaces, `\ `).
- An index entry inside text that no following text matches.

Dropped without a note, because they are reader artifacts rather than content: MyST's
classes (`myst`, `simple`, `contains-task-list`, `task-list-item`, `block_break`,
`footnotes`, `amsmath`, `text-*` alignment of cells), the `docinfo` class, generated
index entries of glossaries and of `envvar`/`pep`/`rfc`, and domain objects' generated
annotations and module prefixes (written as `:module:` or the package directive).

## Round trips

`tests/roundtrip.rs` reads a source, writes it, reads the output, and compares the two
`dump_tree` outlines (soft breaks as spaces, as the writer rewraps):

- docs/manual (Sphinx reader): 69/70.
- guidedoc-rst conformance sources (Docutils settings per corpus.txt): 77/98.
- myst-parser fixtures (MyST with the Sphinx layer, read back as Sphinx RST, normalized
  for MyST-only attributes): 185/202.

Remaining differences: grid widths that a cell's unbreakable content outgrows; anonymous
references inside a `header` directive (the reader matches them in another order); the
targets and comments Docutils moves after a promoted title next to an abstract field;
a field list whose name holds a role (`:math:` as default role); custom-role classes the
reader adds (`raw-html`); trailing no-break spaces; the lossy cases above.
