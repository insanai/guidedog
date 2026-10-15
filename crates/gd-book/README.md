# gd-book cheat sheet

Guidedog's `pdf`, `text` and `dummy` builders, implementing `gd_resolve::OutputBuilder`:
book composition and ordering, the book index, the Typst read roots and the bundled fonts.

```rust
use gd_resolve::OutputBuilder;
let labels = ui.labels();                                   // gd_intl::UiTranslator
gd_book::PdfBuilder { labels: Some(&labels), ..Default::default() }.write(&view, &mut generation, &mut reports)?;
gd_book::TextBuilder { labels: Some(&labels) }.write(&view, &mut generation, &mut reports)?;
gd_book::DummyBuilder.write(&view, &mut generation, &mut reports)?;
assert_eq!(gd_book::language_file("manual.pdf", "de"), "manual-de.pdf");
```

| Item | Does |
|---|---|
| `PdfBuilder { labels, typeset }` | one book per `pdf_documents` entry: `<file>-<lang>.pdf`, `sources/<file>-<lang>.pdf.typ`, and `book-<lang>.typ` for a single book; unit `book:<file>`. `typeset` collects a `TypesetBook` (file, each page's text) per book Typst typeset, which the tests check instead of reading the PDF back |
| `TextBuilder { labels }` | `<docname>.txt` per document, one unit each |
| `DummyBuilder` | links every document (references checked), writes nothing |
| `language_file`, `typst_string`, `typst_markup` | book names and Typst escaping |
| `bundled_font_dir` | writes Libertinus Sans (OFL 1.1) to `<doctrees>/fonts` for Typst |

Problems: `build.pdf.duplicate`, `build.pdf.root`, `build.output` (a chapter or page
could not be made), `build.builder` (no text renderer), `typst.missing`, `typst.root`,
`typst.compile` (with `typst.compiler` reports), `typst.launch`, `host.budget`.
Warnings: `build.untrusted` (project book template left out), Typst's own.

## Design notes

- **Builders are values** implementing `OutputBuilder` over a borrowed `BuildView`; the
  translated renderer labels are a field, the highlighter
  (`guidedoc_defaults::TypstHighlighter`) is made per worker thread.
- **Book inputs come from the pages**: a chapter's `Rendered::assets` list the files its
  Typst reads (images, graphs) with book paths as targets; with the template and the
  logo they are the files the book unit stamps (path, size, time). No build-wide state
  records them.
- **Parallel rendering**: chapters and text pages render on `-j` workers
  (`gd_host::run_workers`), each owning a `gd_link::Worker`; results are written in
  reading/docname order, so output does not depend on the thread count.
- **Confined Typst without a link farm (embedded)**: the embedded compiler runs in
  `gd_link::book_root` with `gd_typst::Options::read_roots` set to the
  readable roots and the graphs folder, checked with every link followed. The private
  folder of symbolic links (`typst_root.rs`) is made only for the external `typst`
  program, which has no such option; it is removed on `Drop`, never following a link.
- **Diagnostics are placed**: an error in a chapter is reported at the
  reStructuredText/Markdown span the source map gives (`gd_link::source_location`),
  with the generated line named; one in the project's template at that file with an
  excerpt; anything else at the generated book, which a failed build keeps in
  `<doctrees>/failed/` (`gd_host::publish`). Typst's warnings on a successful build are
  reported too and kept with the unit.
- **Dummy links for text**: the dummy builder links pages for the text format, so it
  checks references without drawing graphs.
- `compile_text` returns `Result<Compiled, gd_typst::Error>`; a budget refusal of the
  PDF (`OutputTooLarge`) is the `host.budget` problem.
- Fonts are embedded with `include_bytes!` and written only when missing or a different
  size, so repeated builds do not rewrite them.
