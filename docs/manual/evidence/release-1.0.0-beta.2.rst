Release 1.0.0-beta.2
====================

On 16 October 2026 we published `v1.0.0-beta.2 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.2>`_ as a normal GitHub release and marked it latest. The annotated tag names commit ``3fbd47a49ad477012f536dc1effd219e49797961``.

PDF pagination
--------------

This release fixes `issue 1 <https://github.com/insanai/guidedog/issues/1>`_. The review also found the same defect in captioned code listings. The shared renderer now lets both kinds of figure continue across pages. Existing copied book templates and standalone conversion inherit the fix. The :doc:`pdf-pagination` record explains the root cause and regression coverage.

We downloaded the published Linux x86-64 executable and built fresh PDFs with a 120-row captioned table, a 120-line captioned listing and the table with a copied book template. PyMuPDF 1.28.2 found every entry exactly once at a distinct, non-overlapping position. Both tables span three content pages with repeated headers. The listing spans two content pages.

Source and package qualification
--------------------------------

The `Verify run <https://github.com/insanai/guidedog/actions/runs/38829643134>`_ passed on Linux and macOS. Formatting, source limits, the workspace build and Clippy passed. The workspace suite passed 1,896 tests with 4 ignored, including pagination checks against the embedded compiler's laid-out pages.

The `release run <https://github.com/insanai/guidedog/actions/runs/38830356245>`_ reused verification for that exact commit. All 16 jobs passed. It built native archives for Linux x86-64 and arm64, macOS Intel and Apple silicon, and Windows x86-64. Every archive passed its checksum, version, HTML, PDF, discussion and Pages checks. Homebrew installation and formula tests passed before publication.

Downloads
---------

We downloaded all ten assets and verified their GitHub API digests. All nine files listed in ``SHA256SUMS`` match the published manifest. Each platform archive contains one executable. The Linux executable reports ``guidedog 1.0.0-beta.2``, static Graphviz and embedded Typst. The Windows ZIP contains ``guidedog.exe`` with its runtime DLLs beside it.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   8c81732a76b803266a1afc5d20e32611b43bff23076bb39275aa66e7a7f0c3f1

At publication, the example in :doc:`../howto/github-pages` used this version and checksum. That example now follows the current release.

Homebrew and documentation
--------------------------

The tap's `automatic import <https://github.com/insanai/homebrew-guidedog/actions/runs/38833083612>`_ selected 1.0.0-beta.2 without an explicit tag. It verified the release metadata, manifest and archive hashes, then installed and tested the formula on Apple silicon and Intel before committing it. The tracked formula is identical to the published ``guidedog.rb``. Hourly scheduled updates remain enabled and reject downgrades.

We rebuilt the seven language editions of the website and their PDF books with the downloaded release executable. The local site check covered 656 HTML pages and 167,498 internal links, with no missing files or anchors.
