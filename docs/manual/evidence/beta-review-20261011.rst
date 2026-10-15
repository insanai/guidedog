.. _beta-release-review--11-october-2026:

Beta release review — 11 October 2026
=====================================

Guidedog is ready for a **scoped beta for trusted documentation projects on macOS and
Linux**. Its core architecture, project workflow, diagnostics, memory accounting, and
recoverable publication now have substantial repeatable evidence. This is not a claim of
complete Sphinx compatibility, a security sandbox for arbitrary native inputs, or tested
Windows runtime support.

Guidedog is an independent documentation system, implemented in Odin. Sphinx supplies useful
authoring ideas and comparison fixtures; it is not the specification. Running Python
configuration or Python extensions is deliberately outside the design. The release criterion
is a reliable, clear, modern native workflow, not 100% Sphinx compatibility. CPython,
Django, and Flask are large workload probes, not obligations to execute their Python
documentation machinery.

The review found reproducible defects after the earlier verification. Those defects are
fixed in the working tree, with regression tests. A passing older test run was useful
evidence, but it was not sufficient to declare every design promise complete.

What changed in the GDS
-----------------------

The three implemented drafts were promoted with Guidedog's own GDS commands. The registry,
bundle, project configuration, cross-references, and lifecycle history now use permanent
numbers.

Rendered headers use reader-facing labels such as Number, Title, State, Authors, and
Updated. Authors and labels appear as readable lists. The shared project link pass makes
that presentation for HTML, PDF, and text. It reads JSON values from the original source
before smart quotes, emits escaped plain-text nodes, and leaves ordinary fields and source
metadata unchanged. The CLI continues to read and edit the ``gds-*`` keys. Tests cover
Unicode, markup escaping, unchanged source, ordinary fields, both rendered formats, and
allocation refusal.

+---------------------------------+-----------+--------------------------------------+
| Record                          | State     | Meaning                              |
+=================================+===========+======================================+
| 0001 — GDS process              | committed | The RST workflow and recoverable     |
|                                 |           | metadata transactions are            |
|                                 |           | implemented.                         |
+---------------------------------+-----------+--------------------------------------+
| 0002 — CLI and toolchain        | published | The implemented CLI is usable.       |
|                                 |           | Broader worker, native Typst         |
|                                 |           | chapter, packaging, and Windows      |
|                                 |           | obligations remain explicit.         |
+---------------------------------+-----------+--------------------------------------+
| 0003 — Guidedoc core            | committed | The supported conversion contract is |
|                                 |           | implemented. RST conformance remains |
|                                 |           | conservatively described as partial. |
+---------------------------------+-----------+--------------------------------------+
| 0004 — Memory budgets and trust | committed | The Guidedog allocation budget and   |
|                                 |           | declared file-access boundary are    |
|                                 |           | implemented. Native libraries'       |
|                                 |           | internal allocations remain outside  |
|                                 |           | that budget.                         |
+---------------------------------+-----------+--------------------------------------+
| 0005 — Odin domain              | committed | Static Odin API extraction,          |
|                                 |           | references, and inventory support    |
|                                 |           | are implemented, with stated         |
|                                 |           | extraction limits.                   |
+---------------------------------+-----------+--------------------------------------+
| 0006 — Projects and publication | committed | The supported Sphinx-style project   |
|                                 |           | workflow and recoverable generation  |
|                                 |           | publication are implemented. Python  |
|                                 |           | extension execution and future       |
|                                 |           | plugins remain deferred.             |
+---------------------------------+-----------+--------------------------------------+

``XXXXX`` remains correct in the draft template, workflow examples, and tests of new
proposals. No active numbered record retains a draft number. A committed record describes
its supported contract; it does not turn its future-work section into an implemented
feature.

Defects, causes, and fixes
--------------------------

Preview changes could disappear
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The preview server remembered only the greatest modification time. A future-dated file
masked edits to other files. Deleting a page also left the greatest time unchanged, so the
deleted page continued to be served.

The watcher now fingerprints sorted path names, entry types, sizes, and modification times.
Removing a file changes the fingerprint. It watches declared external source, template,
static, include, autodoc, and locale roots. It excludes generated output. A macOS test also
caught canonical directory aliases: ``/var`` and ``/private/var`` must not defeat the output
exclusion. Traversal now keeps paths in the configured namespace.

The old binary reproduced both failures: no rebuild after an edit beneath a future
timestamp, and HTTP 200 for a deleted page. The fixed binary rebuilds and returns HTTP 404.
Tests cover unchanged input, generated-output exclusion, and external-root removal. The
watcher intentionally uses metadata; an edit that preserves both file size and timestamp is
outside this mechanism.

