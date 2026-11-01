# guidedog discussions (gds)

Guidedog Discussions (GDS) is the formal RFC and design-proposal process for the Guidedog project. It tracks architecture proposals, design rationale, API contracts, and consensus review history.

All records, the registry, and draft templates are authored in reStructuredText (`.rst`), while PDF documents are typeset using Typst templates.

Read [the workflow guide](workflow.rst) and [GDS 0001 (Process Specification)](records/0001-gds-process.rst) to learn how proposals progress through the review cycle.

quick commands
--------------

This folder is a discussion catalog like any other, kept with `guidedog dsc`. In
`docs/gds`, after `cargo build --release -p guidedog-cmd`:

```sh
# Refresh the registry and discussion catalog index
../../target/release/guidedog dsc index

# Check records for style and structural validity
../../target/release/guidedog dsc check

# Build both HTML site and all PDF records
../../target/release/guidedog dsc build --target=both

# Build a single record's PDF preview
../../target/release/guidedog dsc build guidedoc-engine --target=pdf

# Preview the discussion website locally
../../target/release/guidedog serve .
```

lifecycle and records
---------------------

Proposals advance through a structured lifecycle:
`prediscussion` -> `discussion` -> `accepted` -> `published` -> `committed` (or `abandoned`).

Records:
  - [GDS 0001: Discussion Process](records/0001-gds-process.rst)
  - [GDS 0002: Guidedog CLI and Toolchain](records/0002-guidedog-cli.rst)
  - [GDS 0003: Guidedoc Conversion Core](records/0003-guidedoc-engine.rst)
  - [GDS 0004: Memory Budgets and Trust](records/0004-budgets-and-trust.rst)
  - [GDS 0005: Odin Domain and API Documentation](records/0005-odin-domain.rst)
  - [GDS 0006: Projects and Publication](records/0006-sphinx-replacement.rst)
  - [GDS 0007: Discussion Catalogs](records/0007-discussion-catalogs.rst)
  - [GDS 0008: GitHub Pages Publishing](records/0008-github-pages-publishing.rst)
  - [GDS 0009: Native Plugin API and Registry](records/0009-native-plugin-api.rst)
  - [GDS 0010: Typed Build Plugins](records/0010-typed-build-plugins.rst)

  - [GDS 0011: Built-in Extensions](records/0011-sphinx-builtin-extensions.rst)
  - [GDS 0012: Single Executable Release](records/0012-single-executable-release.rst)
  - [GDS 0013: Static Publishing](records/0013-static-site-generation.rst)
  - [GDS 0014: Typst Diagrams](records/0014-typst-diagrams.rst)
  - [GDS 0015: Engraved LilyPond Music](records/0015-lilypond-music.rst)

New drafts are created with number `XXXXX` (e.g. `guidedog dsc new "Feature title"`). Permanent numbers are assigned upon promotion into discussion.

safety and transactions
-----------------------

All GDS metadata modifications (`new`, `promote`, `state`, and `index`) use transactional file locking and atomic stage replacement:
  - Use `--dry-run` to preview planned changes before applying them.
  - Lifecycle state changes require a clear rationale string.
  - If a command is interrupted, `guidedog dsc recover --rollback` safely restores prior state from the journal.

Related documentation: the [user manual](../manual/) and the crate cheat sheets
(`crates/*/README.md`).

The [31 October 2026 status review](evidence/status-review-20261031.rst) audits all fourteen records. [GDS 0014](records/0014-typst-diagrams.rst) specifies embedded Typst diagrams. GDS 0002 remains published because its hostile-input worker and native Typst chapter contract are unfinished; the other records in that audit are committed. GDS 0015 is accepted for implementation after the 1 November design review.
