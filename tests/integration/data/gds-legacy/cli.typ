#import "../../shared/guide.typ": frame, record, note, card, flow, down, sequence
#let gds-number = "0002"
#let gds-title = "Guidedog: CLI and documentation toolchain"
#let gds-state = "discussion"
#let gds-type = "Standards Track"
#let gds-authors = ("Guidedog contributors",)
#let gds-created = "2026-09-29"
#let gds-updated = "2026-09-29"
#let gds-discussion = "Local review; no external discussion URL assigned"
#let gds-labels = ("cli",)
#show: frame
#record(gds-number, gds-title, gds-state, gds-type, gds-authors,
  gds-created, gds-updated, gds-discussion, labels: gds-labels)

= Abstract

This record specifies the Guidedog command line: discussion management, single-document
conversion, and the documentation project builder. All three are implemented. The
executable converts reStructuredText, CommonMark, and native Typst to HTML and PDF, builds a
responsive website and a PDF book from a project, and embeds the Typst 0.15.1 compiler when
built with its bridge. Typst packages and system fonts are supported. Open items, such as
worker isolation for hostile Typst and a cross-document symbol index, are listed at the end.

= Motivation and scope

Users need one command that makes output predictably and explains failures in
terms they can act on. The CLI should share engine semantics with embedded callers,
while owning terminal behavior, files, resources, Typst, and publication. This
record separates what is implemented from what remains open; an implemented command
does not by itself establish that every acceptance gate below is met.

= Implemented specification: discussion CLI

The entry point is `cmd/guidedog/main.odin`. `lib/cli` owns argument parsing,
validation, ANSI presentation, and dispatch. `lib/gds` owns metadata, catalog
selection, lifecycle edits, journal recovery, Typst execution, and HTML presentation.
The CLI uses invocation-owned arena allocations. It does not claim the future
Guidedoc core's zero-allocation contract for filesystem or compiler orchestration.

```text
guidedog --help
guidedog --version
guidedog formats
guidedog gds new "Cache policy" --author="Your name" --labels=build
guidedog gds list --state=prediscussion
guidedog gds show cache-policy --format=json
guidedog gds promote cache-policy --dry-run
guidedog gds promote cache-policy
guidedog gds index
guidedog gds check
guidedog gds build
guidedog gds build 2 --target=pdf
guidedog gds build "Guidedoc: Odin conversion engine" --target=html
```

The available GDS commands are `new`, `list`, `show`, `promote`, `state`, `index`,
`check`, `build`, and `recover`. Their lifecycle is defined by GDS 0001.
Selectors accept a number, exact slug, full title, or source filename. Ambiguous
selectors report candidates. Multiple record-selection options are rejected.
`--dir` selects the discussion root; the default is `docs/gds`.

Metadata consists of literal strings or tuples in a single-line `#let gds-*`
preamble. Listing and lifecycle edits do not execute Typst. The local template
supplies new drafts. Authors come from `--author` or Git's configured user name.
Guidedog numbering is independent of zenfmt: 0001 is the process, 0002 is this
record, and 0003 is the next available assignment in this checkout.

Generated dates use the machine's local timezone through Odin's timezone package,
with UTC as a fallback when timezone data is unavailable. An edit preserves a later
existing update date rather than moving it backward. Invalid calendar dates and
dates out of order receive distinct diagnostics with a repair direction.

Mutation commands support dry-run and use a kernel lock while loading and updating
records. The journal stores original and intended contents; recovery compares those
snapshots and refuses to overwrite later edits. The lock uses Windows `LockFileEx`
or POSIX locking on Linux and macOS. Closing the handle releases it, including on
process exit. The ignored lock file may remain; its existence does not mean busy.

#figure(
  flow((
    ([Arguments], [Parse command, options, selector, and output policy]),
    ([Catalog], [Read literal metadata; validate identity and lifecycle]),
    ([Mutation], [Lock → plan → journal → replace files → remove journal]),
    ([Build], [Resolve Typst → stage artifact → theme HTML → replace output]),
  )),
  caption: [Mutation and build are alternative dispatch paths after catalog loading.],
)

== HTML catalog and individual PDFs

