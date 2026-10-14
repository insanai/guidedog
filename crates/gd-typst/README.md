# gd-typst cheat sheet

The Typst toolchain for the host: the embedded compiler (feature `embedded`, default) or
an installed, pinned `typst` program.

| Item | Does |
|---|---|
| `locate(program) -> Option<Backend>` | `Backend::Embedded` or `Backend::Program { path, version }` |
| `backend.version()`, `.is_pinned()`, `.is_embedded()` | against `PINNED_VERSION` |
| `compile_file(&backend, source, root, output, &Options)` | `Result<Compiled, Error>` |
| `compile_text(&backend, text, root, output, &Options)` | generated text, no file beside the sources |
| `Options { target, offline, package_cache, embedded_fonts_only, font_paths, reserve }` | `Default`: PDF, online, system fonts |
| `Diagnostic::report(source) -> gd_report::Report`, `.render(source)` | Elm-style with excerpt and carets |
| `Error::{Compile, Launch, Io, OutsideRoot, ...}`, `.report()` | codes `typst.*` |

## Design notes

- `compile_*` return `Result<Compiled, Error>` instead of an `Outcome` with `ok`, an
  error option and exit codes 0–5 that the embedded backend invented: a source with
  errors is `Error::Compile { exit_code, diagnostics, detail }` (`exit_code` is `None`
  for the embedded compiler); `Compiled` carries the warnings.
- `Backend` is an enum; the `"embedded"` program name, the `""` version and the
  `embedded` flag are gone, and `is_pinned` is a method.
- The target moved into `Options` (`target: Target`, PDF by default), so the compile
  functions take five arguments.
- Diagnostics and errors render through `gd-report` (shared excerpt, caret and escaping
  rules); `Diagnostic::report` now returns the `Report`, `render` the text. Codes are
  `typst.*` (were `typst-*`).
