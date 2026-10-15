# Reference data

Upstream data Guidedog's generated tables come from, kept so every table can be checked
against the same bytes, offline. Guidedog never loads these files at run time: the tables
are committed Rust source. Nothing here is run; the Python files are style definitions
read as text.

| Data | Version | From | Licence | Tables |
|---|---|---|---|---|
| `UnicodeData-17.0.0.txt` | Unicode 17.0.0 | https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt | Unicode License v3 (`LICENSE-UNICODE.txt`) | `crates/guidedoc/src/tables/nfd_*.rs`, `idfold.rs`; with the next file `crates/guidedoc-rst-inline/src/punctuation_table.rs` |
| `DerivedGeneralCategory-17.0.0.txt` | Unicode 17.0.0 | https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt | Unicode License v3 | `punctuation_table.rs`, `crates/guidedoc/src/tables/categories.rs` |
| `CaseFolding-17.0.0.txt` | Unicode 17.0.0 | https://www.unicode.org/Public/17.0.0/ucd/CaseFolding.txt | Unicode License v3 | `crates/guidedoc/src/tables/casefold.rs` |
| `entities.json` | WHATWG HTML (living standard; downloaded 2026-10-15, SHA-256 `d741d877ac77c4194c4ad526b5b4a19aef8dfe411ab840a466891cdbb9f362e6`) | https://html.spec.whatwg.org/entities.json | CC BY 4.0 (WHATWG) | `crates/guidedoc-commonmark/src/inline/entity_table.rs` (the names that end in ";") |
| `pygments-2.19.2/styles/*.py` | Pygments 2.19.2 | the release's `pygments/styles/` | BSD-2-Clause (`pygments-2.19.2/LICENSE`, `AUTHORS`) | `crates/gd-highlight/src/style_tables.rs` |
| `sphinx-9.1.0/pygments_styles.py` | Sphinx 9.1.0 | the release's `sphinx/pygments_styles.py` | BSD-2-Clause (`sphinx-9.1.0/LICENSE.rst`, `AUTHORS.rst`) | Sphinx's own `sphinx` and `none` styles in `style_tables.rs` (its `PyramidStyle` has no name `pygments_style` takes, so it is left out) |

`cargo xtask tables` (tools/xtask/src/tables) writes every table from these files, and
`cargo xtask tables --check` (run by CI) fails when a committed table differs from what
the data gives. The style modules are read by a small Rust reader for the Python they
use, never run. The generator is deterministic: run again on the same data, it
reproduces the committed tables byte for byte.

`SHA256SUMS` records every data file here; `cargo xtask tables` checks them before it
reads anything. A new upstream version is added beside the old one, its tables
regenerated, the old files removed, and `SHA256SUMS` rewritten.