`gds build` defaults to `--target=both`. With no selector it compiles every active
record. A selector compiles only that record. Outputs go to `docs/build/`, beside
the selected GDS directory, and are ignored by Git in this repository. Filenames
are `gds-NNNN-slug.html` and `gds-NNNN-slug.pdf`; drafts retain `XXXXX`.
There is no PDF index and no combined PDF.

Every HTML build also refreshes `index.html` from the full catalog. Records appear
even if they have no artifact yet; links are emitted only for existing artifacts.
The HTML theme embeds CSS and JavaScript, section navigation, a section filter,
light/dark controls, responsive layout, and scrollable tables. Diagram helpers
emit SVG inside semantic HTML. These are discussion pages, not the future general
documentation-project theme or project-wide search implementation.

The installed Typst 0.15.1 command is the development backend. `--typst` overrides
PATH lookup; Windows lookup handles semicolon-separated PATH and `.exe` names.
The CLI passes an argument vector directly, including paths containing spaces.
HTML compilation enables Typst's experimental HTML feature. It requires a trusted
source and installed compiler; this process is not an untrusted-document sandbox.

#figure(
  sequence(([CLI], [Catalog], [Typst process], [Output directory]), (
    (0, 1, [1. Validate and select records]),
    (1, 0, [2. Selected source paths]),
    (0, 2, [3. Compile each target into a temporary file]),
    (2, 0, [4. Artifact or compiler diagnostics]),
    (0, 3, [5. Theme HTML and replace the successful artifact]),
    (0, 3, [6. Refresh HTML index after an HTML build]),
  )),
  caption: [PDF-only builds omit step 6. A failed artifact keeps its previous output.],
)

Successful rebuilds replace existing artifacts without `--force`. Each artifact
is staged separately. A failed full build may have replaced earlier records before
a later record fails; it is not a transaction over the whole output directory.
Renamed records can leave obsolete artifacts that require explicit cleanup.

== Current diagnostics and limits

Current host failures use `Problem`: code, title, path, message, hint, and exit code.
Text reports explain the failure and a suggested repair. `--diagnostics=json`
serializes those fields without ANSI; `--format=json` selects structured catalog
or preview output. Conversion diagnostics are versioned JSON lines (`schema: 1`).
`--version` reports Guidedog, the Guidedoc schema, the Odin compiler, and whether Typst is
embedded or external; `formats` reports each adapter's actual conformance.

Metadata inputs are bounded to 1 MiB per record and 4,096 directory entries.
Recovery journals are bounded to 32 MiB; HTML processing allows 64 MiB per file.
These host limits are distinct from the proposed engine budgets below.
The implementation returns exits 0 through 5 and 130: Ctrl+C sets a flag through C's
`signal`, which Windows, macOS, and Linux share, and the command stops without publishing.
Compiler resource isolation remains future work.

`check` validates metadata, identities, dates, required abstract/reference sections, and
generated index agreement. Records are prose, so no line or length limit applies. `check --render`
also builds artifacts. It does not resolve prose cross-references. Promotion updates
managed registry and bundle references; authors must repair prose links to a renamed
draft. This limitation remains visible until reference-aware updates are implemented.

= Specification: conversion command-line behavior

The first executable is named `guidedog`; `convert` exposes Guidedoc before the
project builder exists. This avoids shipping a second CLI name that later needs
renaming. `convert` reports each reader's conformance through `formats`: CommonMark passes
all 652 examples of 0.31.2; the reStructuredText reader is partial, with its gaps listed in
the readers' coverage ledgers.

```text
guidedog --help
guidedog --version
guidedog formats
guidedog convert guide.rst --to html --output guide.html
guidedog convert guide.md --to pdf --output guide.pdf
guidedog convert guide.typ --to pdf --output guide.pdf
guidedog convert - --from commonmark --to html --output -
```

`.md` selects CommonMark. `.rst` selects reStructuredText. `.typ` selects native
Typst. An explicit `--from` overrides suffix selection; ambiguous or missing
selection produces a direction to specify it. No heuristic claims that arbitrary
plain text is reliably distinguishable as RST or Markdown. `--to` accepts only
`html` or `pdf`; if omitted, the output suffix may select the target. Intermediate
Typst is available through `--emit-typst path`, not as a third product target.

`formats` reports actual adapter availability, conformance status, and toolchain
version. It must distinguish planned, partial, and conformant readers. Global
flags include `--color=auto|always|never`, `--diagnostics=text|json`, and `--quiet`.
Use `--` to terminate option parsing. Never evaluate a shell command from argv.

