:gds-number: 0002
:gds-title: Guidedog: CLI and documentation toolchain
:gds-state: published
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-09-29
:gds-updated: 2026-10-15
:gds-discussion: Local review; no external discussion URL assigned
:gds-labels: ["cli"]

Guidedog: CLI and documentation toolchain
=========================================


Abstract
--------


This record specifies the Guidedog command line: discussion management, single-document
conversion, and the documentation project builder. All three are implemented. The
executable converts reStructuredText, CommonMark, and native Typst to HTML and PDF, builds a
responsive website and a PDF book from a project, and embeds the Typst 0.15.1 compiler. Typst packages and system fonts are supported. Open items, such as
worker isolation for hostile Typst and native Typst chapter modules, are listed at the end.


Motivation and scope
--------------------


Users need one command that makes output predictably and explains failures in
terms they can act on. The CLI should share engine semantics with embedded callers,
while owning terminal behavior, files, resources, Typst, and publication. This
record separates what is implemented from what remains open. An implemented command
does not by itself establish that every acceptance gate below is met.


Implemented specification: discussion CLI
-----------------------------------------

``gd-cli`` owns arguments, validation, terminal presentation, and dispatch.
``gd-discussion`` owns record metadata, catalog selection, lifecycle plans, and
recovery; GDS 0007 describes its catalogs and its plugin, which adds the ``dsc`` commands.
RST publication delegates to ``gd-project``. Guidedoc reads and renders the prose.
Typst sets PDF pages. RST HTML does not require a Typst installation.

.. code-block:: sh

   guidedog dsc new "Cache policy" --author="Your name" --labels=build
   guidedog dsc list --state=prediscussion
   guidedog dsc show cache-policy --format=json
   guidedog dsc promote cache-policy --dry-run
   guidedog dsc index
   guidedog dsc check
   guidedog dsc build --target=both
   guidedog dsc build guidedoc-engine --target=pdf

A selector is a number, exact slug, full title, or source filename.
Ambiguous selectors return candidates. ``--dir`` selects the discussion project.
The default is the catalog the current folder is in.

Metadata is an initial RST field list. Scalar values are plain text.
Authors and labels are JSON string arrays. Listing and editing never execute code.
The local RST template supplies new drafts. An author comes from ``--author``
or Git's configured name. Missing authors receive a correction direction.

Mutations are separate from builds. A mutation holds the kernel lock while it
plans and applies changes. The recovery journal records original and intended text.
Rollback refuses to overwrite an edit made after the interrupted operation.
Closing the lock handle releases the lock. A leftover lock filename is not a busy lock.

.. graphviz::
   :caption: Metadata edits and publication use different transactions.
   :alt: Catalog validation branches to a journaled edit or to the project builder.

   digraph dispatch {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f"];
     edge [color="#216553"];
     catalog [label="Load and validate catalog"];
     edit [label="Edit path\nlock, plan, journal, replace"];
     build [label="Publication path\nproject engine, staged generation"];
     catalog -> edit; catalog -> build;
   }

HTML builds publish a complete site in the discussion project's ``_build/html``.
The catalog, search, labels, and cross-document links use the normal project graph.
A selected build still refreshes that coherent site.
Selected PDFs use a separate generation in ``_build/selected/<artifact>/pdf``.
They do not replace the complete catalog in ``_build/pdf``.
PDF builds create one book per selected active record in ``_build/pdf``.
No selector means all active records. There is no combined PDF by default.
A normal ``guidedog build html`` or ``build pdf`` in ``docs/gds`` works too.

The project engine owns cache validity and generation publication.
Compiler or renderer failure preserves the previous target generation.
Each HTML or PDF target commits independently. ``both`` is not a transaction
covering both target directories.

Source and metadata limits remain distinct from compiler costs.
A record is limited to 1 MiB. Directory entries are limited to 4,096.
The recovery journal is limited to 32 MiB. Documentation prose is exempt from
source formatting limits. The external Typst compiler's allocations are not
bounded by the host's budget.

Legacy Typst catalogs keep their old metadata and publication contracts.
This RST project has no need for Typst's experimental HTML exporter.
The source-format change does not promote this record to an accepted state.

Specification: conversion command-line behavior
-----------------------------------------------


