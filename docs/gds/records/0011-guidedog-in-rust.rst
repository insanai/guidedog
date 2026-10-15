:gds-number: 0011
:gds-title: Guidedog in Rust
:gds-state: discussion
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-10-15
:gds-updated: 2026-10-15
:gds-discussion: No external discussion URL assigned
:gds-labels: ["architecture","library","build","compatibility","performance","security"]

Guidedog in Rust
================

Abstract
--------

Guidedog and its engine, Guidedoc, move from Odin to Rust. The Odin implementation
(GDS 0002 to GDS 0010) becomes the specification of *behaviour*: commands, options,
``conf.toml`` keys, directives, roles, output, limits, error cases, and tests. It is not a
template for the code. Each Odin package is redesigned the way a Rust programmer would
write it and lands as a crate of one Cargo workspace.

A user observes the same ``guidedog`` command, the same projects, and the same HTML and
PDF. What changes is underneath. The Odin slabs, arenas, allocator guards, and generation
checks give way to ownership, reused buffers, lifetimes, and explicit ``Limits``. The Typst
compiler is a Rust library linked directly instead of through a C ABI bridge. Graphviz is
the system library. HTTP goes through the ``curl`` program. The port also adds Rust API
documentation (``gd-rustdoc`` and a ``rust`` domain), linked plugins in custom binaries,
and a guided GitHub Pages setup. This record states those decisions, what each earlier
record keeps, and what the port deliberately gives up.

Motivation and scope
--------------------

The Odin implementation works, and its records are committed. Three costs kept recurring
in them, and each is a cost of the language boundary rather than of the design:

- **Memory discipline by convention.** GDS 0001's memory-safety rules (check every
  allocation, ``host.Guard``, arenas that survive refusal, no views past their memory,
  explicit ``unload`` and ``release``) are enforced by scans, refusal sweeps and
  AddressSanitizer runs. Every review in GDS 0004, 0007, 0008 and 0009 found unchecked
  allocations or ownership faults that those sweeps then caught. Rust's ownership and
  borrow rules make most of these mistakes compile errors instead of test findings.
- **The Typst bridge.** Typst is a Rust library. GDS 0002 had to design a versioned C ABI
  (``native/typst_bridge``), forbid unwinding across it, vendor OpenSSL for Linux and still
  link system libcurl. In Rust the compiler is an ordinary dependency.
- **Native dependencies built by script.** tree-sitter and Graphviz were built from C by
  scripts under ``native/``. Rust has maintained tree-sitter crates, and the system
  Graphviz library is linked like any other.

Scope: every crate of the workspace, the binaries (``guidedog``, ``dsc``, ``dscc``,
``graphdog``, ``guidedog-pages``), the source-limit checker, and this documentation.
Out of scope: changing any user-visible behaviour without its own record, and the Odin
domain and odindoc, which stay because Odin remains a documented *language*.

Specification
-------------

Behaviour is the contract
~~~~~~~~~~~~~~~~~~~~~~~~~

The Odin code and its tests are the specification. Where the Odin documentation and the
Odin code disagree, the code wins. A crate keeps its Odin package's behaviour and test
expectations unless a change is a deliberate improvement. Every deliberate change is
listed in the crate's ``README.md`` ("Redesigns") or in an internals page under
``docs/manual/internals/``. Persistent formats move with explicit versions. Guidedoc's
object format is version 2, and the project environment and manifest versions are
bumped, so Odin caches are rebuilt rather than misread. Wire formats that other programs
speak (the plugin process protocols, the discussion journal, gettext MO files, Sphinx
inventories) do not change.

The crate split and dependency direction
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

There is one Cargo workspace. Every crate inherits the single workspace version
(``version.workspace = true``), edition, licence, ``rust-version`` and lints.