== ANSI belongs at the terminal boundary

Use the installed Odin package `core:terminal/ansi`. Its constants are escape-code
components, so a colored heading combines `ansi.CSI`, a color, and `ansi.SGR`.
The terminal renderer is the only package allowed to emit these sequences.

```odin
import "core:terminal/ansi"

error_style :: ansi.CSI + ansi.FG_RED + ansi.SGR
reset_style :: ansi.CSI + ansi.RESET + ansi.SGR
```

Automatic color requires a terminal on the diagnostics stream, no `NO_COLOR`, and
a terminal other than `dumb`. Explicit `--color` takes precedence. JSON and artifact
bytes never contain ANSI styling. Diagnostic source excerpts escape terminal control
characters, including embedded escape sequences from filenames or source text.
Honor redirected stderr, narrow terminals, tabs, combining marks, and Unicode.
Color supplements words and markers; it never carries meaning by itself.

stdout contains requested artifact bytes or machine-readable command output.
stderr contains diagnostics. In JSON mode stderr is a versioned JSON-lines report
stream. Help and version use stdout on success. Version output includes compiler,
Guidedoc schema, and Typst backend versions when applicable.

#table(
  columns: (auto, 1fr),
  table.header([*Exit*], [*Meaning*]),
  [0], [Completed successfully; warnings may exist],
  [1], [Invalid document, unresolved references, or unsupported target content],
  [2], [Command syntax or option error],
  [3], [Capacity or configured resource limit exceeded],
  [4], [Host I/O or Typst backend failure],
  [5], [Internal invariant failure; request a reproducible report],
  [130], [Cancelled by the user],
)

== Elm-style diagnostics are structured data

Every warning and error answers four questions: what happened, where, what this
means for the result, and what to do next. Prefer a specific edit or an exact flag
to advice such as “check your input.” The library returns facts; renderers choose
prose layout, color, and machine-readable encoding.

A `Diagnostic` holds stable namespaced code, severity, source ID and byte span,
problem kind, bounded typed arguments, consequence, and one or more directions.
Directions contain a suggested replacement, a complete argv vector, or a named
API field and value. Source text remains borrowed. Fixed inline numeric fields and
static message templates allow capacity errors to be reported when report storage
is itself full. `Result.primary` is always available by value on failure.

```text
UNRESOLVED REFERENCE                         rst.reference.unresolved

  guide.rst:18: See installation_ before continuing.
                     ^^^^^^^^^^^^^

I could not find a target named "installation".
The HTML page has not been published.

Add a target beside the installation section:

  .. _installation:

Or change this reference to an existing target name.
```

Rendering calculates line and display column from byte offsets and a bounded line
index. A report without source context uses an argv span, resource name, or workspace
region. Reports never invent a source position. JSON retains the same code, evidence,
consequence, and directions as the text view.

When report storage fills, retain the first failure in `primary`, count suppressed
reports, and set an explicit truncation flag. In strict reporting mode, truncation
fails the conversion. A bounded excerpt avoids copying a huge source line into the
report. The host renderer may use ordinary allocations; a bounded-buffer renderer
must remain available for allocation-free embedders.

== CLI resource policy

_Superseded in part_ by the draft "Memory budgets and what a build may read": how a
build's budget is reserved and shared (one budget per build, `--budget`), what it counts
and what it names as outside it, and which files a build, a graph, and Typst may read,
trusted and under `--untrusted`. The limits below stand.

Initial defaults are proposed, not measured performance claims: 32 MiB per source,
256 MiB total prepared Odin storage, 256 nesting levels, 1,024 stored reports,
1,000,000 AST nodes, and 128 MiB output. Core receives limits explicitly; the CLI
selects them. A separate project budget bounds aggregate files and resources.
Each exhausted limit names the corresponding flag or API field and a concrete
next value when computable. Byte and node capacities are checked independently.

A cancellation probe is checked at stage boundaries and at bounded work intervals.
Compressed archives, network retrieval, and office formats are outside this release.
Local paths are confined to approved roots with symlink-aware checks. Include and
substitution expansion have depth, total-byte, and work limits.

= Guidedog: the project builder