The first executable is named ``guidedog``. ``convert`` exposes Guidedoc before the
project builder exists. This avoids shipping a second CLI name that later needs
renaming. ``convert`` reports each reader's conformance through ``formats``. CommonMark passes
all 652 examples of 0.31.2. The reStructuredText reader is partial, with its gaps listed in
the readers' coverage ledgers.

.. code-block:: text

   guidedog --help
   guidedog --version
   guidedog formats
   guidedog convert guide.rst --to html --output guide.html
   guidedog convert guide.md --to pdf --output guide.pdf
   guidedog convert guide.typ --to pdf --output guide.pdf
   guidedog convert - --from commonmark --to html --output -


``.md`` selects CommonMark. ``.rst`` selects reStructuredText. ``.typ`` selects native
Typst. An explicit ``--from`` overrides suffix selection. An ambiguous or missing
selection produces a direction to specify it. No heuristic claims that arbitrary
plain text is reliably distinguishable as RST or Markdown. ``--to`` accepts only
``html`` or ``pdf``. If it is omitted, the output suffix may select the target. Intermediate
Typst is available through ``--emit-typst path``, not as a third product target.

``formats`` reports actual adapter availability, conformance status, and toolchain
version. It must distinguish planned, partial, and conformant readers. Global
flags include ``--color=auto|always|never``, ``--diagnostics=text|json``, and ``--quiet``.
Use ``--`` to terminate option parsing. Never evaluate a shell command from argv.


ANSI belongs at the terminal boundary
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


``gd_report::ansi`` holds the style codes, and ``write_styled`` wraps text in one of them
only when color is on. The terminal renderer is the only module allowed to emit these
sequences.

.. code-block:: rust

   use gd_report::ansi;

   let title = ansi::styled("ERROR", ansi::BOLD_RED, color);


Automatic color requires a terminal on the diagnostics stream, no ``NO_COLOR``, and
a terminal other than ``dumb``. Explicit ``--color`` takes precedence. JSON and artifact
bytes never contain ANSI styling. Diagnostic source excerpts escape terminal control
characters, including embedded escape sequences from filenames or source text.
Honor redirected stderr, narrow terminals, tabs, combining marks, and Unicode.
Color supplements words and markers; it never carries meaning by itself.

stdout contains requested artifact bytes or machine-readable command output.
stderr contains diagnostics. In JSON mode stderr is a versioned JSON-lines report
stream. Help and version use stdout on success. Version output includes compiler,
Guidedoc schema, and Typst backend versions when applicable.


.. list-table::
   :header-rows: 1
   :widths: 1 1

   * - **Exit**
     - **Meaning**
   * - 0
     - Completed successfully; warnings may exist
   * - 1
     - Invalid document, unresolved references, or unsupported target content
   * - 2
     - Command syntax or option error
   * - 3
     - Capacity or configured resource limit exceeded
   * - 4
     - Host I/O or Typst backend failure
   * - 5
     - Internal invariant failure; request a reproducible report
   * - 70
     - Crash: a panic, reported with its cause by the crash reporter
   * - 130
     - Cancelled by the user


Elm-style diagnostics are structured data
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


Every warning and error answers four questions: what happened, where, what this
means for the result, and what to do next. Prefer a specific edit or an exact flag
to advice such as "check your input." The library returns facts. Renderers choose
prose layout, color, and machine-readable encoding.

A ``Diagnostic`` holds stable namespaced code, severity, source ID and byte span,
problem kind, bounded typed arguments, consequence, and one or more directions.
Directions contain a suggested replacement, a complete argv vector, or a named
API field and value. Source text remains borrowed. Fixed inline numeric fields and
static message templates allow capacity errors to be reported when report storage
is itself full. ``Diagnostics::primary`` returns the first error on failure.

.. code-block:: text

   UNRESOLVED REFERENCE                         rst.reference.unresolved
   
     guide.rst:18: See installation_ before continuing.
                        ^^^^^^^^^^^^^
   
   I could not find a target named "installation".
   The HTML page has not been published.
   
   Add a target beside the installation section:
   
     .. _installation:
   
   Or change this reference to an existing target name.


Rendering calculates line and display column from byte offsets and a bounded line
index. A report without source context uses an argv span, resource name, or workspace
region. Reports never invent a source position. JSON retains the same code, evidence,
consequence, and directions as the text view.

When report storage fills, retain the first failure in ``primary``, count suppressed
reports, and set an explicit truncation flag. In strict reporting mode, truncation
fails the conversion. A bounded excerpt avoids copying a huge source line into the
report. Libraries return ``gd_report::Report`` values and never print. ``render_text``
lays them out for a terminal, escaping text with ``escape_terminal``, and ``render_json``
writes the same facts as JSON.