A failed Typst root could broaden file access
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

When the restricted temporary root could not be prepared, the builder fell back to a common
ancestor. That ancestor could include unrelated files. This made an operational failure
weaken the access boundary.

Root preparation now returns a checked problem and stops compilation. It does not substitute
a broader root. A test forces partial link preparation to fail and checks that the temporary
root is removed while the original sources remain. Windows users receive concrete
symlink-permission directions. This closes the fallback; it does not impose a CPU or memory
limit on native Typst itself.

Cleanup differed by platform and could follow the wrong root
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

On macOS, ``guidedog clean docs/gds`` failed on ``.DS_Store`` with “Not a directory.” Core
``remove_all`` had different behavior for ordinary files and top-level links. Its recursive
path construction was also a poor boundary for allocation refusal and deep directory trees.

All project-generation cleanup now uses one checked, iterative postorder work list. Files
and links are unlinked directly. Directories are removed after their children. The
configured clean root is checked without following a link; a symlink root is refused. Every
work-list growth and joined child path is checked. Regression tests cover files, nested
directories, outside-directory symlinks, root links, and refusal of each allocation used by
cleanup. Outside files survive every case.

Embedded Typst had a different clock contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The embedded backend ignored ``SOURCE_DATE_EPOCH`` and discarded time of day before applying
offsets. Reproducible documents could therefore differ between external and embedded Typst,
and an offset near midnight could produce the wrong date.

The adapter now captures one instant per compile. It retains the UTC time and local date. A
valid reproducible epoch fixes the automatic date to UTC, matching the CLI. Epoch seconds
are checked against Odin's nanosecond range before multiplication. The GDS date path uses
the same range restriction. Offset addition in the native adapter is checked rather than
allowed to overflow.

The private C bridge is ABI 3. It checks the version prefix before borrowing the larger
request, so an old short request is rejected safely. A Rust test covers this boundary.
End-to-end date tests run against both backends, including offsets crossing midnight and an
unrepresentable offset. Public Odin APIs are unchanged.

A verification tool could conceal differences
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The Sphinx comparison tool assumed page paths had the same form as its root. Relative roots
and absolute directory entries could yield empty page paths and zero extracted IDs.
Diagnostic paths also differed between absolute and working-directory-relative reports.

The tool now resolves the page root before traversal and normalizes diagnostic paths
lexically. Tests require a real page ID to survive a relative-root comparison. The pinned
Sphinx corpus was compared again after this fix. Remaining differences are reported, not
erased or counted as universal compatibility.

Some corrections were misleading
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Several runtime hints used ``--budget=MIB``, although a user needed a concrete next action.
Some suggested increasing the budget for a renderer retry limit or for native Typst memory
that the Guidedog budget cannot control.

Hints now identify the responsible limit. Core embedding errors direct the caller to its
resource provider. Renderer retry exhaustion suggests simplifying the source or reporting
the reproducer. Native compiler launch failures point to the executable, permissions, and
system memory. Runtime hint tests reject a placeholder ``--budget=MIB``; help and
documentation still use placeholders to explain command syntax.

A refused copy of the compiled PDF was also reported as a compiler failure. It now returns a
Guidedog memory problem, exit 3, with numeric budget advice. The regression test uses a
genuinely refused ledger charge, rather than only setting an error flag.

Lifecycle evidence became too small to read
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

GDS lifecycle reasons were emitted as one long line in an RST literal block. The PDF
template fitted that line by reducing its type size. The generator now wraps at spaces
without cutting words, paths, escapes, or UTF-8 code points. A test checks preserved words
and an unbroken Unicode token. Existing audit entries were reflowed without changing their
decisions. The final GDS books retain readable code typography.

Architecture and Odin API assessment
------------------------------------

The conversion core remains a useful small boundary. Callers provide storage, explicit
resource callbacks, and typed options. Conversion returns structured status and diagnostics.
The core does not import files or execute project configuration. Host packages handle
filesystem access, allocators, compiler invocation, and publication. This is consistent with
the simple procedural API the project wants: named operations, explicit data, and visible
ownership rather than hidden object lifetimes.

The fixes preserve that split. Watcher state stays private to the host. Typst root
preparation returns a host problem. Output cleanup is shared by publication and clean. The
embedded clock change crosses only the private C binding. No Python runtime or repository
Python pipeline was introduced. Temporary verification scripts are outside the source tree.