#note("Superseded")[The draft record "Guidedog: everything Sphinx does, the Odin way"
(`XXXXX-sphinx-replacement.typ`) replaces this section's project builder: the
`guidedog.toml` layout, `init`, and `build --target`. That record is the current statement
of the project layout, `conf.toml`, builders, incremental builds, and publication. What
this section says about the build cache, publication, confinement, and the themes still
holds and is kept for its history; where the two differ, the draft governs.]

The full application adds project discovery, document ordering, cross-references,
templates, incremental builds, local preview, and publication. It reuses Guidedoc
for RST and CommonMark and the Typst adapter for native Typst content.

```text
guidedog quickstart docs
guidedog build html docs
guidedog build pdf docs
guidedog serve docs --port 8000
```

These commands are implemented in `lib/project` and `lib/cli/project.odin`; the
superseded `guidedog init manual --template atlas` and `guidedog build --target html`
forms no longer exist.

Configuration is declarative TOML: source roots, entry document, navigation,
targets, language, theme tokens, allowed resources, and build budgets. It is not
executable Python. Built-in assets and templates are embedded into the executable.
User overrides are explicit files in the project.

== Project graph and stable names

Discovery sorts normalized project-relative paths. Parsing collects document IDs,
heading targets, named RST targets, citations, includes, and resource dependencies.
Resolution constructs a deterministic symbol index and resolves cross-file edges
before rendering. Duplicate IDs and cycles are diagnosed with both source sites.
URLs are stable project IDs plus anchors, never ephemeral node indices.

For RST and CommonMark, the project host supplies read-only symbol and resource
views. Conversion does not open neighboring files. Mixed-source books generate
Typst chapter modules for RST and CommonMark and import native Typst chapter modules
through a documented content-export convention. An arbitrary standalone `.typ`
file can always be built alone; joining it into a book may require adapting page
rules and exports. Never promise automatic merging of unrelated Typst programs.

Native Typst chapters declare navigation and exported cross-reference labels in a
small project metadata contract. The adapter resolves these against the shared
project manifest. It does not guess arbitrary runtime labels from source tokens.

*Implemented today:* discovery, duplicate-page diagnostics, a cross-file symbol index of
labels, terms, domain objects, and documents, and a link pass that resolves references
through it: to page URLs for the site, to chapter labels for the book, and to
`#<document key>-<anchor>` in `singlehtml`. Native `.typ` documents become site pages
through Typst's HTML export and book chapters through `#include`. The native Typst
metadata contract above is not implemented.

A build-cache key includes source and transitive dependency digests, reader version,
configuration, renderer version, template, fonts, and Typst version; the implementation
also includes the executable's size and modification time, so a rebuilt `guidedog` never
reuses an older conversion. A page's key also covers what its references resolved to:
the link pass records every lookup it makes, and the next build repeats them. Diagnostics
are cached with the output they came from and reported again when it is kept. Development
caches are host-owned and may allocate. A clean build and a cached build must emit
the same artifacts for the same inputs, excluding explicitly requested timestamps.

== Responsive HTML and the Typst book

The first HTML theme, proposed name *Atlas*, has a restrained neutral palette,
a clear accent color, a readable article measure, and generous spacing. Desktop
layout includes project navigation, article, and an on-page outline. On narrow
screens, navigation becomes an accessible drawer and the article remains readable
at 320 CSS pixels. Wide tables and code scroll within their containers.

Require semantic landmarks, skip links, keyboard navigation, visible focus, proper
heading order, image alternatives, high contrast, and reduced-motion support.
The page remains readable without JavaScript. Search uses a generated local index;
no remote service is required. CSS, fonts, and icons ship with the binary, with
license notices. Native Typst HTML is integrated through its supported export and
template mechanisms; do not concatenate entire HTML documents into a page shell.

The PDF theme, proposed name *Folio*, is a Typst book template: title page, contents,
numbered chapters, running headings, readable body type, restrained code styling,
figures, tables, notes, citations, and useful page breaks. Test A4 and Letter,
long URLs, wide tables, multilingual text, and embedded fonts. User content is
passed as content and data, with a small documented theme-token surface.

Atlas and Folio are implemented and embedded. Desktop and phone-width pages and PDF
pages were reviewed as rendered images; a formal accessibility audit remains open.
A project `pdf.preamble` file adds package imports and show rules to the book.