CLI resource policy
~~~~~~~~~~~~~~~~~~~


*Superseded in part* by the draft "Memory budgets and what a build may read". That draft decides how a
build's budget is reserved and shared (one budget per build, ``--budget``), what it counts
and what it names as outside it, and which files a build, a graph, and Typst may read,
trusted and under ``--untrusted``. The limits below stand.

The defaults are ``guidedoc::default_limits``, not measured performance claims: 32 MiB of
included text, 256 MiB of text, 256 nesting levels, 256 stored reports, 1,000,000 AST
nodes, and 1 GiB of output. Core receives limits explicitly. The CLI
selects them. A separate project budget bounds aggregate files and resources.
Each exhausted limit names the corresponding flag or API field and a concrete
next value when computable. Byte and node capacities are checked independently.

A cancellation probe is checked at stage boundaries and at bounded work intervals.
Compressed archives, network retrieval, and office formats are outside this release.
Local paths are confined to approved roots with symlink-aware checks. Include and
substitution expansion have depth, total-byte, and work limits.


Guidedog: the project builder
-----------------------------


.. note::

   The record "Sphinx-style projects and recoverable publication"
   (``0006-sphinx-replacement.rst``) replaces this section's project builder (the
   ``guidedog.toml`` layout, ``init``, and ``build --target``). That record is the current statement
   of the project layout, ``conf.toml``, builders, incremental builds, and publication. What
   this section says about the build cache, publication, confinement, and the themes still
   holds. Where the two differ, GDS 0006 governs.


The full application adds project discovery, document ordering, cross-references,
templates, incremental builds, local preview, and publication. It reuses Guidedoc
for RST and CommonMark and the Typst adapter for native Typst content.

.. code-block:: text

   guidedog quickstart docs
   guidedog build html docs
   guidedog build pdf docs
   guidedog serve docs --port 8000


These commands are implemented in ``gd-project`` and ``gd-cli``.

Configuration is declarative TOML: source roots, entry document, navigation,
targets, language, theme tokens, allowed resources, and build budgets. It is not
executable Python. Built-in assets and templates are embedded into the executable.
User overrides are explicit files in the project.


Project graph and stable names
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


Discovery sorts normalized project-relative paths. Parsing collects document IDs,
heading targets, named RST targets, citations, includes, and resource dependencies.
Resolution constructs a deterministic symbol index and resolves cross-file edges
before rendering. Duplicate IDs and cycles are diagnosed with both source sites.
URLs are stable project IDs plus anchors, never ephemeral node indices.

For RST and CommonMark, the project host supplies read-only symbol and resource
views. Conversion does not open neighboring files. Mixed-source books generate
Typst chapter modules for RST and CommonMark and import native Typst chapter modules
through a documented content-export convention. An arbitrary standalone ``.typ``
file can always be built alone. Joining it into a book may require adapting page
rules and exports. Never promise automatic merging of unrelated Typst programs.

Native Typst chapters declare navigation and exported cross-reference labels in a
small project metadata contract. The adapter resolves these against the shared
project manifest. It does not guess arbitrary runtime labels from source tokens.

**Implemented today:** discovery, duplicate-page diagnostics, a cross-file symbol index of
labels, terms, domain objects, and documents, and a link pass that resolves references
through it: to page URLs for the site, to chapter labels for the book, and to
``#<document key>-<anchor>`` in ``singlehtml``. Native ``.typ`` documents become site pages
through Typst's HTML export and book chapters through ``#include``. The native Typst
metadata contract above is not implemented.

A build-cache key includes source and transitive dependency digests, reader version,
configuration, renderer version, template, fonts, and Typst version. The implementation
also includes the executable's size and modification time, so a rebuilt ``guidedog`` never
reuses an older conversion. A page's key also covers what its references resolved to.
The link pass records every lookup it makes, and the next build repeats them. Diagnostics
are cached with the output they came from and reported again when it is kept. Development
caches are host-owned and may allocate. A clean build and a cached build must emit
the same artifacts for the same inputs, excluding explicitly requested timestamps.


Responsive HTML and the Typst book
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


