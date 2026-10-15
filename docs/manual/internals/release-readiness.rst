Readiness for 1.0.0 beta
========================

This page records what we checked before calling Guidedog a 1.0.0 beta, what we measured, and what is still open. It describes the state of 15 October 2026, at version 1.0.0-beta.1.

What a beta promises
--------------------

A beta promises that the features are complete and that the interfaces we name below will not change without a reason recorded in the changelog. It does not promise that every platform and every project has been tried. The open items at the end of this page say where that is so.

The interfaces we hold stable from the beta on are these: the command line of ``guidedog`` and its plugin commands, the configuration names of ``conf.toml``, the plugin process protocol 1.0, the diagnostic codes, and the HTML and Typst output structure that themes and templates rely on. The Rust library crates are published for reuse, but their API may still change before 1.0.0.

What we verified
----------------

.. list-table::
   :header-rows: 1
   :widths: 34 66

   * - Check
     - Result
   * - Tests
     - All 1,893 tests of ``cargo test --workspace`` pass; 4 are ignored.
   * - Lint and limits
     - ``cargo clippy --workspace --all-targets`` is clean. ``cargo run -p check`` finds no violations of the function, file and line limits. ``cargo fmt --check`` is clean. The 9 generated tables match ``tools/data``.
   * - Docutils
     - ``cargo xtask verify-docutils`` regenerated all 98 reference trees and ``ids.txt`` with a pinned Docutils 0.22.4. All are identical to the committed fixtures.
   * - Sphinx
     - ``cargo xtask sphinxdiff-roots`` built 42 of Sphinx 9.1.0's test roots (43 builds) with Sphinx and with Guidedog. Every difference is explained in :doc:`sphinx-comparison`. The run found one defect, which we fixed: errors in a document's content were reported as warnings.
   * - Sphinx extensions
     - All of Sphinx 9.1's built-in extensions are handled without running Python (GDS 0011, "Sphinx's built-in extensions"). napoleon converts all 148 docstrings of Sphinx's own tests as Sphinx does; the test roots of napoleon, autosummary, apidoc, coverage and imgconverter come out identical in what the comparison checks. ``doctest`` shows its examples and does not run them.
   * - CommonMark and MyST
     - All 652 CommonMark 0.31.2 examples pass. 215 of 218 MyST reader fixtures match.
   * - Plugins
     - ``cargo xtask release plugin-compatibility`` ran the 0.6.0 release's host with our plugins and our host with the 0.6.0 release's plugins. Five pairings passed, none failed. One was skipped because no 0.6 release ships a build plugin.
   * - Real projects
     - We built the manuals of CPython, Django and Flask as HTML and PDF (:doc:`../evidence/manuals`). The runs found one defect, which we fixed: a build kept only 256 diagnostics of one document, and CPython's ``os.rst`` has 272.
   * - Release programs
     - ``cargo xtask release smoke``, ``smoke-discussion`` and ``smoke-pages`` passed with a minimal environment.
   * - Documentation
     - The manual, the GDS catalog, the site and the Rust API build with ``-W`` and no warnings. ``cargo xtask build-site`` builds the site in 7 languages with its PDF books.
   * - Packages
     - Each of the 26 publishable crates carries its README, ``LICENSE`` and ``COPYING``. All 26 names are free on crates.io.
   * - Licence
     - Guidedog is released under the LGPL-3.0-or-later. NOTICE grants an additional permission to link Graphviz, which is under the Eclipse Public License 2.0.

What we measured
----------------

The Linux x86-64 release program ``guidedog`` is one file of 90.2 MB. It embeds Typst, links Graphviz statically and speaks HTTPS through rustls. On Linux it needs only the C library, ``libm`` and ``libgcc_s``.

The manual (85 documents) builds in 0.52 s from a cold start and in 0.13 s when nothing changed. CPython's manual (589 pages) builds as HTML in 1.24 s with a peak of 219 MB, Django's (678 pages) in 2.66 s with 127 MB. Of the 16 pairs of reader and renderer, 10 allocate nothing when they convert the same text again, and the others allocate one to five times.

What is open
------------

Decisions for the maintainers
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#. **Scope.** Guidedog supports all of reStructuredText and all of Sphinx's built-in extensions, in its own way. Extensions a project writes itself (CPython's, Django's ``djangodocs``) and third-party ones (``sphinx-tabs``, Pallets) are outside the beta; their directives are reported where they occur.

Work not yet done
~~~~~~~~~~~~~~~~~

#. **Large books.** The CPython book did not fit a machine with 12 GB of memory: Typst lays out the whole book in one compilation. CPython splits its PDF into about 40 books, which ``conf.py`` computes; declared as ``pdf_documents``, each is much smaller. Guidedog now empties Typst's cache after every book.
#. **Unchanged builds.** An unchanged build of CPython's manual takes 0.92 s and 170 MB, close to the 1.24 s of a clean build. Most of that time is checking and loading what the last build stored. We should profile it.
#. **Translations.** The six translations were made by machine with a glossary. A native speaker of each language should read them before we call them final.

Known limits
~~~~~~~~~~~~

#. The reStructuredText writer does not round-trip every document: all 65 manual pages come back the same, but only 77 of 98 corpus sources and 185 of 202 MyST sources do.
#. With static Graphviz, PNG output is unavailable, and ``sfdp`` lays out without its triangulation library.
#. An incremental build does not notice a new Rust module file under ``target/`` or a hidden folder. A clean build does.
#. The memory budget is coarse. It counts what the host holds, not what Typst, Graphviz or tree-sitter allocate.

Recommendation
--------------

We published ``v1.0.0-beta.1`` after source qualification, platform archive smoke tests and Homebrew checks. :doc:`../evidence/release-1.0.0-beta.1` records the release, download checksums and automatic tap update. The work not yet done can follow in later betas.