.. list-table:: Crates and their responsibilities
   :header-rows: 1
   :widths: 32 68

   * - Crates
     - Responsibility
   * - ``guidedoc``
     - Engine core: AST, builder, validation, diagnostics, registry, pipeline.
   * - ``guidedoc-rst``, ``guidedoc-rst-inline``
     - reStructuredText reader: blocks, inline markup, directives, roles.
   * - ``guidedoc-commonmark``
     - CommonMark 0.31.2 and MyST reader.
   * - ``guidedoc-sphinx``, ``guidedoc-cpp``
     - Sphinx directives, roles and domains (Python, C++, Odin, Rust) for RST and MyST.
   * - ``guidedoc-html``, ``guidedoc-typst``, ``guidedoc-text``
     - Renderers; the Typst renderer carries the LaTeX math adapter.
   * - ``guidedoc-defaults``
     - The built-in registry and the highlighter adapter.
   * - ``gd-toml``, ``gd-json``, ``gd-jinja``, ``gd-highlight``, ``gd-gettext``,
       ``gd-graphdog``, ``gd-flate``, ``gd-sha``, ``gd-report``, ``gd-fetch``, ``gd-git``,
       ``gd-inventory``, ``gd-pyscan``, ``gd-autodoc``, ``gd-odindoc``, ``gd-rustdoc``,
       ``gd-typst``, ``gd-i18n``, ``gd-discussion``, ``gd-plugins``, ``gd-pages``
     - Supporting libraries. They never depend on ``guidedoc``, except ``gd-i18n``, whose
       job is translating documents; only adapters join the others to the engine.
   * - ``gd-host``
     - Where the engine meets the operating system: sessions, confined files, atomic
       publication, budget, workers, interrupts, crash reports, ``convert_artifact``.
   * - ``gd-conf``, ``gd-migrate``, ``gd-env``, ``gd-outputs``, ``gd-resolve``,
       ``gd-link``, ``gd-read``, ``gd-intl``, ``gd-site``, ``gd-book``, ``gd-serve``
     - The project builder, split by stage: configuration, migration, environment and
       plugin runs, generations, resolution, linking, reading, translation, the HTML and
       the PDF/text builders, and the preview server.
   * - ``gd-project``, ``gd-cli``
     - The coordinator (load, build, clean) and the ``guidedog`` command line.
   * - ``cmd/*``
     - ``guidedog``, ``dsc``, ``dscc``, ``graphdog``, ``guidedog-pages``: composition and
       exit status only.
   * - ``tools/check``
     - The source-limit checker.

Dependencies point one way. ``cmd`` depends on the CLI, the CLI on the project, the project on the host, the host on the defaults, the defaults on the readers and renderers, and those on the core. The project builder is itself layered.
``gd-env`` depends on ``gd-conf``; ``gd-outputs`` on ``gd-env``; ``gd-resolve`` on ``gd-outputs``; ``gd-link`` on ``gd-resolve``; ``gd-site`` and ``gd-book`` on ``gd-link``.
``gd-intl`` depends on ``gd-env``, and ``gd-read`` on ``gd-intl``. ``gd-project`` depends on all of them, and ``gd-cli`` on ``gd-project``. ``gd-cli`` never
depends on ``gd-pages``: publishers reach it through ``gd-plugins``.

.. graphviz::
   :caption: Dependencies point down; supporting libraries stand beside the engine.
   :alt: cmd depends on cli, cli on project, project on host, host on defaults, defaults on readers and renderers, which depend on the guidedoc core. Supporting libraries are joined only by adapters.

   digraph crates {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontcolor="#16382f", fontname="Helvetica", fontsize=11];
     edge [color="#216553", arrowsize=0.7];
     cmd [label="cmd/*\nguidedog, dsc, dscc, graphdog, guidedog-pages"];
     cli [label="gd-cli"];
     project [label="gd-project\ngd-site, gd-book, gd-read, gd-link,\ngd-resolve, gd-outputs, gd-env, gd-conf"];
     host [label="gd-host"];
     defaults [label="guidedoc-defaults"];
     adapters [label="readers and renderers\nguidedoc-rst, -commonmark, -sphinx,\n-html, -typst, -text"];
     core [label="guidedoc"];
     libs [label="supporting libraries\ngd-jinja, gd-gettext, gd-highlight,\ngd-graphdog, gd-typst, gd-toml, gd-json, ...", fillcolor="#f5f2ea"];
     cmd -> cli -> project -> host -> defaults -> adapters -> core;
     project -> libs [style=dashed];
     defaults -> libs [style=dashed];
   }

The engine crates (``guidedoc*``) use only the standard library, so they stay embeddable.

Memory: ownership instead of allocators
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

GDS 0003 made memory part of the API: the caller sized slabs, the engine refused to call
an allocator, and a ``Capacity`` result told the host which region to grow and retry.
GDS 0004 metered everything else through one budget, with guards, arenas that survive a
refused block, and an option to spill working memory to disk. In Rust:

- A ``Workspace`` owns its tables as ``Vec`` and ``String`` and ``clear()``\ s them between
  conversions, keeping their capacity. A conversion in steady state allocates nothing; a
  test with a counting allocator checks it. There are no slab plans, banks, scratch
  regions, or retries.
- Growth that input drives uses ``try_reserve``, so a refused reservation is a
  ``Capacity`` or ``Memory`` result, not an abort. Everything else follows Rust's global
  allocator, which aborts on failure. So the per-allocation refusal guarantee of GDS 0001 and
  GDS 0004 is deliberately **given up** for ordinary small allocations. The refusal
  sweeps that tested it are dropped.
- Lifetimes replace the generation and revision checks. A ``Document<'w>`` borrows its
  workspace, so a stale or foreign document handle does not compile.
- ``Drop`` replaces ``unload``, ``release``, ``destroy_*`` and ``close``: stages remove
  themselves unless committed, locks release, secrets zero themselves.
- Flat data stays flat: nodes, ids and side tables are ``Vec``\ s indexed by newtypes
  (``NodeId(u32)``, ``DocId(u32)``), not ``Rc<RefCell<..>>`` graphs. ``Rc``/``Arc`` appear
  only for genuinely shared values (included sources as ``Arc<str>``, Jinja's shared
  containers).
- The budget of GDS 0004 stays, coarser: ``gd_host::Budget`` counts what the host can size
  before allocating (sources, included files, the PDF the embedded compiler returns).
  ``limits_within`` lowers a conversion's ``Limits`` to it. Units that wait for each other
  without deadlock, the raise-or-stop policy and ``--budget=N`` hints are kept. ``Meter``,
  ``Guard``, ``Arena`` and ``Disk`` (working memory on disk) are dropped.

The details are in ``crates/guidedoc/README.md``, ``crates/gd-host/README.md`` and
``docs/manual/internals/engine-api-map.rst``.

Bounded work and stack guards
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Bounded work is unchanged in intent. ``guidedoc::Limits`` holds ``max_depth``,
``max_nodes``, ``max_work``, ``max_include_depth``, ``max_included_bytes``,
``max_expansion``, ``stack_bytes``, ``max_text_bytes``, ``max_reports`` and
``max_output_bytes``. Reaching one is a ``Limit`` or ``Capacity`` result, never a panic or
a stack overflow. Recursive readers and renderers measure the machine stack with a
``StackGuard`` (``stack_guard``, ``stack_exhausted``) against ``stack_bytes``, as GDS 0003
specified. ``gd_host::run_workers`` and ``run_with_stack`` start threads with a known
8 MiB stack, so the stack budget is known to fit. The same rule holds in the libraries.
gd-jinja bounds nesting, recursion and stack bytes. gd-odindoc counts parser nesting
(160 levels) instead of estimating stack. gd-rustdoc bounds depth, files and items.
gd-json refuses nesting past 127. Values nested deeply are dropped iteratively.

Dependencies
~~~~~~~~~~~~

Standard library first. Small things are written in-house: TOML, JSON's API, Jinja,
gettext, hashing (``gd-sha``), inflate (``gd-flate``), argument parsing, the loopback HTTP
test server. An external crate is admitted only when it is mature, self-contained and
solves a genuinely large problem. Its reason is written beside it in ``Cargo.toml``:

.. list-table:: External dependencies
   :header-rows: 1
   :widths: 30 70

   * - Dependency
     - Why, and where it may appear
   * - ``typst``, ``typst-layout``, ``typst-pdf``, ``typst-html``, ``typst-kit`` (pinned
       ``=0.15.1``)
     - The embedded compiler. Typst lives **only** inside ``gd-typst`` (feature
       ``embedded``, on by default), with an installed, pinned ``typst`` program as the
       fallback backend. No Typst type or design leaks outside ``gd-typst``, and no other
       dependency is chosen because Typst happens to compile it in.
   * - ``tree-sitter``, ``tree-sitter-python``, ``tree-sitter-rust``
     - Python parsing for autodoc (``gd-pyscan``) and Rust parsing for ``gd-rustdoc``.
   * - ``serde_json`` (and ``serde``)
     - Behind ``gd-json``'s small API (validating parse, duplicate-key refusal, HTML-safe
       writer, Elm-style errors). Chosen by the maintainer.
   * - Graphviz (``libgvc``, ``libcgraph``)
     - Linked from the system through a hand-written FFI module in ``gd-graphdog``
       (``src/ffi.rs``); every call runs under one process-wide lock.
   * - ``curl`` (the program)
     - ``gd-fetch`` runs it with an argument vector, never a shell, for intersphinx and
       the Pages publisher. Headers travel in a private file, never on the command line.
       No libcurl is linked.

There are no ``native/`` build scripts. The project no longer builds Graphviz and tree-sitter from
C sources. ``unsafe`` appears only at foreign-function boundaries (Graphviz;
the Keychain and Credential Manager in ``gd-pages``; ``renameat2`` and ``signal`` in
``gd-host``; the C library's ``localtime_r`` for local dates) and in test-only counting
allocators, each block with a ``// SAFETY:`` comment.

Errors in Elm style
~~~~~~~~~~~~~~~~~~~

``gd-report`` gathers what the Odin code spread over four files: one ``Report`` (code,
severity, title, message, hints, path, line, column, excerpt) and the text and JSON
renderers. Every crate returns structured errors that convert to a ``Report`` and never
prints. A user-facing report has a title rule, the location, an excerpt with carets, a
plain explanation, and a ``Hint:``, plus "Did you mean ...?" from
``gd_report::suggest`` where a name was misspelled. Terminal text is escaped
(``escape_terminal``), and widths follow Unicode 15.1.

API style
~~~~~~~~~

The public APIs follow raylib's principles, adapted to Rust:

- few concepts and plain data: public-field structs with ``Default`` (``Request``,
  ``Limits``, ``Options``, ``Settings``) filled with struct-update syntax;
- a small verb-noun vocabulary: ``load_*`` with ``Drop`` for unloading, ``begin_*`` and
  ``end_*`` pairs, ``is_*`` predicates, ``*_count``, no ``get_`` prefix;
- a one-call common path beside the staged one (``guidedoc::convert_text`` beside
  ``read_document``, ``transform_document``, ``check_render``, ``render_document``;
  ``gd_rustdoc::document_crate`` beside ``load_crate`` and ``document``);
- no hidden global state: registries are immutable values, one workspace per thread;
- enums with exhaustive ``match`` (no ``_ =>`` on ``NodeKind`` in renderers), newtype ids,
  ``Option`` instead of sentinels, traits and closures instead of procedure-plus-``rawptr``
  callbacks, ``#[non_exhaustive]`` inputs where plugins must evolve.

Documentation
~~~~~~~~~~~~~

Guidedog documents itself in reStructuredText under ``docs/``, built by Guidedog: the
manual (``docs/manual``, internals under ``docs/manual/internals``) and these records
(``docs/gds``). The only Markdown files are each crate's short ``README.md`` cheat sheet
and the files GitHub needs at the root and in ``docs/gds/README.md``. Design notes,
ledgers and conformance tables are ``.rst`` pages, never ``DESIGN.md`` or similar files in
crates.

Plugins
~~~~~~~

GDS 0009 and 0010 stay the contract. ``gd-plugins`` realizes them with traits
(``BuildPlugin``, ``Publisher``). One implementation has two deployments: **linked** into a
custom ``guidedog`` binary (``gd_cli::run_with(Registry::new(vec![badge::plugin()])?)``),
or **served** as an installed program behind the unchanged JSON Lines protocol
(``gd_plugins::serve_plugin``). Rust has no stable ABI, so there is no dylib loading and no
WebAssembly runtime. A build looks up the linked registry first, then the machine registry.
Programs built with the Odin SDK still speak the same protocol. See
``docs/manual/internals/plugins.rst``.

Templates: Jinja kept in-house
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``gd-jinja`` ports the Odin engine rather than adopting minijinja. We evaluated minijinja 3.0.0
on 2026-10-15. On hostile templates minijinja overflows the stack and aborts
where gd-jinja returns a ``Limit`` error, and it has no output or memory limit. It differs
from Jinja2 in scoping and escaping in ways its extension API cannot change. The layer
needed to restore the Odin behaviour was already about 900 lines. Speed and error quality
were about equal. See ``docs/manual/internals/jinja.rst``.

Translations: large gettext catalogs
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``gd-gettext`` serves compiled catalogs from one buffer checked once at load. ``MoCatalog``
validates the header, tables, strings and hash table up front. It then keeps the strings as one
UTF-8 text pool, so lookups need no checks and no ``unsafe``. A sound GNU hash table gives
:math:`O(1)` lookups (bounded at 64 probes even for a hostile table); otherwise lookups use a binary search. The
writer produces the bytes GNU msgfmt writes, and ``PoReader`` streams PO files. Loading a
61.6 MB catalog went from 155-200 ms and about 240 MB held to 66-82 ms and about the file's
size. See ``docs/manual/internals/gettext.rst``.

Rust API documentation
~~~~~~~~~~~~~~~~~~~~~~

This is new in the port, by analogy with GDS 0005. ``gd-rustdoc`` reads a crate's source with
tree-sitter (nothing is compiled or run). It follows the module tree as rustc does, applies
rustdoc's visibility and inlining rules, and writes reStructuredText of a new ``rust``
domain in ``guidedoc-sphinx``: one page per documented module plus an index. Roles link by
full ``::`` path, with the Python domain's ``~`` and leading ``.``. Projects select crates
with the ``rust_autoapi_*`` keys (``rust_autoapi_crates``, ``rust_autoapi_root``,
``rust_autoapi_options``, ``rust_autoapi_member_order``, ``rust_autoapi_add_toctree_entry``,
``rust_autoapi_generate_api_docs``). ``--untrusted`` drops the crate folders from the
readable roots.

Source limits
~~~~~~~~~~~~~

GDS 0001's limits carry over to Rust. A function body has at most 70 logical lines. A file has
at most 1,408 lines. Lines have a soft limit of 99 and a hard limit of 108 characters.
``cargo run -p check`` (``tools/check``) checks Rust, CSS, JavaScript, HTML and Typst
templates, shell, TOML and C. Documentation under ``docs`` and vendored ``data`` are exempt.
``rustfmt.toml`` sets ``max_width = 99`` and ``clippy.toml`` sets
``too-many-lines-threshold = 70``. A change is done when ``cargo build --workspace``,
``cargo test --workspace``, ``cargo clippy --workspace --all-targets`` and the checker pass.

Rationale and alternatives
--------------------------

**Keep Odin.** The Odin implementation is complete and committed. But the recurring costs
above would stay, and the Typst bridge would remain a hand-maintained C ABI across two
runtimes. Not adopted.

**Port line by line.** A mechanical translation would carry slabs, arenas and generation
checks into a language whose ownership model makes them redundant, and would keep the
procedure-plus-``rawptr`` callback style. Not adopted: the records are the specification,
the code is not a template.

**Adopt existing crates for the supporting libraries** (minijinja, a gettext crate, toml,
a Pygments port). Each was judged on its own merits. minijinja fails the bounds
requirement. The gettext crates copy every string or bind libintl's global state. The
others are small enough to own. Only problems that are genuinely large (a typesetting
compiler, parsers for whole languages, a mature JSON implementation) take a dependency.

**Dynamic plugins** (dylib or WebAssembly). Rust has no stable ABI, and a WebAssembly
runtime is a large dependency for a feature the process protocol already provides with
better isolation. Not adopted.

Security and operational considerations
---------------------------------------

The trust boundaries of GDS 0004 are unchanged: confinement of includes and linked files,
``--untrusted``, a Typst root that holds only what the build may read, plugins refused
under ``--untrusted``, pinned SHA-256 registrations. Two things change. First, an
allocation failure outside a ``try_reserve`` path aborts the process instead of returning a
memory problem. Publication stays safe: outputs are staged and switched whole, so
an abort leaves the previous generation in place. Second, unsafe code shrinks to FFI
modules, and the C ABI bridge to Typst disappears, so a Typst panic is caught inside
``gd-typst`` instead of at a hand-written boundary. A panic elsewhere is a crash: the crash
reporter prints the cause and exits 70. Credentials never reach a command line. The Linux
Secret Service is reached through ``secret-tool`` on standard input and output, and HTTP
headers are passed to ``curl`` in a private file.

Backwards compatibility
-----------------------

Projects, ``conf.toml`` files, discussion catalogs, plugin programs and published sites are
unaffected: the commands, options, keys, directives, roles and outputs are the same. Caches
written by the Odin build (``.gdo`` objects, environment, manifest) are not read. The first
Rust build rebuilds them. Odin embedders of ``lib/*`` must move to the crates. There is no
compatibility layer. Removed options are those that existed only for Odin's allocators
(working memory on disk). The Odin domain and odindoc remain, because Odin is still a
documented language.

Acceptance criteria and implementation status
---------------------------------------------

Evidence required before this record can be committed:

- every crate in the table builds, and ``cargo test --workspace``,
  ``cargo clippy --workspace --all-targets`` and ``cargo run -p check`` pass;
- the conformance evidence of GDS 0003 and 0006 holds again: all 652 CommonMark examples,
  the Docutils comparison corpus, the MyST fixtures and the RST ledgers;
- the Odin and Rust outputs of the manual and these records are compared, and every
  difference is explained or fixed;
- linked and served plugins produce identical outputs (the badge example), and Odin-built
  plugin programs still interoperate;
- the release builds ``guidedog`` with the embedded Typst compiler and runs on a clean
  machine with only the declared system dependencies (Graphviz, ``curl``).

Status on 2026-10-15: the engine, readers, renderers, defaults, host, supporting
libraries, discussion catalogs (``dsc``), plugins and the Pages publisher are ported, with
their tests. The project builder (``gd-conf`` through ``gd-cli``) and the in-process
``dscc`` builder are being ported, so ``guidedog build`` is not yet available from Rust.

Open questions
--------------

- The embedded Typst compiler reads through a symlink root, as the Odin CLI backend did;
  should ``gd-typst`` instead accept the allowed read roots directly?
- Hard time and memory isolation for hostile native Typst (GDS 0002) remains open; a worker
  process is still the likely answer.
- Should the checker fail soft-width findings by default, as the Odin checker did? Today
  ``--soft`` must be passed.
- A Jinja list that contains itself leaks (an ``Rc`` cycle); is that acceptable for trusted
  templates, or should templates be refused such values?
- Which committed records need their implementation evidence re-established in Rust
  before their state can be relied on, and should that be recorded as lifecycle events?

Review history
--------------

2026-10-15: drafted with the port, recording the decisions taken while porting the
engine, host, supporting libraries, discussions, plugins and the Pages publisher, and the
adopted plan for the project builder.

References
----------

- GDS 0001, The Guidedog Discussion Process: coding rules and source limits.
- GDS 0002, Guidedog: CLI and documentation toolchain.
- GDS 0003, Guidedoc: the conversion engine.
- GDS 0004, Memory budgets and what a build may read.
- GDS 0005, An Odin domain and Odin API documentation.
- GDS 0006, Sphinx-style projects and recoverable publication.
- GDS 0007, Discussion catalogs; GDS 0008, GitHub Pages; GDS 0009 and 0010, plugins.
- ``docs/manual/internals/engine-api-map.rst``, ``ast.rst``, ``plugins.rst``,
  ``jinja.rst``, ``gettext.rst``, ``discussion.rst``.
- The crate cheat sheets, ``crates/*/README.md``.
- `The Rust Reference <https://doc.rust-lang.org/reference/>`_ and
  `Typst compiler crate 0.15.1 <https://docs.rs/typst/0.15.1/typst/>`_.

.. rubric:: Lifecycle event

::

   2026-10-15: prediscussion → discussion. Opened for review with the Rust port