The first HTML theme, proposed name **Atlas**, has a restrained neutral palette,
a clear accent color, a readable article measure, and generous spacing. Desktop
layout includes project navigation, article, and an on-page outline. On narrow
screens, navigation becomes an accessible drawer and the article remains readable
at 320 CSS pixels. Wide tables and code scroll within their containers.

Require semantic landmarks, skip links, keyboard navigation, visible focus, proper
heading order, image alternatives, high contrast, and reduced-motion support.
The page remains readable without JavaScript. Search uses a generated local index.
No remote service is required. CSS, fonts, and icons ship with the binary, with
license notices. Native Typst HTML is integrated through its supported export and
template mechanisms. Do not concatenate entire HTML documents into a page shell.

The PDF theme, proposed name **Folio**, is a Typst book template. It has a title page, contents,
numbered chapters, running headings, readable body type, restrained code styling,
figures, tables, notes, citations, and useful page breaks. Test A4 and Letter,
long URLs, wide tables, multilingual text, and embedded fonts. User content is
passed as content and data, with a small documented theme-token surface.

Atlas and Folio are implemented and embedded. Desktop and phone-width pages and PDF
pages were reviewed as rendered images. A formal accessibility audit remains open.
A project ``pdf.preamble`` file adds package imports and show rules to the book.


Typst and the single executable
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


Typst's compiler is a Rust library. ``gd-typst`` links the Typst 0.15.1 crates directly
(feature ``embedded``, on by default), so the release binary needs neither an installed
``typst`` command nor a downloaded package or font. Typst types never leave ``gd-typst``.
A panic inside the compiler is caught inside the crate and becomes a failure. A hard
process termination cannot be made recoverable by catching panics.

``gd-typst`` implements Typst's resource environment. Project files are read only
under the Typst root. **Typst packages are supported**, because publication-quality PDF
depends on them and Guidedog writes Typst rather than LaTeX. ``@preview`` and ``@local``
imports resolve like the typst CLI: from the package data directory, then the cache,
then a Typst Universe download into the cache. A package reads only inside its own
directory and has no shell or network access of its own. That is Typst's sandbox, and
it makes a package no riskier than a LaTeX package. Versions are pinned by the import
itself (``@preview/name:1.2.3``). ``--offline`` (or ``pdf.packages = "offline"``) restricts
builds to packages already on disk, for CI and air-gapped machines.

**Fonts** also follow the typst CLI: extra ``--font-path`` directories first, then system
fonts, then the fonts embedded in the executable as a fallback. So a book can use the
fonts installed on the machine. ``--embedded-fonts-only`` (or ``pdf.fonts = "embedded"``)
ignores system fonts for byte-reproducible output across machines. Trusted templates
are embedded. No other filesystem or network access is delegated to document content.
For hard time or memory isolation, a future implementation would invoke an
internal worker and apply OS limits. This worker mode is not implemented.
Per-platform containment and cancellation must be verified before hostile native
Typst is advertised as safely isolated. An in-process compiler is not a sandbox.

One request (``gd_typst::Options``) carries the target, the package mode and cache, the
font mode and paths, and the folders the compiler may read. One owned result
(``Compiled``) returns the output and the diagnostics.

When the crate is built without ``embedded``, ``gd_typst::locate`` finds an installed,
pinned ``typst`` program, which runs with an argument vector. That fallback does not
satisfy the single-binary release gate. Binary size, licensing, supported platforms,
HTML export fidelity, and worker containment are explicit technical checkpoints.


.. graphviz::
   :caption: Native .typ input starts at step 3. Failed compilation never reaches step 5.
   :alt: Native .typ input starts at step 3. Failed compilation never reaches step 5.

   digraph design {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontcolor="#16382f", fontname="Helvetica", fontsize=11];
     edge [color="#216553", arrowsize=0.7];
     n0 [label="Guidedog -> Guidedoc\n1. RST / CommonMark + resources"];
     n1 [label="Guidedoc -> Guidedog\n2. Generated Typst + source map"];
     n2 [label="Guidedog -> Typst worker\n3. Compile Typst to PDF"];
     n3 [label="Typst worker -> Guidedog\n4. PDF or mapped diagnostics"];
     n4 [label="Guidedog -> Publisher\n5. Stage validated artifacts"];
     n5 [label="Publisher -> Guidedog\n6. Commit or report failure"];
     n0 -> n1;
     n1 -> n2;
     n2 -> n3;
     n3 -> n4;
     n4 -> n5;
   }


Publication is a host transaction
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