The strongest parts are the caller-buffer core, declarative configuration, static API
extraction, structured diagnostics, pre-allocation charging, and publication of complete
generations. The main remaining architectural limit is native work inside the process.
Graphviz, Tree-sitter, and embedded Typst can allocate independently. A Guidedog ledger
cannot make those implementations obey its budget. Worker isolation and operating-system
limits are the appropriate next boundary for hostile or multi-tenant workloads.

The source checker covers implementation rules, not prose layout. RST and Typst paragraphs
retain their own structure. The final implementation check reports zero violations across
653 files.

Verification and evidence
-------------------------

Heavy compilation and execution took place in an isolated checkout at
``insan@10.175.52.18:/home/insan/gd-beta-20261011``. The base commit is ``e00b667``; the
candidate includes the uncommitted fixes from this review. Linux uses Odin
``dev-2026-09-nightly:a2fb372`` and Typst 0.15.1. Focused macOS checks use the installed
October 2026 Odin toolchain.

Final results and raw-log locations are recorded below. Local retained logs are under
``build/review-beta-20261011/logs/``; visual checks are under its ``qa/`` directory. These
directories are generated evidence, not required runtime dependencies.

AddressSanitizer instruments the Odin code in these runs. It is not a complete sanitizer
audit of the native libraries' own internals. The system Pango plugin suppression remains
documented in ``tools/asan.supp``; the embedded candidate uses static Graphviz without that
plugin. The separate Rust bridge test checks the old ABI prefix boundary.

One concurrent sanitizer run reported an empty captured stderr for quickstart allocation
refusal 34, although the child exited with memory status 3. The isolated test passed, 1,000
independently captured runs of that exact case produced complete reports, and a full
concurrent repeat passed without a production-code change. The original observation remains
in ``asan-integration-header-observation.log``; it was not established as a CLI defect. The
refusal harness now captures to files and reads them after waiting for the child, retaining
the same exit and diagnostic assertions. This makes a missing CLI report fail independently
of live pipe polling. Final suite results below use that capture path.

.. list-table::
   :header-rows: 1
   :widths: auto

   * - Check
     - Result
     - Retained evidence
   * - Ordinary libraries and tools
     - All 29 suites pass with allocation tracking and failure on bad memory.
     - ``test-*.log``
   * - Embedded Typst and static Graphviz
     - All 29 library and tool suites pass with the same memory checks.
     - ``embedded-lib-*.log``, ``embedded-tools-*.log``
   * - Final integration candidate
     - All 166 tests pass for ordinary, embedded, and AddressSanitizer binaries.
     - ``integration.log``, ``embedded-integration.log``, ``asan-integration-final.log``
   * - Library and tool AddressSanitizer
     - All 26 library suites and three tool suites pass; affected packages were rerun after
       their final changes. The final project suite has 57 tests.
     - ``asan.log``, ``asan-final-affected.log``, ``asan-gds-final.log``,
       ``asan-project-final.log``
   * - macOS
     - Focused watcher, cleanup, root, clock, metadata, refusal, and capture checks pass;
       metadata/refusal checks also pass with AddressSanitizer.
     - ``mac-*.log``
   * - Rust bridge
     - The old-short-request ABI regression test passes.
     - ``bridge-test.log``
   * - Windows
     - Ordinary and embedded configurations type-check. No Windows runtime test was run.
     - ``windows-check-final.log``, ``windows-embedded-check-final.log``
   * - Implementation rules
     - 653 source files checked; zero violations. ``git diff --check`` also passes.
     - ``rules.log``
   * - GDS and API
     - Native strict HTML and PDF builds pass. GDS: 16 HTML pages, 1,030 internal links;
       API: 30 pages, 19,398 links. No missing files or anchors in either.
     - ``gds-*-final.log``, ``gds-links.log``, ``api-*-final.log``
   * - Book inspection
     - Six GDS books have 15, 18, 29, 14, 9, and 21 pages. The API book has 309. Fonts are
       embedded; no text lies outside page bounds. Sampled rendered pages and the GDS HTML
       header were inspected.
     - ``qa/pdf-qa.json``, ``qa/api-qa.json``, rendered contact sheets
   * - Allocation-before-expansion repro
     - The 60-million-character template with ``--budget=4`` exits 3 at 14,008 KiB peak RSS,
       about 13.7 MiB. Its report identifies ``_templates/layout.html:1`` and suggests
       ``--budget=256`` or ``--memory=disk``.
     - ``budget-rss.log``, ``budget.err``
   * - Sphinx comparison probe
     - 43 pinned roots attempted; 41 compare, two fail in the reference build. Remaining
       documented differences persist. The maxdepth diagnostic difference improves from four
       to zero.
     - ``sphinx-normalized.txt``, ``evidence-summary.json``

