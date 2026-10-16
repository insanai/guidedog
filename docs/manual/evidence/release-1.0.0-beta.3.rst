Release 1.0.0-beta.3
====================

On 16 October 2026 we published `v1.0.0-beta.3 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.3>`_ as a normal GitHub release and marked it latest. The annotated tag names commit ``e5dce6f662b7dcfec23717f21269022b6fbc0d1c``.

Translation and index fixes
---------------------------

This release fixes `issue 2 <https://github.com/insanai/guidedog/issues/2>`_. Translated glossary terms retain their generated index metadata in website and PDF indexes. The review also found and fixed reused inline index identities across independently translated paragraphs. The :doc:`translation-index-review` record explains both root causes, the shared fixes, regression coverage and the wider review scope.

We downloaded the published Linux x86-64 executable and checked fresh translated builds, cached rebuilds and edited catalogs in Japanese, Chinese, Korean, Hindi, Arabic and German. All three glossary entries remained indexed, both inline index anchors appeared exactly once, and term links kept their original targets. The checks used real gettext extraction, ``intl update`` and MO compilation. A German PDF passed checks of the generated index labels and visible index text.

We also repeated the earlier pagination checks with a 120-row captioned table and a 120-line captioned listing. PyMuPDF 1.28.2 found every entry exactly once at a distinct, non-overlapping position across multiple pages. Table headers repeated on every table page.

Source and package qualification
--------------------------------

The `Verify run <https://github.com/insanai/guidedog/actions/runs/38867324126>`_ passed on Linux and macOS. Formatting, source limits, generated tables, the workspace build, Clippy, examples and CLI/plugin smoke tests passed. The local workspace suite passed 1,905 tests with 4 ignored.

The `release run <https://github.com/insanai/guidedog/actions/runs/38871743797>`_ reused verification for that exact commit. All 16 jobs passed. It built native archives for Linux x86-64 and arm64, macOS Intel and Apple silicon, and Windows x86-64. Every archive passed its checksum, version, HTML, PDF, discussion and Pages checks, including the new translated-glossary and inline-index regression. Homebrew installation and formula tests passed before publication.

Downloads
---------

We downloaded all ten assets and verified their GitHub API digests. All nine files listed in ``SHA256SUMS`` match the published manifest. Each platform archive contains one executable. The Linux executable matches its build manifest and reports ``guidedog 1.0.0-beta.3``, static Graphviz and embedded Typst. The Windows ZIP contains ``guidedog.exe`` with its runtime DLLs beside it.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   387649a79809da941c04837f1c0883ced75f58a5374e2d7ff05e2df7d798c6ae

The example in :doc:`../howto/github-pages` uses this published version and checksum.

Homebrew and documentation
--------------------------

The tap's `automatic import <https://github.com/insanai/homebrew-guidedog/actions/runs/38874275274>`_ discovered 1.0.0-beta.3 without an explicit tag. It verified the release metadata, manifest and archive hashes, then installed and tested the formula on Apple silicon and Intel before committing it. The tracked formula is identical to the published ``guidedog.rb``. Hourly scheduled updates remain enabled and reject downgrades.

We rebuilt the website, seven manual language editions and their PDF books with the downloaded release executable. The root-cause review and release record are part of that documentation. The local site check covered 670 HTML pages and 168,548 internal links, with no missing files or anchors.
