Rust 1.100, Graphviz 16.1.1 and allocation measurements
=======================================================

On 13 November 2026 we ported Guidedog to Rust 1.100.0 and pinned the packaged Graphviz runtime to 16.1.1.
We measured compiler and allocator variants before choosing to retain the default process allocator.
These changes are unreleased; the published beta.12 archives still contain their original toolchain and libraries.

What changed
------------

``rust-toolchain.toml`` now selects Rust 1.100.0, including rustfmt and Clippy.
Every workspace crate inherits a minimum supported Rust version of 1.100 from the root manifest.
The edition remains 2024, the Rust dependency lockfile is unchanged, and we added no external Rust dependency.

Clippy 1.100 identified three index loops that can use iterators: included-source positions, bracket scanning in highlighting, and grid-cell overlap tracking.
We changed their traversal without changing their intended document semantics.
Cargo's new unused-dependency diagnostic also showed that the examples' Graphviz dependency belongs in ``dev-dependencies``.

The complete test run found an additional compatibility issue.
Cargo 1.100 places test executables under a different intermediate directory, so three helpers that walked two parents could no longer find programs and examples.
We now locate the public outputs in ancestors of the executable without assuming an intermediate depth.
The manual's custom-plugin test builds a separate crate with Cargo, offline, and reads its executable path from Cargo's JSON artifact messages.
It no longer locates dependency ``.rlib`` files by hand.

The source build instructions, seven README editions, six translated installation catalogs, release workflow, Windows runtime notices and downloadable ``NOTICE`` copy follow the new pins.
GDS 0012's toolchain requirement is current.
The runtime API and allocator policy are unchanged.

What Rust's allocator API changes
---------------------------------

Rust 1.100 stabilizes the core ``std::alloc::Allocator`` interface and allocator-aware operations on ``Vec`` and ``Box``.
We compiled and ran a stable-toolchain probe using ``Vec::with_capacity_in``, ``Vec::new_in`` and ``Box::new_in`` with ``System`` and ``Global``.
No nightly feature was required.
Both vectors occupied 24 bytes on this x86-64 host, but that observation does not establish the size of a vector holding another allocator type.

The new API lets a collection choose its allocator.
It does not replace the default allocator or supply an automatically faster allocation algorithm.
Applications could already select a process allocator through ``GlobalAlloc`` before this release.
``Global`` follows that choice; ``System`` explicitly selects the platform allocator.

Guidedoc already retains its node tables, text pool, output and reader scratch storage in a workspace.
The allocation regression test passed for all sixteen reader/renderer pairs without raising any ceiling.
Its repeated HTML and Typst conversions through all four readers allocate zero times for the recorded samples.
This is a sample-based regression guarantee, not a claim that project loading, every possible document or Typst PDF compilation allocates nothing.

Making workspace tables allocator-generic would change public types and spread allocator parameters through readers and renderers.
It would not control ordinary ``String`` storage or the allocations inside Typst and Graphviz.
We found no measured benefit that justifies that change, and we added no allocator-related unsafe code.
The host memory budget remains a separate accounting mechanism.

Measurement conditions
----------------------

We used a Linux x86-64 host with an AMD Ryzen 9 5950X, kernel 7.0.0-28-generic and glibc 2.41.
Both compiler builds used the same runtime source, lockfile and release profile: thin LTO and four codegen units.
The baseline compiler was ``rustc 1.99.0 (b940084d7 2026-09-28)``; the new compiler was ``rustc 1.100.0 (0ef1bc386 2026-11-09)``.
The baseline build used ``--ignore-rust-version`` solely to build the comparison after raising the workspace minimum.

We pinned the benchmark driver and its children to CPU 8, rotated variant order and ran no competing build or test suite during timing.
The site builds used one reading worker and a fixed publication clock.
This is a local comparison with CPU frequency scaling enabled, not a cross-platform performance guarantee.

All compiler comparisons linked the same installed Graphviz 2.42.4 shared library.
The synthetic performance fixtures contain no Graphviz directives.
We qualified the new static Graphviz runtime separately, so no timing below attributes a compiler effect to a Graphviz upgrade.

We tested jemalloc 5.3.0 through ``LD_PRELOAD`` on the Rust 1.100 binaries, with no ``MALLOC_CONF`` tuning.
This is a Linux process-allocator experiment, affecting C allocations as well as Rust's default allocations.
It is not a benchmark of a new per-collection allocator implementation.

The in-process example excludes file loading and the initial warm conversion from the timer.
Each result uses fifteen batches per mode and variant.
The small RST batches contain 10,000 conversions each; the larger RST and Markdown batches contain 150 each.
Fresh mode includes workspace creation and destruction.
Reused mode retains its buffers.

