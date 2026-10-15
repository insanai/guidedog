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
| `pygments-2.19.2/styles/*.py` | Pygments 2.19.2 | the release's `pygments/styles/` | BSD-2-Clause (`pygments-2.19.2/LICENSE`, `AUTHORS`) | `crates/gd-highlight/src/style_tables.rs` |
| `sphinx-9.1.0/pygments_styles.py` | Sphinx 9.1.0 | the release's `sphinx/pygments_styles.py` | BSD-2-Clause (`sphinx-9.1.0/LICENSE.rst`, `AUTHORS.rst`) | Sphinx's own `sphinx`, `none`, and `pyramid` styles in `style_tables.rs` |

The Rust tables were converted mechanically from the tables the Odin implementation
generated from these files (Guidedog 0.6.3), and each table file names its source. The
Odin generators were not ported; a new upstream version needs a small generator that
writes the Rust tables in the same shape, run once and committed with the new data.

`SHA256SUMS` records every data file here; check them with
`shasum -a 256 -c SHA256SUMS` in this folder. A new upstream version is added beside the
old one, its tables regenerated, the old files removed, and `SHA256SUMS` rewritten.
