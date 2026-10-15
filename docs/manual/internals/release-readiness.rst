Readiness for 1.0.0 beta
========================

This page records what we checked before calling the Rust implementation a 1.0.0 beta, what we measured, and what is still open. It describes the state of 15 October 2026, at version 0.7.0.

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
     - 1,817 tests pass, none fail, 4 are ignored (``cargo test --workspace``).
   * - Lint and limits
     - ``cargo clippy --workspace --all-targets`` is clean. ``cargo run -p check`` finds no violations of the function, file and line limits in 1,021 files. ``cargo fmt --check`` is clean. The 9 generated tables match ``tools/data``.
   * - Docutils
     - ``cargo xtask verify-docutils`` regenerated all 98 reference trees and ``ids.txt`` with a pinned Docutils 0.22.4. All are identical to the committed fixtures.
   * - Sphinx
     - ``cargo xtask sphinxdiff-roots`` built 42 of Sphinx 9.1.0's test roots (43 builds) with Sphinx and with Guidedog. The result agrees with the record of the Odin implementation on every root but one, where Sphinx itself changed. The run found one regression, which we fixed (:doc:`sphinx-comparison`).
   * - CommonMark and MyST
     - All 652 CommonMark 0.31.2 examples pass. 215 of 218 MyST reader fixtures match.
   * - Plugins
     - ``cargo xtask release plugin-compatibility`` ran the released 0.6.0 host with our plugins and our host with the released 0.6.0 plugins. Five pairings passed, none failed. One was skipped because no 0.6 release ships a build plugin.
   * - Release programs
     - ``cargo xtask release smoke``, ``smoke-discussion``, ``smoke-graphdog`` and ``smoke-pages`` passed with a minimal environment.
   * - Documentation
     - The manual, the GDS catalog, the site and the Rust API build with ``-W`` and no warnings. ``cargo xtask build-site`` builds the site in 7 languages with its PDF books (2,226 files). Every translation catalog has no fuzzy and no untranslated message.
   * - Packages
     - Each of the 26 publishable crates carries its README and the licence text. All 26 names are free on crates.io.

What we measured
----------------

The release program ``guidedog`` is one file of 88.9 MB. It embeds Typst, links Graphviz statically and speaks HTTPS through rustls. On Linux it needs only the C library, ``libm`` and ``libgcc_s``.

The manual (85 documents) builds in 0.52 s from a cold start and in 0.13 s when nothing changed. Of the 16 pairs of reader and renderer, 10 allocate nothing when they convert the same text again, and the others allocate one to five times.

The Rust sources have 200,913 lines of code, and the Odin sources had 138,243. We accounted for the growth: 12,100 lines of tests, 9,600 lines of new features (the reStructuredText writer, Rust API documentation, guided publishing to Pages), 5,300 lines that replace the Odin standard library and C code (JSON, SHA-256, inflate, an Odin parser), 6,900 lines from splitting the project into crates, and about 7,000 lines of verification tools in ``cargo xtask``.

What is open
------------

Decisions for the maintainers
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#. **Graphviz licence.** NOTICE grants an additional permission for Graphviz under EPL 1.0, but the Graphviz 16.1.0 we link statically is licensed under EPL 2.0. The wording must name the licence we ship.
#. **Licence of the reusable crates.** Every crate is AGPL-3.0-or-later. Libraries meant for other projects, such as ``gd-jinja``, ``gd-json``, ``gd-toml`` and ``gd-highlight``, are hard to reuse under the AGPL. We may want a permissive licence for them.
#. **Version.** We have to choose between ``1.0.0-beta.1`` and a plain ``1.0.0`` for the first release of the Rust implementation.

Work not yet done
~~~~~~~~~~~~~~~~~

#. **Real projects.** The builds of CPython, Django and Flask were last measured with the Odin implementation (:doc:`../evidence/manuals`). We must build them again with the Rust implementation and record the diagnostics, the timings and the memory use.
#. **Other platforms.** We checked that Windows and macOS compile. We did not run the tests or the release programs there.
#. **Credential store.** The store of ``guidedog-pages`` was not tried against a real keyring. ``cargo xtask release credential-store`` runs it against a private Secret Service, which this machine lacks.
#. **Translations.** The six translations were made by machine with a glossary. A native speaker of each language should read them before we call them final.

Known limits
~~~~~~~~~~~~

#. The reStructuredText writer does not round-trip every document: 69 of 71 manual pages, 77 of 98 corpus sources and 185 of 202 MyST sources come back the same.
#. With static Graphviz, PNG output is unavailable, and ``sfdp`` lays out without its triangulation library, as in the Odin release.
#. An incremental build does not notice a new Rust module file under ``target/`` or a hidden folder. A clean build does.
#. The memory budget is coarse. It counts what the host holds, not what Typst, Graphviz or tree-sitter allocate. The disk-backed working memory of the Odin implementation is gone.

Recommendation
--------------

We recommend tagging ``1.0.0-beta.1`` once the Graphviz licence wording is settled and the real projects have been rebuilt with the Rust implementation. The other items can follow in later betas.
