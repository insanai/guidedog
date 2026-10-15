Testing
=======

Guidedog is tested at three levels. Each crate tests its own behaviour. The
``guidedog-tests`` crate (``tests/integration``) drives the built programs end to end
and holds the repository-wide quality gates. The compiler, Clippy and
``tools/check`` enforce the rules that can be stated as lints and limits.

Running the tests
-----------------

.. code-block:: sh

   cargo test --workspace                  # everything, the end-to-end tests included
   cargo clippy --workspace --all-targets  # lints, missing docs and unsafe code included
   cargo run -p check                      # function, file and line-length limits
   cargo xtask tables --check              # the data tables match tools/data
   cargo test -p guidedog-tests --test gates     # only the quality gates
   cargo test -p guidedog-tests --test projects  # one end-to-end group

The end-to-end tests run the programs cargo built into ``target/<profile>/``
(``guidedog`` and the plugin examples). ``cargo test --workspace``
builds them. To run only ``guidedog-tests``, run ``cargo build --workspace`` first. A
test that finds a program missing says so instead of failing obscurely. Every test
works in a private temporary folder and sets ``GUIDEDOG_CONFIG_DIR`` and ``NO_COLOR``,
so tests run in parallel and never touch the machine's plugin registry.

Which crate tests what
----------------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Where
     - What it covers
   * - ``guidedoc``
     - The AST, the builder, limits, anchors, and steady-state conversion without
       allocation (a counting allocator in ``tests/services.rs``).
   * - ``guidedoc-rst``, ``guidedoc-rst-inline``
     - reStructuredText against the Docutils corpus (``tests/conformance``) and the
       ledger in :doc:`rst-ledger`.
   * - ``guidedoc-commonmark``
     - The CommonMark 0.31.2 specification and the MyST conformance corpus.
   * - ``guidedoc-sphinx``
     - Sphinx directives, roles and domains, and MyST under Sphinx.
   * - ``guidedoc-html``, ``-typst``, ``-text``
     - Each renderer's output; ``guidedoc-defaults`` checks that a document holding
       every ``NodeKind`` renders with every built-in renderer.
   * - ``gd-read``, ``gd-resolve``, ``gd-link``, ``gd-site``, ``gd-book``, ``gd-project``
     - Reading, incremental rebuilds, resolution, the HTML and PDF builders, trust and
       budgets, through each crate's library API.
   * - ``gd-cli``, ``cmd/guidedog``
     - Argument parsing, help, versions, conversions in process (``gd_cli::run_in``
       with captured streams), and the program's exit statuses.
   * - ``gd-discussion``, ``gd-plugins``, ``gd-pages``, ``gd-serve``, ...
     - Their own behaviour, with fixture programs and local servers where a protocol is
       involved.
   * - ``tests/integration``
     - The user's view: whole projects built by the real ``guidedog``, and the
       repository-wide gates below.

End-to-end tests
----------------

Each test binary of ``tests/integration`` groups one area, and each module inside it covers one feature of that area.

.. list-table::
   :header-rows: 1
   :widths: 22 78

   * - Binary
     - Covers
   * - ``projects``
     - The manual's tutorials and the theme tutorial built as written (HTML, every
       address scheme, PDF); publication (a failed or ``-W`` build publishes nothing, a
       file that becomes a folder, unusable cached objects, truthful summaries); builds
       without Python; Sphinx project behaviour (numbering, templates, ``html_context``,
       additional pages, indexes, intersphinx, math, canonical names); budgets and
       capacity limits.
   * - ``sphinx``
     - ``rst_prolog``/``rst_epilog``, ``numref``, toctrees and their numbering, ``only``,
       object descriptions in the TOC, anchors and ids as Sphinx and Docutils give them,
       declared object types, ``objects.inv`` contents, translation workflows, and trust
       (untrusted builds read only the source folder, books and graphs are confined).
   * - ``tools``
     - ``guidedog convert`` (problems with directions, stack budgets, confined includes,
       Typst sources and packages), autodoc for Python and Odin sources, incremental
       rebuilds across builders, installed build plugins and the public plugin
       catalog, release versions, and the manual's copies of repository files.
   * - ``discussions``
     - The discussion workflow through ``guidedog dsc``: new,
       promote, state changes and their rules, recovery from an interrupted change, the
       catalog lock, dates, configured workflows and numbering, and building catalogs.
   * - ``manual_code``
     - Every ``code-block:: rust`` of the manual has a compiled twin with the same text;
       a block that only sketches an API is marked ``:class: sketch``.
   * - ``allocations``
     - Repeated conversion in a reused ``Workspace``, for every reader and renderer
       pair, allocates no more than the time before and no more than its recorded
       ceiling (a counting global allocator; see below).

Quality gates
-------------