Whole-command results include process startup, file access and publication.
We used 31 small-command samples, 21 Markdown samples and nine samples per site build mode and variant.
The first RST and PDF runs were noisy, so we preserved them and ran confirmations with 61 RST and 31 PDF samples per variant; the tables use those confirmations.
The 1,000-article site also contains its index page, feeds, search, taxonomy and pagination outputs.
Each cold build starts with no saved Guidedog environment or outputs; the operating system's file cache remains warm.
Each unchanged build writes zero outputs.
Each body edit reads one document and writes two outputs, the article and search index.
Those counts were asserted on every sample.

Compiler results
----------------

The following whole-command values are medians in milliseconds.
Negative change means less elapsed time.

.. list-table::
   :header-rows: 1
   :widths: 4 2 2 2

   * - Workload
     - Rust 1.99
     - Rust 1.100
     - Change
   * - 362-byte RST to HTML
     - 21.486
     - 21.309
     - -0.82%
   * - 131,231-byte RST to HTML
     - 25.570
     - 25.935
     - +1.43%
   * - 69,020-byte Markdown to HTML
     - 26.879
     - 27.082
     - +0.76%
   * - 35,358-byte RST to PDF
     - 100.889
     - 100.563
     - -0.32%
   * - Cold 1,000-article site
     - 1565.725
     - 1546.802
     - -1.21%
   * - Unchanged site
     - 590.712
     - 583.142
     - -1.28%
   * - One article body edit
     - 749.002
     - 741.706
     - -0.97%

The warmed in-process RST conversion took 11.650 microseconds with Rust 1.99 and 11.679 with Rust 1.100 for the small input.
The large RST took 3.162 and 3.182 milliseconds, a 0.63% increase.
The large Markdown input took 1.570 and 1.542 milliseconds, a 1.76% decrease.
Fresh-workspace results follow the same general pattern.
We found no substantial general speedup from the compiler change alone.

We also computed paired bootstrap intervals from 5,000 resamples of the recorded trial pairs, with a fixed random seed.
The 95% interval for the command-level RST change spans -0.29% to +8.65%, and the PDF interval spans -2.46% to +0.45%.
Those samples do not distinguish a compiler regression or improvement from the variation in process startup and file publication.
For unchanged-site latency the interval spans -1.86% to -0.68%, a small improvement in this test.
These intervals describe our sample and do not account for every source of host or workload variation.

Allocator results and decision
------------------------------

The following medians compare the same Rust 1.100 executable with its normal allocator and jemalloc.
Elapsed values are milliseconds.

.. list-table::
   :header-rows: 1
   :widths: 4 2 2 2

   * - Workload
     - Default
     - jemalloc
     - Change
   * - Small RST command
     - 21.309
     - 22.234
     - +4.34%
   * - Large RST command
     - 25.935
     - 26.385
     - +1.74%
   * - Large Markdown command
     - 27.082
     - 29.228
     - +7.92%
   * - PDF command
     - 100.563
     - 101.664
     - +1.09%
   * - Cold site
     - 1546.802
     - 1496.133
     - -3.28%
   * - Unchanged site
     - 583.142
     - 545.201
     - -6.51%
   * - One body edit
     - 741.706
     - 692.483
     - -6.64%

The unchanged-site improvement has a bootstrap interval of -6.98% to -6.05%; the body-edit interval is -7.23% to -4.90%.
The small command's slowdown also persists in the sample, with an interval of +3.80% to +5.52%.
The command-level large-conversion and PDF intervals cross zero, so their median differences are not firm evidence of a speed change.

The site improvement costs memory.
Median per-process peak RSS, measured with Linux ``wait4``, rose from 40,064 to 48,620 KiB for unchanged builds, and from 40,724 to 49,176 KiB for edits.
Those are increases of about 21%, including process and library memory rather than just allocator-owned buffers.
The confirmed PDF samples rose from 35,272 to 38,244 KiB.

Jemalloc also reduced fresh-workspace large-RST conversion by 2.98%, but its reused-workspace result changed by only +0.06%.
Its reused small-RST and Markdown confidence intervals cross zero.
We therefore retain the default allocator and the existing reuse design.
The large-site result is useful evidence for an application's own memory/latency tradeoff, but it does not justify imposing an allocator on every library consumer or every platform.
We added no production allocator dependency, feature switch or hidden global allocation policy.

Graphviz qualification
----------------------

Graphviz 16.1.1 was published on 8 November 2026.
Its relevant fixes include cluster-name collisions that could overrun the heap, a rank-zero labeled-edge crash, and several other cluster, routing and ortho crashes.
The upstream patch also fixes a GDI+ font lookup, but Guidedog's Windows package omits that optional renderer.
We treat this upgrade as a correctness and robustness improvement, not a demonstrated diagram-speed improvement.

We verified both downloaded archives against their upstream SHA-256 files:

.. code-block:: text

   graphviz-16.1.1.tar.xz
   52e7fb9a57b52b5c67f41dba6504ba3231e118258779421cb8f4f99104de71f6

   windows_10_cmake_Release_Graphviz-16.1.1-win64.zip
   72b1f3a9a1fd3f6e726478c1e044a95483a8fbddec13ba2fb395438d75d001b4

The ``textspan.h``, ``textspan_lut.h`` and ``gvplugin_textlayout.h`` headers are byte-identical to 16.1.0.
That preserves the structs and width-estimation entry point our static text-layout plugin uses.
The static build retains its eight layout engines and SVG/DOT renderers, with Expat 2.7.1.
It adds no PNG renderer.

All 29 static Graphviz unit tests and its doctest passed, including new regressions for the cluster/node name collision and rank-zero flat edge.
The Rust 1.100 static executable reports ``graphviz 16.1.1 (static)``.
Its fresh-environment smoke tests passed HTML, PDF, Graphviz and Typst diagrams, music, discussions, static-site generation and Pages commands.
All eighteen DLLs required by our Windows packager exist in the verified archive.
Inspection of their PE import tables found no missing non-system runtime dependency.
We did not execute Windows or macOS binaries on this Linux host; native platform CI remains necessary before a release.

Verification and reproduction
-----------------------------

The complete workspace run passed 2,216 tests across 212 reports, with five ignored tests.
Strict Clippy, formatting, generated-table verification and the pinned Bravura check passed.
The checker examined 1,263 implementation files with zero hard violations and six existing soft-width notices.
All 66 music scores passed geometry, SVG and PNG checks.
The deterministic 12,000-case fuzz run parsed 4,186 cases and engraved 3,822.
The complete local website build passed all seven HTML and PDF manual editions and all eighteen discussion PDFs with the new static executable.
Its link checker examined 951 pages and 212,837 internal links, with zero missing files or anchors.
All 120 manual translation catalogs have zero fuzzy or untranslated entries, and the installation pins are correct in every rendered language.
Refreshing the catalogs produced identical message/translation pairs; we kept the existing formatting and relative source references.

The exact conversion inputs are in :download:`rust-1.100-inputs.tar.gz`.
The four files have 362, 131,231, 69,020 and 35,358 bytes respectively for small RST, large RST, Markdown and the PDF source.
The raw records preserve the initial and confirmation runs separately:

- :download:`rust-1.100-core.csv`: 270 in-process batch records, with iterations and byte counts.
- :download:`rust-1.100-response.csv`: 333 initial whole-command records, with peak RSS.
- :download:`rust-1.100-confirmation.csv`: 276 confirmation records for RST and PDF.
- :download:`rust-1.100-comparisons.json`: medians, changes and bootstrap intervals.

For example, after unpacking the inputs we can run the new example with either compiler:

.. code-block:: sh

   cargo +1.99.0 build --ignore-rust-version --locked --release \
       --target-dir /var/tmp/guidedog-199 -p guidedoc-examples \
       --example measure_conversion
   cargo +1.100.0 build --locked --release \
       --target-dir /var/tmp/guidedog-100 -p guidedoc-examples \
       --example measure_conversion
   /var/tmp/guidedog-100/release/examples/measure_conversion \
       small.rst rst html 10000 15
   /var/tmp/guidedog-100/release/examples/measure_conversion \
       large.rst rst html 150 15
   /var/tmp/guidedog-100/release/examples/measure_conversion \
       large.md commonmark html 150 15

For command timings we use separate executable copies, ``convert --force --offline`` and, for PDF, ``--embedded-fonts-only``.
The site fixture follows ``crates/gd-project/tests/ssg_benchmark.rs``: 1,000 dated RST articles, each with the sentence ``We explain one observation and show the evidence.`` repeated in 100 paragraphs.
We build it with ``-j 1 --now=2026-11-13T00:00:00Z -W``, clearing its output and adjacent ``.output.guidedog`` cache before each cold sample.
For edits we append ``One body edit.`` to article 0500 after its unchanged build.
An application may reproduce the allocator experiment with ``LD_PRELOAD=/path/to/libjemalloc.so.2`` on Linux; that library is not part of Guidedog.
We compare byte-identical HTML conversion outputs across variants and assert the site read/write counts before accepting timing samples.

References
----------

- `Rust 1.100 announcement <https://blog.rust-lang.org/2026/11/12/Rust-1.100.0/>`_.
- `Rust release notes <https://dev-doc.rust-lang.org/releases.html#version-11000-2026-11-12>`_.
- `Allocator interface <https://doc.rust-lang.org/stable/std/alloc/trait.Allocator.html>`_.
- `Graphviz 16.1.1 release <https://gitlab.com/graphviz/graphviz/-/releases/16.1.1>`_.
- `Graphviz 16.1.1 changelog <https://gitlab.com/graphviz/graphviz/-/blob/16.1.1/CHANGELOG.md>`_.