Conversion to a path writes a sibling temporary file, flushes and closes it, then
replaces the destination using the platform's supported rename operation. Refuse
existing output unless ``--force`` is explicit. stdout output cannot be rolled back, so
finish conversion into staging memory before emitting it.

A website or book with resources is a set of files, so two individual renames do
not form an atomic transaction. Stage an entire versioned build directory and
switch an application-owned current-version pointer where supported. On platforms
without an atomic switch, preserve the previous build and document recovery.
An optional provenance manifest records versions, digests, diagnostics, and resources.
It belongs to the same staged set. Crash durability requires directory sync behavior
where available and is tested separately from logical all-or-nothing publication.

**Project builds** follow the Publication section of ``0006-sphinx-replacement``. A build
writes the whole next output folder beside the live one and switches the two, in one step
where the system can (``renameat2`` or ``renamex_np``). It journals the switch, so a
failure is undone and a crash is finished by the next build. ``gd_host::Stage`` removes a
staged folder when it is dropped uncommitted, and ``gd-outputs`` switches whole
generations.

Delivery phases
~~~~~~~~~~~~~~~


.. graphviz::
   :caption: Each phase proves behavior before adding a larger surface. No phase promises speed.
   :alt: Each phase proves behavior before adding a larger surface. No phase promises speed.

   digraph design {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontcolor="#16382f", fontname="Helvetica", fontsize=11];
     edge [color="#216553", arrowsize=0.7];
     n0 [label="0. Specification\nReview contracts and resolve integration\nquestions"];
     n1 [label="1. CLI\nskeleton, ANSI reports, help, version,\nGDS management"];
     n2 [label="2. Core vertical slice\nFlat AST; CommonMark -> HTML and Typst ->\nPDF"];
     n3 [label="3. Full language coverage\nComplete CommonMark and full RST; native\nTypst routes"];
     n4 [label="4. Guidedog project\nReferences, dependency graph, responsive\nHTML, book theme"];
     n5 [label="5. Release qualification\nOne binary, failure recovery, conformance and\nresource audits"];
     n0 -> n1;
     n1 -> n2;
     n2 -> n3;
     n3 -> n4;
     n4 -> n5;
   }


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
Phase 5's single binary works on macOS and Linux. Windows runtime, worker isolation, and
font-license review remain.


Security considerations
-----------------------


Source content, terminal excerpts, include paths, and generated Typst are distinct
trust boundaries. The rules above require escaping, approved resource roots,
explicit raw-content policy, and isolated Typst execution when hard bounds matter.
No document may acquire filesystem or network authority by naming a resource.
The single-binary packaging requirement is not itself a sandbox guarantee.


Backwards compatibility
-----------------------


The CLI, conversion, and project commands described above are available in this checkout.
The diagnostics JSON schema is version 1. Changes to it require a new schema version.
The project does not emulate every Sphinx extension or configuration
script. Full RST support and Sphinx-specific extension support are separate claims.
Existing source files must receive useful diagnostics when policy prevents a feature.


Alternatives considered
-----------------------


A separate converter executable would duplicate the future Guidedog command surface.
An installed Typst command is useful during development but fails the single-binary
release requirement. Reimplementing Typst would replace a mature compiler with a second
implementation. Linking the compiler crates reuses the real compiler. Mandatory sidecars for every byte
conversion would improperly move host policy into the core.


Discussion management
---------------------


``guidedog dsc`` follows Docodin's integrated DOD commands and the inherited GDS process. The CLI implements ``new``, ``promote``, ``list``,
``show``, ``state``, ``index``, ``check``, ``build``, and ``recover`` according to GDS 0001.
GDS HTML/PDF builds use the embedded Typst compiler when it is linked, and otherwise the
installed fallback. Both this
repository and documentation projects can select a GDS root with ``--dir``.


Acceptance criteria and reference implementation
------------------------------------------------


The CLI must pass help, usage, option precedence, redirection, JSON, ANSI, exit-code, and
report-overflow tests. The project builder must pass the engine discussion's conformance
and resource tests, the publication failure tests above, and visual review of HTML and PDF
templates. Single-binary qualification requires a clean machine with no Python or installed
Typst.


Evidence
~~~~~~~~


