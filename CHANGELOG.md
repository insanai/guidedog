# Changelog

Guidedog follows [Semantic Versioning](https://semver.org). The version lives in the
`VERSION` file and in `[workspace.package]` of the root `Cargo.toml`, which every crate
inherits; `guidedog --version` reports it, with the Guidedoc engine and document schema
beside it.

## 1.0.0-beta.1 - 2026-10-15

First beta release of the Rust workspace in `insanai/guidedog`.

- Rust 1.99 toolchain, pinned for reproducible release builds.
- Native Linux and macOS archives for x86-64 and ARM64, and a Windows x86-64 ZIP with the Graphviz runtime bundled.
- One `guidedog` executable in every platform archive, with linked Pages and discussion plugins and the `gd-graphdog` Graphviz library, with embedded Typst, notices, build metadata, SHA-256 checksums and build provenance.
- Guidedog builds its own website, multilingual manual, API reference and discussion catalog for GitHub Pages.
- The official Homebrew tap installs the new Rust release.
- Prerelease tags publish GitHub prereleases and do not replace the latest stable release.

## 0.7.0 — 2026-10-15

A self-contained Guidedog in one Cargo workspace. Commands, options, `conf.toml` keys,
directives, roles and output stay compatible, except the discussion commands below;
existing projects build as before.

- **One toolchain.** `cargo build --release -p guidedog-cmd` builds Guidedog; `cargo test
  --workspace` runs every suite and `cargo run -p check` checks the source limits. No
  other compiler, build script or separate Typst bridge is needed.
- **Embedded Typst by default.** The Typst 0.15.1 compiler is a regular dependency of
  `gd-typst` (the `embedded` feature); an installed `typst` program remains the fallback.
  tree-sitter and its Python grammar are ordinary dependencies.
- **Self-contained release programs.** Graphviz 16.1.0 and Expat 2.7.1 are linked
  statically into `guidedog` and `graphdog` (`cargo xtask graphviz` builds them from
  pinned sources); a build from source without `GUIDEDOG_GRAPHVIZ_STATIC` links the
  system's Graphviz instead. HTTP and HTTPS are built in (`ureq` with `rustls` and
  Mozilla's root certificates): neither libcurl nor the `curl` program is needed.
- **Reusable crates.** The engine (`guidedoc`), the readers and renderers
  (`guidedoc-rst`, `guidedoc-commonmark`, `guidedoc-sphinx`, `guidedoc-html`,
  `guidedoc-typst`, `guidedoc-text`), and the libraries (`gd-*`) are separate crates with
  one workspace version. Each crate's README lists its API.
- **An embeddable engine.** A `Workspace` owns its tables and reuses their capacity,
  so a conversion in steady state allocates nothing. Documents borrow their workspace,
  so stale handles do not compile. `Limits` bound every conversion and report
  `Capacity` or `Limit` failures.
- **Rust API documentation.** The `rust` domain and the `rust_autoapi_*` settings document
  Rust crates from source with tree-sitter, without running the compiler. Guidedog's own
  API reference is generated this way.
- **Plugins in Rust.** `gd-plugins` keeps publisher interface 1.1, build-plugin interface
  1.0 and both process protocols 1.0. Plugins are Rust crates: link them into a
  `guidedog` of your own with `gd_cli::run_with`, or register them as programs as
  before. The product-badge and folder publisher examples are crate examples of
  `gd-plugins`.
- **Discussion catalogs are a plugin of guidedog.** `guidedog dsc init`, `new`, `list`,
  `show`, `promote`, `state`, `index`, `check`, `build`, `recover` and `config` replace
  the `dsc` and `dscc` programs and `guidedog gds`. They work on any catalog, with its
  own prefix, lifecycle, record types and numbering in `discussion.toml`; Guidedog's
  `docs/gds` is one of them (`cd docs/gds && guidedog dsc check`). Builds are
  guidedog's own, so nothing else needs to be installed, and there is no `dscc`
  archive. Catalogs, records, locks and journals are unchanged.
- **`graphdog`** ships in the `guidedog` archive: a `dot`-compatible renderer for DOT to
  SVG or laid-out DOT (PNG too when built against a system Graphviz), with Elm-style
  errors.
- **Examples** are Cargo examples: `cargo run -p guidedoc-examples --example hello`.
- **Licence.** Guidedog is now released under the GNU Lesser General Public License
  v3.0 or later (LGPL-3.0-or-later): `LICENSE` holds its text and `COPYING` the GNU GPL
  it builds on. NOTICE keeps the additional permission to link Graphviz (EPL-2.0).
- **Releases** cover Linux (x86-64, ARM64) and macOS (Apple silicon, Intel). Windows
  has no release archive in this version.

## 0.6.3 — 2026-10-14

- Add opt-in adaptive Graphviz SVGs. Mark a diagram with the
  `graphviz-adaptive` class to follow the website's selected color scheme.
  Ordinary artwork keeps its colors; PDF keeps the light print master.
- Add a complete project-theme tutorial, downloadable HTML and Typst files,
  and an executable check across all HTML builders and PDF. Correct the
  diagrams' plugin lifecycle, native memory boundaries and publication claims.
- Make local download links save their attachments, including HTML template
  source. The site checker validates attachment targets without treating their
  contents as generated pages. PDF downloads use the published `html_baseurl`
  when configured, so a standalone book can reach its examples.
- Complete the six manual translations and accept the implemented plugin designs
  in GDS 0008, 0009 and 0010. GDS 0002 retains its unfinished Typst obligations.

## 0.6.2 — 2026-10-14

- Add typed native build plugins for source transformations, complete HTML pages
  and namespaced static assets. A reusable plugin library works in an embedded
  application or an explicitly registered executable. Build hooks use their own
  versioned contract, separate from the publishing service. This beta does not
  promise compatibility with earlier plugin SDKs.
- Include loaded input fingerprints, declared dependencies and plugin identity in
  cache invalidation. Enforce text and asset limits, portable path conflicts,
  bounded process requests, untrusted-mode refusal and generation preservation.
- Qualify build plugins before CLI registration. Check fresh sessions, repeatable
  declared hooks, generated assets and cleanup through bounded child processes.
  Runtime responses remain checked; crashes and stalls preserve the previous site.
- Replace the publishing-first tutorial with the tested product-badge package and
  its executable wrapper. Explain Pages separately as a publishing service. Record
  the build-hook contract in GDS 0010, which remains under discussion.

## 0.6.1 — 2026-10-13

Publisher reliability fixes and a complete plugin tutorial.

- **Bounded publisher requests.** `guidedog publish --timeout=SECONDS` bounds each
  whole request, including a blocked pipe write (default: 300 seconds). Handshakes
  wait at most ten seconds. Cancellation and shutdown are bounded. A timeout after
  a possible write reports `Unknown` and directs recovery before another publication.
- **Explicit memory ownership.** Process requests have separate scratch arenas;
  sessions, snapshots, plans and results retain their own owners. Unload uses the
  session allocator, including after failed load. Secret copies remember their
  allocator and are wiped before release. Rechecking a snapshot preserves accepted
  custom file limits. The Windows pipe and credential conversions follow native API
  contracts.
- **Windows resource boundaries.** Publisher children inherit only their three
  standard streams. An unrelated pipe can reach EOF while a publisher is running.
  File reads enforce limits on bytes actually read, and allocation refusals cannot
  return partial success. WinHTTP checks redirect policy, status and header setup,
  closes failed body files and finishes partial writes.
  `fetch.Error` adds `Memory`; callers with exhaustive error switches should handle it.
- **Filesystem cleanup without shell extensions.** The new `fs.remove_all` boundary
  replaces shell-based deletion on Windows with direct filesystem iteration and
  removal. The engine, publishers, examples and tests use it. A native diagnostic
  traced the intermittent cleanup crash into a Subversion shell extension. Windows
  traversal uses an explicit stack, closes its handles and keeps link targets intact.
  The same traversal fixes Linux cleanup following directory links into their targets.
- **Durable publication evidence.** An accepted push stays accepted if saving its
  local report fails. The journal remains until that report is durable. Registry,
  profile and binding changes hold an exclusive lock across reading and replacement,
  including against another session in the same process. Concurrent builds and dry
  runs use separate folders.
- **Checked Git messages.** Tree construction and reference/report parsing refuse
  incomplete allocations. Git name sorting does not allocate. Request framing does
  not depend on allocating formatted strings; partial writes and invalid object sizes
  are errors.
- **Native credential qualification.** Windows and macOS tests require native-store
  round trips. Linux qualification starts an isolated D-Bus session and GNOME keyring
  and requires Secret Service storage and removal. No real GitHub token is needed.
- **Developer documentation.** The manual teaches a publisher in ten steps, starting
  with the tested folder example and following the real GitHub Pages implementation.
  Complete source downloads match the tested examples. All seven editions, READMEs,
  GDS records, API cheat sheets and release instructions are updated. Inline-markup
  boundaries are corrected in translated paragraphs without adding visible spaces.

The native SDK is **1.1** and accepts SDK 1.0 publishers. Process protocol **1.0** and
snapshot schema **1** retain their required fields and meanings; actual old/new hosts
and publishers are tested in both directions. This is source compatibility, not a
dynamic-library ABI. Plugins remain trusted native programs selected explicitly.
The public catalog describes plugins; it does not install them automatically.

## 0.6.0 — 2026-10-13

Publish to GitHub Pages from your own machine, and a publisher interface for plugins.

- **`guidedog-pages`.** A new program, in the `guidedog` archives and the Homebrew
  formula, publishes a Guidedog site to GitHub Pages (GDS 0008). It builds the project
  with the installed `guidedog` and sends the whole site to a branch of its own in one
  Git push, which GitHub applies only if the branch still has the commit that was read,
  so a publication never overwrites a newer one. It then waits for GitHub Pages to
  deploy that commit. `init`, `auth import` and `auth login`, `bind`, `setup`,
  `doctor`, `publish`, `watch`, `status`, and `recover`. Tokens stay in the macOS
  Keychain, the Windows Credential Manager, or the Linux Secret Service; a binding on
  the machine decides which repository and branch a profile may publish to.
- **`guidedog publish` and `guidedog plugin`.** Guidedog builds a project with its own
  engine and hands the frozen site to a registered publisher program, through a
  versioned JSON Lines protocol. `guidedog plugin register` pins a program's SHA-256;
  a changed program does not run until it is registered again. A normal build never
  starts a publisher.
- **`gd-plugins`.** The publisher interface of GDS 0009: publishers as typed
  interfaces, sessions, frozen and checked snapshots, plans, verified outcomes, the process
  protocol, and the machine registry. Its publisher example is a whole publisher.
- **`gd-git`.** Git objects, trees, commits, packs, and the push protocol's messages,
  without a git program.
- **`gd-fetch`** sends HTTP requests (`send`): any method, headers, streamed bodies,
  bounded answers, cancellation, and no redirects.
- The manual explains both routes to GitHub Pages, with launchd, systemd, and Task
  Scheduler examples, and the plugin chapter shows a publisher that compiles.

Interface and protocol 1.0 are a preview: they become stable once a second release has
used them unchanged. Guidedog 0.6.0 ships no GitHub app for device login; store a
fine-grained token with `guidedog-pages auth import`, or pass your own app's client ID.

## 0.2.3 — 2026-10-12

Safety fixes from the independent discussion catalog review.

- Promotion now honours an open state's evidence requirement. `promote` accepts
  `--reason` and `--evidence`, and the generated lifecycle diagram shows that requirement.
- An explicit empty list of record types is refused. Semantic configuration errors
  retain the setting's line number. Initializing with an empty vocabulary reports a
  problem instead of indexing an empty list.
- Streaming reads and whole-record edits enforce the same metadata limits, so a command
  cannot write a record the next load refuses. Line limits no longer depend on read sizes.
- Allocation refusal stops a build before an incomplete request reaches its renderer;
  it cannot return a shortened success result.
- TOML values deeper than 64 levels are refused with a diagnostic before exhausting the
  parser's stack. A failed string allocation is a parse error, even if later allocations
  succeed. Parsed values own their input so a temporary source buffer can be released safely.
- The highlighter's growth check counts scanner work as input grows. It also detects
  repeated whole-line searches, without depending on shared-runner clock timing.

## 0.2.2 — 2026-10-12

Discussion catalogs you can configure, of any size.

- **`discussion.toml`.** A catalog's lifecycle states and the moves between them, the
  state of a new draft and the state promotion opens it in, which moves need evidence,
  record types, labels, and numbering now live in one file. `dsc init` writes every
  setting with a comment, `dsc config` prints the settings in effect, and
  `dsc init --config=FILE` starts a catalog from one. A mistake names its line and the
  fix. Without the file, a catalog follows GDS 0001 as before; 0.2.1's
  `.discussion.json` is still read.
- **More than 9999 discussions.** Numbers have at least `numbering.width` digits and
  grow past it, so 10000 follows 9999, and lists sort by value. `numbering.start` and
  `numbering.limit` set where promotion begins and ends. A catalog may hold 100,000
  records.
- **Memory that does not grow with the records' text.** `dsc` keeps each record's
  metadata, about a kilobyte, and reads a record whole only to change it. A catalog of
  2,000 records of 100 KB is checked with 17 MB of memory instead of 211 MB, and as
  fast. Duplicate numbers are found in linear time.
- **A process draft that matches the catalog.** The draft `init` writes describes the
  configured lifecycle, with a table of the states and a diagram of the moves.
- **`gd-toml`.** Guidedog's TOML 1.0 reader is now its own library, used by projects
  and catalogs alike.
- **Library changes.** `Init.process` is now `Init.draft` (the process draft's text), and
  `Init.process` holds the configuration. `edit_plan` reads the record it changes from
  disk when the catalog was loaded. The memory-refusal tests found two more unchecked
  allocations, both fixed before release.

## 0.2.1 — 2026-10-12

Discussion catalogs for any project, with the programs `dsc` and `dscc`
([issue 1](https://github.com/insanai/guidedog/issues/1)).

- **`dsc` and `dscc`.** `dsc` keeps proposals and decisions as reStructuredText records
  under a prefix of your choice (`eds`, `sds`, `rfc`): `init` makes a whole catalog,
  with your own process draft and record template if you give them; `new`, `promote`,
  `state`, `index`, `check`, `build`, and `recover` work as `guidedog gds` always did.
  `dsc` comes in the Guidedog archive and builds with the installed `guidedog`; `dscc`
  is `dsc` with Guidedog built in, released as its own archive for every platform.
- **One implementation.** `guidedog gds` now runs the same commands, from the same
  discussion library. GDS records, numbers, locks, journals, and problem codes are
  unchanged. The GDS library is folded into the discussion library, whose default
  prefix is `gds`; its `replace_field` returns a problem beside the text, and
  takes only literal values, never a Typst expression.
- **Readable metadata for every catalog.** The leading `:PREFIX-number:`,
  `-title:`, `-authors:`, `-last-updated:`, `-labels:`, `-category:`, `-status:`, and
  `-summary:` fields show as Number, Title, Authors, Updated, Labels, and so on, in HTML
  and PDF, for any prefix. A page whose first fields are ordinary keeps them as written.
- **Catalog changes that are whole or nothing.** Every operation of the discussion library
  reports running out of memory as `discussion.memory` and returns nothing it made:
  a metadata edit never yields a shortened record, and an index plan never a partial
  one. `apply` refuses an empty or cut-short record, or an index without its generated
  region, before writing anything. Tests refuse every allocation of these operations in
  turn, and interrupt an update after each file, then recover it.
- **Fixes found on the way.** A metadata edit changes only its own line, so a record
  keeps its line endings. Staging files are removed even when memory has run out.

## 0.2.0 — 2026-10-11

The first public release. Guidedog builds HTML documentation sites and PDF books from
reStructuredText and Markdown, as one native program. It needs no Python
environment and no LaTeX installation. This is a beta for trusted documentation projects
on macOS and Linux.

- **Projects you already know how to write.** `guidedog quickstart`, `build`, `serve`,
  `clean`, and `migrate` give a Sphinx-inspired workflow with declarative `conf.toml`
  configuration. The builders are html, dirhtml, singlehtml, pdf, text, gettext, and dummy.
  Builds are incremental and can read on several threads (`-j auto`). Python configuration
  and extensions are never executed; full Sphinx compatibility is not a goal.
- **Guidedoc, the conversion engine.** It reads reStructuredText (with Sphinx's directives
  and roles), CommonMark, and MyST Markdown into one document model, and renders HTML,
  Typst, and text. It works in memory the caller provides and returns structured
  diagnostics instead of crashing, so other programs can embed it.
- **PDF books through Typst.** Chapters, cross-references, figures, indexes, equations,
  diagrams, and language-aware typesetting, from editable Typst templates. Typst can be an
  installed program or embedded in the executable.
- **The web edition.** Navigation, search, code highlighting with Guidedog's own
  highlighter, Graphviz diagrams, MathJax equations, light and dark themes, and Jinja
  templates you can edit.
- **Translations.** `guidedog build gettext` and `guidedog intl update` and `stat` keep PO
  catalogs in step with the sources. The manual itself is published in seven languages:
  English, Simplified Chinese, Japanese, Korean, Hindi, Spanish, and German, on the web
  and as PDF books.
- **API documentation.** Odin API pages are extracted statically from source by the Odin
  domain, and Python autodoc reads modules without importing them. Intersphinx
  inventories link projects together.
- **Builds you can trust.** A build publishes all of its output or none of it, and an
  interrupted publication is recovered on the next run. A memory budget (`--budget`,
  `--memory=disk|ram`) bounds what a build holds, and a build refuses work before
  allocating past it. Errors say what went wrong, where, and how to fix it. A crash ends
  with an explanation and a report to send, and never leaves half-written output behind.
  `--untrusted` leaves out raw content and fetches nothing.
- **Design records.** `guidedog gds` manages the numbered design discussions (GDS) that
  explain how Guidedog works and why.

Guidedog is released under the GNU Affero General Public License v3.0 or later
(AGPL-3.0-or-later). For other licensing terms, contact the authors. Created by Vikrant
Rathore, with assistance from Ronak Rathore and Kanak Rathore.