== Typst and the single executable

Typst's published compiler library is Rust. The selected design is a small Rust
adapter exposing a versioned C ABI, statically linked into the Odin application.
The conversion engine remains Odin. Release builds must prove that the linked
binary needs neither an installed `typst` command nor a downloaded package or font.
This is an integration decision to validate, not an existing upstream C API.

The ABI accepts pointer-and-length source and resource records, explicit target,
limits, and diagnostic callbacks. Rust owns its compiler state and output until
an explicit release call; no Rust object layout crosses the ABI. Copying into
caller buffers is explicit. Convert panics into failures at the boundary where
supported; never unwind through Odin frames. Allocation failure and hard process
termination cannot be promised recoverable merely by catching panics.

The adapter implements Typst's resource environment. Project files are read only
under the Typst root. *Typst packages are supported*, because publication-quality PDF
depends on them and Guidedog writes Typst rather than LaTeX: `@preview` and `@local`
imports resolve like the typst CLI, from the package data directory, then the cache,
then a Typst Universe download into the cache. A package reads only inside its own
directory and has no shell or network access of its own; that is Typst's sandbox, and
it makes a package no riskier than a LaTeX package. Versions are pinned by the import
itself (`@preview/name:1.2.3`). `--offline` (or `pdf.packages = "offline"`) restricts
builds to packages already on disk, for CI and air-gapped machines.

*Fonts* also follow the typst CLI: extra `--font-path` directories first, then system
fonts, then the fonts embedded in the executable as a fallback, so a book can use the
fonts installed on the machine. `--embedded-fonts-only` (or `pdf.fonts = "embedded"`)
ignores system fonts for byte-reproducible output across machines. Trusted templates
are embedded. No other filesystem or network access is delegated to document content.
For hard time or memory isolation, Guidedog can invoke its own executable in an
internal worker mode and apply OS limits; there is still one distributed binary.
Per-platform containment and cancellation must be verified before hostile native
Typst is advertised as safely isolated. An in-process compiler is not a sandbox.

The bridge exists in `native/typst_bridge`: ABI version 2, one request carrying root,
main file or generated text, target, date, package mode, cache, font mode, and font
paths; one owned result released by `gd_typst_free`; panics become an internal-failure
status. It builds offline from the cached crates on macOS; Linux vendors OpenSSL
statically so the binary depends only on the C runtime. Release builds pass
`-define:GUIDEDOG_TYPST_BRIDGE=true`; the default build keeps the development backend.

A development adapter may invoke an installed, pinned Typst CLI with an argv array.
It is labeled a development backend and does not satisfy the single-binary release
gate. Static-link feasibility, binary size, licensing, supported platforms, HTML
export fidelity, and worker containment are explicit early technical checkpoints.

#figure(
  sequence(([Guidedog], [Guidedoc], [Typst worker], [Publisher]), (
    (0, 1, [1. RST / CommonMark + resources]),
    (1, 0, [2. Generated Typst + source map]),
    (0, 2, [3. Compile Typst to PDF]),
    (2, 0, [4. PDF or mapped diagnostics]),
    (0, 3, [5. Stage validated artifacts]),
    (3, 0, [6. Commit or report failure]),
  )),
  caption: [Native .typ input starts at step 3. Failed compilation never reaches step 5.],
)

== Publication is a host transaction

Conversion to a path writes a sibling temporary file, flushes and closes it, then
replaces the destination using the platform's supported rename operation. Refuse
existing output unless `--force` is explicit. stdout output cannot be rolled back;
finish conversion into staging memory before emitting it.

A website or book with resources is a set of files, so two individual renames do
not form an atomic transaction. Stage an entire versioned build directory and
switch an application-owned current-version pointer where supported. On platforms
without an atomic switch, preserve the previous build and document recovery.
An optional provenance manifest records versions, digests, diagnostics, and resources;
it belongs to the same staged set. Crash durability requires directory sync behavior
where available and is tested separately from logical all-or-nothing publication.

*Superseded for project builds* by the Publication section of the draft
`XXXXX-sphinx-replacement`, which builds the whole next output folder beside the live
one and switches the two (in one step where the system can), journals the switch so a
failure is undone and a crash finished by the next build, publishes the environment
with the output, and decides `-W` before publishing. The decision below, whose
file-by-file moves could not survive a file that becomes a folder, is kept as history.

