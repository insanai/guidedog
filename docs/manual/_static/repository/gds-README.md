# guidedog discussions (gds)

Guidedog Discussions (GDS) is the formal RFC and design-proposal process for the Guidedog project. It tracks architecture proposals, design rationale, API contracts, and consensus review history.

All records, the registry, and draft templates are authored in reStructuredText (`.rst`), while PDF documents are typeset using Typst templates.

Read [the workflow guide](workflow.rst) and [GDS 0001 (Process Specification)](records/0001-gds-process.rst) to learn how proposals progress through the review cycle.

quick commands
--------------

From the repository root:

```sh
# Refresh the registry and discussion catalog index
build/guidedog gds index

# Check records for style and structural validity
build/guidedog gds check

# Build both HTML site and all PDF records
build/guidedog gds build --target=both

# Build a single record's PDF preview
build/guidedog gds build guidedoc-odin --target=pdf

# Preview the discussion website locally
build/guidedog serve docs/gds
```

lifecycle and records
---------------------

Proposals advance through a structured lifecycle:
`prediscussion` → `discussion` → `accepted` → `published` → `committed` (or `abandoned`).

Key active records:
  - [GDS 0001: Discussion Process](records/0001-gds-process.rst)
  - [GDS 0002: Guidedog CLI and Toolchain](records/0002-guidedog-cli.rst)
  - [GDS 0003: Guidedoc Conversion Core](records/0003-guidedoc-odin.rst)
  - [GDS 0004: Memory Budgets and Trust](records/0004-budgets-and-trust.rst)
  - [GDS 0005: Odin Domain and API Documentation](records/0005-odin-domain.rst)
  - [GDS 0006: Projects and Publication](records/0006-sphinx-replacement.rst)
  - [GDS 0007: Discussion Catalogs](records/0007-discussion-catalogs.rst)
  - [GDS 0008: GitHub Pages Publishing](records/0008-github-pages-publishing.rst) — in discussion; implemented in 0.6.0 as `guidedog-pages`
  - [GDS 0009: Native Plugin API and Registry](records/0009-native-plugin-api.rst) — in discussion; implemented in 0.6.0 as `lib/plugins`

The 0.6.1 review updates GDS 0008 and 0009 with request deadlines, memory ownership,
credential-store qualification and recovery evidence. Both await the maintainer's
acceptance; their state does not change just because code ships.

The October beta review assigned permanent numbers to the three implemented drafts.
Their lifecycle events retain the decision and evidence. GDS 0002 remains published
with explicit unfinished obligations; a working command does not complete every
future promise in that broader record. See the [beta review](../manual/evidence/beta-review-20261011.md).

New drafts are created with number `XXXXX` (e.g. `guidedog gds new "Feature title"`). Permanent numbers are assigned upon promotion into discussion.

safety and transactions
-----------------------

All GDS metadata modifications (`new`, `promote`, `state`, and `index`) use transactional file locking and atomic stage replacement:
  - Use `--dry-run` to preview planned changes before applying them.
  - Lifecycle state changes require a clear rationale string.
  - If a command is interrupted, `guidedog gds recover --rollback` safely restores prior state from the journal.

Related documentation: [User Manual](../manual/README.md) and [API Reference](../api/README.md).