The Sphinx probe uses Sphinx 9.1.0, Docutils 0.22.4, and Pygments 2.19.2 only as a test
reference. Guidedog's documentation builds do not run that Python toolchain. This comparison
is separate from the native RST and MyST conformance fixtures, whose remaining differences
are retained in the test output.

The exact final ordinary binary was copied once for the large workload run. Its SHA-256 is
``86e94a2324ccceff7023f070c37e80d04a22bb532ae82477202b091d13bbe7e6``; the embedded binary is
``2d0fdbc3bf7036b3b6b6d8dc57c80165554b499c3349d8dd5d3fa505d121ea4e``. These are Linux review
binaries, not published distribution archives. ``candidate-binaries.sha256`` and
``manuals-binary.sha256`` record the match.

============== ========== ============== ========= ===============================
Large workload HTML pages Internal links PDF pages Fresh and unchanged builds
============== ========== ============== ========= ===============================
CPython        589        178,783        5,821     HTML and PDF both exit 0 twice.
Django         678        245,481        2,580     HTML and PDF both exit 0 twice.
Flask          79         8,539          346       HTML and PDF both exit 0 twice.
============== ========== ============== ========= ===============================

All twelve builds use ``-j auto``. Every diagnostic severity/code count matches the
project's existing TSV baseline. All six unchanged rebuilds preserve every output file's
path, size, inode, and modification time. Django and Flask have no missing internal targets.
CPython retains two known missing files, ``lifecycle.dot.css`` and ``PAGEURL``, and no
missing anchors. These are reported limitations of the imported fixture, not clean strict
builds. Raw evidence is in ``{project}-{builder}-{1,2}.{out,err,time,files}``,
``{project}-linkcheck.txt``, and ``{project}-pdf-info.txt``; ``evidence-summary.json``
records the baseline and unchanged-output comparisons.

One intermediate sanitizer retry lacked the required Odin/Typst ``PATH`` and is excluded
(``asan-integration-repeat.log``). The correctly configured repeat and the final candidate
run pass. Failed and invalid intermediate logs remain available rather than being
overwritten as successful evidence.

A build exiting zero does not mean an imported Sphinx project has no diagnostics. The
permissive build mode deliberately retains unsupported source and reports it. Strict
``-n -W`` builds of Guidedog's GDS and API projects have no errors or warnings. The CPython
fixture still contains unsupported project-specific extension directives. Django and Flask
also retain their recorded source diagnostics. Their exact severity/code counts are compared
with the existing TSV baselines.

The earlier “29 killed publications,” “101 of 101 builds,” and all-platform sanitizer
statements are historical results. This review does not claim to have repeated those exact
stress experiments. It runs the retained regression suites, fresh large builds, unchanged
rebuilds, targeted failure reproducers, and the final candidate checks listed above. Neither
this review nor AddressSanitizer proves that no crash can ever occur.

Release scope and remaining gates
---------------------------------

Ship the beta with an explicit support statement: trusted projects on macOS and Linux,
documented native RST and Markdown authoring, and either a supported installed Typst
compiler or the matching embedded bridge. Prefer the static Graphviz build, which avoids the
known system Pango plugin defect. Windows remains experimental until the disk-memory,
symlink, publication, and thread-crash paths run on Windows. Python-extension parity is not
a release gate.

The embedded build is not a C-runtime-only executable. Linux dependency inspection still
shows libcurl, the C++ runtime, and their transitive dependencies. The README now states
this. Before distributing a production binary, test the exact archive on a clean supported
machine, verify native-library and font availability, and record its hash and toolchain.
Source-suite success cannot substitute for that packaging check.

Do not advertise ``--untrusted`` as a general sandbox for arbitrary native input. It narrows
file access and project capabilities. It does not bound all native allocations or execution
time. Exposed build services need process isolation and system resource limits.

GDS 0002's broader worker and native-Typst chapter obligations remain open. An actual
issue-tracker URL should be supplied through ``GUIDEDOG_ISSUE_TRACKER`` for distributed
builds; this checkout has no configured remote from which to derive one. The reviewer did
not invent a URL, tag a release, or publish an artifact.

Within this scope, the beta is justified. An unqualified “everything is finished and
production-safe on every platform” statement is not justified by the implementation or
evidence.