*Implemented decision.* A project build stages each file it writes in a folder beside
the output folder, and a file whose bytes are already published is only recorded. When
every unit of the build succeeded, the staged files are renamed into place, the files
only the previous generation made are removed, and the manifest is saved last. A failed
or interrupted build removes the staging folder; the published output is untouched. The
switch is file by file rather than one pointer: the output folder stays the path users
serve and preview, and unchanged files keep their times. A crash while files are being
moved leaves the old manifest, so the next build rewrites exactly the pages that differ.
== Delivery phases

#figure(
  flow((
    ([0 · Specification], [Review contracts and resolve integration questions]),
    ([1 · Odin CLI], [Odin skeleton, ANSI reports, help, version, GDS management]),
    ([2 · Core vertical slice], [Fixed SoA AST; CommonMark → HTML and Typst → PDF]),
    ([3 · Full language coverage], [Complete CommonMark and full RST; native Typst routes]),
    ([4 · Guidedog project], [References, dependency graph, responsive HTML, book theme]),
    ([5 · Release qualification], [One binary, failure recovery, conformance and resource audits]),
  )),
  caption: [Each phase proves behavior before adding a larger surface. No phase promises speed.],
)

Phase 2 first uses a development Typst backend while a separate feasibility check
validates static linkage. Its CommonMark slice is explicitly partial until the full
suite passes. Phase 3 cannot close with a partial RST parser marketed as complete.
The release gate requires all declared routes to work from a clean machine without
Python or a Typst installation, and without a network download unless a document imports a
Typst package that is not cached. Optimization follows working behavior and measured
evidence, not an earlier calendar milestone.

Status on 29 September 2026: phases 1 and 2 are complete. Phase 3 is complete for
CommonMark and native Typst and partial for RST, as its ledgers record. Phase 4 is
implemented except the native Typst chapter contract.
Phase 5's single binary works on macOS and Linux; Windows runtime, worker isolation, and
font-license review remain.


= Security considerations

Source content, terminal excerpts, include paths, and generated Typst are distinct
trust boundaries. The rules above require escaping, approved resource roots,
explicit raw-content policy, and isolated Typst execution when hard bounds matter.
No document may acquire filesystem or network authority by naming a resource.
The single-binary packaging requirement is not itself a sandbox guarantee.

= Backwards compatibility

The CLI, conversion, and project commands described above are available in this checkout.
The diagnostics JSON schema is version 1; changes to it require a new schema version.
The project does not emulate every Sphinx extension or configuration
script. Full RST support and Sphinx-specific extension support are separate claims.
Existing source files must receive useful diagnostics when policy prevents a feature.

= Alternatives considered

A separate converter executable would duplicate the future Guidedog command surface.
An installed Typst command is useful during development but fails the single-binary
release requirement. Reimplementing Typst in Odin would replace a mature compiler
with an unrelated language implementation. A narrow static bridge preserves the
Odin application and reuses the real compiler. Mandatory sidecars for every byte
conversion would improperly move host policy into the core.

= Discussion management

`guidedog gds` is part of the first CLI milestone, following docodin's integrated
DOD commands and the inherited GDS process. The CLI implements `new`, `promote`, `list`,
`show`, `state`, `index`, `check`, `build`, and `recover` according to GDS 0001.
GDS HTML/PDF builds use the embedded Typst compiler when it is linked, and otherwise the
development backend. Both this
repository and documentation projects can select a GDS root with `--dir`.

= Acceptance criteria and reference implementation

The CLI must pass help, usage, option precedence, redirection, JSON, ANSI, exit-code, and
report-overflow tests. The project builder must pass the engine discussion's conformance
and resource tests, the publication failure tests above, and visual review of HTML and PDF
templates. Single-binary qualification requires a clean machine with no Python or installed
Typst.

== Evidence

The reference implementation is `cmd/guidedog` with `lib/cli`, `lib/gds`, `lib/host`,
`lib/typst`, `lib/project`, `lib/defaults`, the Guidedoc packages of GDS 0003, and
`native/typst_bridge`. On 29 September 2026:

- `tests/` holds 137 end-to-end tests (30 September 2026), among them: discussion workflows, recovery, dates,
  Typst builds;
  conversion to HTML and PDF from RST, CommonMark, and Typst; overwrite refusal; JSON and
  text diagnostics; include confinement; offline package resolution from a private cache;
  project init, build, incremental rebuild, failed-build preservation, and configuration
  errors; the preview server; long headings; standard-input includes; exact and
  one-short capacity for every storage region; memory budgets; publication of whole
  generations; diagnostics that do not depend on earlier builds; dependencies on tags,
  nested templates, and moved objects; `singlehtml` addressing; link confinement; and
  `--untrusted`.
- Package suites (test procedures): core 18, host 5, project 18, i18n 28, gettext 31,
  jinja 46, highlight 64, graphdog 22, inventory 4, fetch 1, CommonMark 22 (all 652
  examples), RST blocks 208, RST inline and resolution 65, Sphinx 73, HTML 28, Typst 24,
  text 13. Each includes a zero-allocation test
  where the package is inside the core boundary.
- Every suite passes on macOS arm64 and Linux x86_64, with the installed and the embedded
  Typst backends. The embedded executable runs with no `typst` on PATH: 64 MB on macOS,
  80 MB on Linux, where it depends only on the C runtime. Windows type-checks with and
  without the bridge.
- Ctrl+C during a PDF build exits 130, leaves no staged file, and publishes nothing.
- `tools/check` passes every source file. A 460 KB, 4,000-section Markdown file converts to
  HTML in 0.07 s at 33 MB peak memory. Each document and page is converted in its own
  arena, so a 1,500-document site builds at 15 MB peak. These are observations, not
  performance claims; every page carries the full navigation, so site size grows with the
  square of the page count.
- An independent review of the host packages found eleven defects, from anchor sizing to
  memory growth, output validation, and terminal escaping; all are fixed with tests.

```text
odin build cmd/guidedog -out:build/guidedog
cargo build --release --manifest-path native/typst_bridge/Cargo.toml
odin build cmd/guidedog -define:GUIDEDOG_TYPST_BRIDGE=true -out:build/guidedog
odin run tools/check
odin test tests -out:build/guidedog-tests
```

This record remains in `discussion`. The open items below keep it short of `committed`;
the lifecycle decision belongs to maintainers.

= Decisions still requiring implementation evidence

First, worker isolation: hard time and memory limits for hostile native Typst need the
worker mode and per-platform verification. Second, the native Typst chapter contract.
Third, the RST ledger's recorded gaps. Fourth,
Windows runtime validation, embedded font licenses, and packaging. These are visible
acceptance risks, not reasons to quietly reduce the supported language specification.

= Review history

29 September 2026: separates terminal and host policy from the allocation-free
engine; specifies full source-language scope, Typst PDF routes, and later project
features. The initial revision preceded the implementation.

29 September 2026, implementation review: record the working GDS CLI, HTML catalog,
individual PDFs, journal recovery, local dates, and portable host boundaries.
Correct the inherited assignment from 0017 to Guidedog's 0002; zenfmt numbering
does not reserve GDS numbers. Record the 18-test evidence and unresolved acceptance
gates. Preserve discussion status because the full record is not yet implemented.

29 September 2026, implementation of conversion and projects: record `convert`, `init`,
`build`, and `serve`; the embedded Typst bridge; Typst packages and system fonts at the
user's direction, replacing offline-only packages and embedded-only fonts; Ctrl+C
handling; Linux runtime evidence; and the remaining open items.

29 September 2026, review of the build pipeline: record whole-generation publication with a
manifest, recorded lookups and replayed diagnostics, session memory budgets reserved before
allocation and shared by reading threads, confinement of linked files, `--untrusted`, and
the current test evidence. The cross-file symbol index is implemented.

= References

- GDS 0003, Guidedoc: Odin conversion engine.
- GDS 0001, The Guidedog Discussion Process.
- #link("https://pkg.odin-lang.org/core/terminal/ansi/")[Odin ANSI package].
- #link("https://docs.rs/typst/0.15.1/typst/")[Typst compiler crate 0.15.1].
- #link("https://typst.app/docs/reference/html/")[Typst HTML export].

// Lifecycle event recorded by guidedog.

#par()[#text(
  "" +
  "2026-09-29: prediscussion → discus" +
  "sion. Assigned a permanent number an" +
  "d opened for discussion. "
)]
