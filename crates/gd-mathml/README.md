# gd-mathml cheat sheet

Formulas as MathML Core, so an HTML page shows them without a script. LaTeX goes through
`guidedoc_typst::translate_math` and the embedded Typst compiler's HTML export, then
through `guidedoc_html::filter_mathml`.

| Item | Does |
|---|---|
| `MathMl::new()`, `MathMl::with_limits(Limits)` | one per thread; holds the cache |
| `math.prepare(doc, &mut builder) -> PassOutcome` | the body of a pass: typesets new formulas in one compiler run, reports each refused one |
| `math.prepare_counted(..) -> Prepared { formulas, typeset, refused }` | the same, counted |
| `impl guidedoc::MathWriter for MathMl` | set as `RenderOptions::math_writer`; writes `<math>` or declines |
| `math.mathml(latex, display) -> Option<String>` | a prepared formula |
| `Limits { max_formula_bytes, max_formulas, max_cache_bytes }` | 16 KiB, 8,192, 32 MiB |
| `MATHML_CSS` | Typst's rules for aligned equations and cases; a host adds them to its page |
| `MATH_UNSUPPORTED`, `MATH_LOSSY`, `MATH_FAILED`, `MATH_LIMIT`, `MATH_UNAVAILABLE` | warnings `mathml.*` |

## Design notes

- The HTML renderer does not depend on the compiler. It calls a `MathWriter`; this crate
  is the one that needs both.
- A formula is never lost. One the adapter cannot translate, one MathML Core cannot draw
  (Typst says "was ignored during MathML export"), and one past a limit stay LaTeX in
  the renderer's usual markup, each with a warning at the formula.
- One compiler run takes up to 1,024 formulas, one per line. When a line fails, it is
  removed and the rest compiles again, at most 12 times.
- The compiler runs offline with embedded fonts and may read no file.
- Typst writes `style="math-shift: ..."` on a few elements. The filter drops every
  `style` attribute, so the output has none.
- A formula marked `notation = mathml` is the author's MathML. It is skipped here; the
  HTML renderer filters it.