The reference implementation is ``cmd/guidedog`` (crate ``guidedog-cmd``). It composes
``gd-cli`` with ``gd_host::catch_interrupts``, ``install_crash_reporter``, ``catch_crash``
and ``run_with_stack`` around ``gd_cli::run_with(registry)``. ``run_with`` also serves
custom binaries with linked plugins (GDS 0009, 0010). The program builds on
``gd-discussion``, ``gd-host``, ``gd-typst``, ``gd-project``, ``guidedoc-defaults``, and the
Guidedoc crates of GDS 0003.

- ``tests/integration`` holds the end-to-end tests: discussion workflows, recovery, dates,
  and Typst builds; conversion to HTML and PDF from RST, CommonMark, and Typst; overwrite
  refusal; JSON and text diagnostics; include confinement; offline package resolution
  from a private cache; project quickstart, build, incremental rebuild, failed-build
  preservation, and configuration errors; the preview server; memory budgets;
  publication of whole generations; ``singlehtml`` addressing; link confinement; and
  ``--untrusted``.
- Each crate carries unit tests, and the CommonMark reader passes all 652 examples.
- Ctrl+C during a PDF build exits 130, leaves no staged file, and publishes nothing.
- ``tools/check`` passes every source file.

.. code-block:: sh

   cargo build --release -p guidedog-cmd     # target/release/guidedog, Typst embedded
   cargo test --workspace
   cargo clippy --workspace --all-targets
   cargo run -p check


This record is retained as ``published``, with incomplete obligations below.
GDS 0004 specifies the implemented resource policy. GDS 0006 supersedes the
original project surface and publication design. The remaining obligations
prevent marking this broader record ``committed``.


Decisions still requiring implementation evidence
-------------------------------------------------


Hard time and memory isolation for hostile native Typst still needs a worker
mode and per-platform verification. The native Typst chapter metadata contract
is not implemented. Typst is supported by standalone conversion and PDF templates.
Windows runtime and clean-machine package checks passed for 0.6.2 on native runners
(see :doc:`../evidence/plugin-acceptance-20261014`). Embedded font licenses are
recorded with the fonts. They are no longer an open documentation item.
Packaging must account for actual runtime dependencies: Graphviz is linked from the
system. A clean machine distribution check remains a separate release gate. The RST ledgers
no longer have the previously listed syntax gaps. Their pinned tests and
conservative reader descriptor state the evidence accurately.

These limits do not prevent a beta for trusted documentation projects on
the five released platforms with the declared dependencies. They do prevent
advertising hostile-input isolation or a dependency-free Linux executable.


Review history
--------------


29 September 2026: separates terminal and host policy from the allocation-free
engine. Specifies full source-language scope, Typst PDF routes, and later project
features. The initial revision preceded the implementation.

29 September 2026, implementation review: record the working GDS CLI, HTML catalog,
individual PDFs, journal recovery, local dates, and portable host boundaries.
Correct the inherited assignment from 0017 to Guidedog's 0002. zenfmt numbering
does not reserve GDS numbers. Record the 18-test evidence and unresolved acceptance
gates. Preserve discussion status because the full record is not yet implemented.

29 September 2026, implementation of conversion and projects: record ``convert``, ``init``,
``build``, and ``serve``; the embedded Typst compiler; Typst packages and system fonts at the
user's direction, replacing offline-only packages and embedded-only fonts; Ctrl+C
handling; Linux runtime evidence; and the remaining open items.

29 September 2026, review of the build pipeline: record whole-generation publication with a
manifest, recorded lookups and replayed diagnostics, session memory budgets reserved before
allocation and shared by reading threads, confinement of linked files, ``--untrusted``, and
the current test evidence. The cross-file symbol index is implemented.


References
----------


- GDS 0003, Guidedoc: the conversion engine.
- GDS 0001, The Guidedog Discussion Process.
- `Typst compiler crate 0.15.1 <https://docs.rs/typst/0.15.1/typst/>`_.
- `Typst HTML export <https://typst.app/docs/reference/html/>`_.

.. Lifecycle event recorded by guidedog.

.. rubric:: Lifecycle event

2026-09-29: prediscussion -> discussion. Assigned a permanent number and opened for discussion.

.. rubric:: Source-format revision, 2026-10-01

RST discussions now use the normal project engine. The design record remains
in discussion. See GDS 0001 for the revised authoring contract.

.. rubric:: Lifecycle event

::

   2026-10-11: discussion -> published. Retain the implemented CLI contract with
   explicit unfinished worker, native Typst chapter, Windows and packaging
   obligations.