Most rules about imports, dependencies and unsafe constructs belong to cargo and the compiler, so the gates ask them, and scan the sources only for what the compiler cannot see.

Dependencies (``gates::dependencies``)
   Reads ``cargo metadata``. Every package has a layer (core and supporting libraries,
   readers and renderers, defaults, host, project, command line, programs), and depends
   only on its own layer or lower ones. The engine crates (``guidedoc*``) need nothing
   outside the workspace. The supporting libraries never depend on the engine. External
   crates are the chosen few (Typst only in ``gd-typst``, tree-sitter in ``gd-pyscan``
   and ``gd-rustdoc``, ``serde_json`` in ``gd-json``). A new crate or dependency fails
   the gate until it is placed in its table, with the reason in ``Cargo.toml``.

Unsafe code (``gates::unsafe_code``)
   The workspace denies ``unsafe_code``. Every crate without FFI also forbids it with
   ``#![forbid(unsafe_code)]``. The FFI modules (the Graphviz binding, the operating
   system calls in ``gd-host``, the credential stores, ``localtime_r``, two counting
   allocators in tests) open themselves with ``#[allow(unsafe_code)]``. The gate keeps
   that list explicit and checks that every unsafe block and impl has a ``// SAFETY:``
   comment above it. Clippy's ``undocumented_unsafe_blocks`` says the same while
   editing.

Hints (``gates::hints``)
   Every diagnostic written in the code (``Message`` tables, ``Problem`` and ``Report``
   literals, and helpers that take a ``hint`` parameter) has a hint that gives
   directions: an imperative clause that names something to act on, a "Did you mean"
   suggestion, or "Nothing to do:" with the reason. See :doc:`../errors`.

Documentation of the API
   The workspace lint ``missing_docs`` makes every public item documented, so no scan of the sources is needed for it. The README
   cheat sheets stay short fragments for reading, not doctests. The programs a reader
   would copy are the ``guidedoc-examples`` crate's examples and the manual's twins.

Limits
   ``cargo run -p check`` holds functions to 70 logical lines, files to 1,408 lines, and
   lines to 99 (soft) and 108 (hard) characters, in ``crates``, ``cmd``, ``tools``,
   ``examples`` and ``tests``.

Allocation
----------

Rust has no per-call allocator, and its global allocator aborts when memory runs out, so a test cannot refuse allocations one by one. Two counting-allocator tests check allocation instead. ``guidedoc`` proves that the engine core
converts in steady state without allocating, and ``allocations`` bounds what the readers
and renderers add. Their scratch vectors (block and inline item lists, anchor tables,
renderer state) are still built per conversion. The ceilings record today's counts, to
be lowered as that scratch moves into the workspace. Work driven by the input is
bounded by ``Limits`` and the host budget, which return ``Capacity`` or ``Limit``
results. The ``projects`` capacity tests check them through the real readers and
renderers.

Verification against Sphinx and Docutils
----------------------------------------

Three tasks compare Guidedog with the Python programs it replaces. They run in a separate verification environment with pinned versions; Guidedog itself never runs Python, and the tests only read the files the tasks stored.

.. list-table::
   :header-rows: 1
   :widths: 35 65

   * - Task
     - What it does
   * - ``cargo xtask rstref``
     - Stores the Docutils reference trees (and, with ``--ids``, ``ids.txt``) of the reStructuredText corpus that ``guidedoc-rst``'s conformance test reads. See :doc:`rst-conformance`.
   * - ``cargo xtask verify-docutils``
     - Regenerates that corpus with a pinned Docutils and compares it byte for byte with the committed files, writing ``VERIFICATION.txt``.
   * - ``cargo xtask sphinxdiff``, ``cargo xtask sphinxdiff-roots``
     - Compare a Sphinx HTML build with a Guidedog build (pages, ids, links, numbers, ``:numref:`` texts, ``objects.inv``, warnings), one project or 42 of Sphinx's test roots. See :doc:`sphinx-comparison`.

What the suite leaves out
-------------------------

Some checks live elsewhere or have no meaning for Guidedog. They are left out on purpose:

- The conformance suites live in the reader crates, and the serve test in ``gd-serve``.
- Refusing the n-th allocation has no meaning with Rust's global allocator. Budgets are
  tested instead.
- Safe Rust has no crash-probe build for segmentation faults, bounds and aborts. A panic
  ending in a crash report is tested in ``gd-host``.
- Checking every target operating system needs each target's standard library
  installed, and belongs to the release workflow's operating system matrix.
- The questions a build asks at a terminal are tested in ``gd-cli`` and ``gd-project``
  without a pseudo-terminal.
- Guidedog keeps working memory in RAM. A test checks that the ``--memory`` option is
  refused with an explanation.
