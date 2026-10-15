# Guidedog manual

This directory holds the source of the official **Guidedog manual**: tutorials, topic guides, practical tasks, the configuration and command reference, internals, and the translated editions.

The manual is written in reStructuredText (`.rst`) with Graphviz diagrams and LaTeX math notation, and Guidedog builds it into an HTML website and PDF books.

Start at [index.rst](index.rst), or see the other documentation projects:
- [API reference](../api/README.md): the Rust API of Guidedog's main crates, generated from their source.
- [Guidedog Discussions (GDS)](../gds/README.md): design records and decisions.

## Overview

- **Tutorials**: a first project, a small book, and a reusable HTML/PDF theme.
- **Topics**: writing, domains, projects, typography, mathematics, diagrams, templates, media, translation.
- **Practical tasks and reference**: CLI commands, configuration, formats, errors, compatibility.
- **Document code**: Python (autodoc), Rust (`rust.rst`), Odin, and declared object types.
- **Internals**: the engine, invariants, publication, memory, and the design notes of the Rust crates.
- **API appendix**: generated from the crates listed in `rust_autoapi_crates` in `conf.toml`, so PDF books link prose and code together.

`evidence/` keeps historical review and verification records of the Odin implementation; it is not part of the build.

## Building

Build Guidedog first (`cargo build --release -p guidedog-cmd` at the repository root), then, from the root:

```sh
target/release/guidedog build html docs/manual -j auto -W
target/release/guidedog build pdf docs/manual -W
target/release/guidedog serve docs/manual
```

### Translated editions

The manual is translated into Simplified Chinese (`zh_CN`), Japanese (`ja`), Korean (`ko`), Hindi (`hi`), Spanish (`es`), and German (`de`). PO catalogs are in `locales/<language>/LC_MESSAGES/*.po`; compiled `.mo` files are made during the build and ignored by Git.

The catalogs were copied unchanged from the Odin implementation's manual. Pages rewritten for the Rust port (internals, plugins, about, repository) and the new `rust.rst` have new or changed messages; refresh them with the `intl` commands:

```sh
target/release/guidedog build -b gettext docs/manual docs/manual/locales/templates -W
target/release/guidedog intl update -p docs/manual/locales/templates \
  -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
target/release/guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
```

Build one edition:

```sh
mkdir -p docs/manual/_editions
target/release/guidedog build -b html docs/manual docs/manual/_editions/de \
  -D language=de -D 'html_extra_path=[]' -j auto -W
target/release/guidedog build -b pdf docs/manual docs/manual/_editions/books/de -D language=de -W
```

The English build copies `_editions` into the site (`html_extra_path`), so build the editions first. Typst applies language-aware hyphenation, paragraph breaking, and non-Latin shaping. CJK editions use Noto Serif CJK and Hindi uses Noto Serif Devanagari, from `docs/shared/fonts` and `_static/data/fonts`. The language selector works without JavaScript; JavaScript adds detection of the browser's language.

`cargo xtask fetch-fonts` downloads the CJK fonts into `docs/shared/fonts` with checked SHA-256 sums. For the complete release site, run `cargo xtask build-site`: it builds the landing page and the manual in all seven languages, their PDF books, and the GDS project, and assembles them with relative links.

## Reusing the manual theme

1. Create a project: `guidedog quickstart mydocs`.
2. Copy `_templates` and `_static` from `docs/manual/`.
3. Copy the shared assets from `docs/shared/_static` into our static folder.
4. For multilingual PDF books, copy `docs/shared/fonts` and set `pdf_font_paths`.
5. Set `templates_path`, `html_static_path`, and the stylesheets in `conf.toml`.

## Repository copies

`_static/repository/` holds byte-for-byte copies of repository files the manual offers as downloads (a documentation project publishes only its own files): `LICENSE`, `NOTICE`, the `guidedoc`, `gd-discussion` and GDS READMEs (`library-README.md`, `discussion-README.md`, `gds-README.md`), the plugin examples from `crates/gd-plugins/examples` (`build-plugin-badge.rs`, `build-plugin-main.rs`, `publisher.rs`), and the beta review. Copy the original over a copy when it changes.

## License and credits

The manual and its templates are part of Guidedog, released under the **GNU Affero General Public License v3.0 or later** (AGPL-3.0-or-later); see [LICENSE](../../LICENSE) and [NOTICE](../../NOTICE). Created by Vikrant Rathore, with assistance from Ronak Rathore and Kanak Rathore. Bundled fonts are under the SIL Open Font License; see `docs/shared/fonts/` for licenses and provenance.
