Release 1.0.0-beta.5
====================

On 16 October 2026 we published `v1.0.0-beta.5 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.5>`_ as a normal GitHub release and marked it latest. The annotated tag names commit ``83cc1ea2d9f703bd99c7c1e08ae5eb29bcca7d02``.

PDF theme and imported modules
-------------------------------

Books and discussion records share Guidedog's palette, fonts and language-aware typography through ``theme.typ``. Their layouts remain distinct. Books use centered covers and teal chapter openings. Discussion records use metadata panels, state badges, a status note and compact contents. Quickstart copies all three PDF modules, while discussion initialization copies its layout and the shared theme. Built-in fallback templates are self-contained.

The review also corrected PDF input tracking. Changes to imported modules and assets beneath ``_templates/`` now invalidate cached PDFs. The :doc:`pdf-theme-review` record describes the design, root cause, bounds and regression coverage. The manual explains how we adopt new defaults without overwriting existing customizations.

Qualification and downloads
-----------------------------

The `Verify run <https://github.com/insanai/guidedog/actions/runs/38897306182>`_ passed on Linux and macOS. The local workspace suite passed 1,910 tests with 4 ignored. The workspace build, Clippy, formatting, source limits, generated tables and discussion smoke test passed. Focused PDF checks covered both layouts, nested imports, shared-theme edits, missing modules and untrusted fallback.

All 16 jobs of the `release run <https://github.com/insanai/guidedog/actions/runs/38898463021>`_ passed. Native archives for Linux x86-64 and ARM64, macOS Intel and Apple silicon, and Windows passed their fresh-runner smoke tests. Homebrew formula installation and tests passed before publication.

We downloaded all ten assets and verified their GitHub API digests. All nine entries in ``SHA256SUMS`` match. Each platform archive carries one executable and a manifest naming beta.5. The source archive contains all three PDF modules and no agent instruction files.

The published Linux archive is identical to the workflow artifact we checked outside the checkout with an empty ``PATH``. Quickstart and discussion initialization installed files identical to the embedded modules. The executable built a book PDF and a discussion catalog's HTML and PDFs. Editing only the shared theme changed the PDF's actual text color; removing copied discussion templates restored the embedded teal layout. Section bookmarks retained their hierarchy. A template-folder link cycle terminated, and an oversized template input set failed with its diagnostic.

Independent PDF checks found all 120 table entries and 120 listing entries exactly once, at distinct positions within page bounds and across multiple pages. Records without sections omitted an empty contents block. We visually reviewed the manual cover, chapter opening and discussion layout, including later pages with code and tables.

The Linux x86-64 archive has SHA-256:

.. code-block:: text

   61f9e4a0a66276c500e50e2c039e93f12794d100ec7718003a71c421f927f802

The example in :doc:`../howto/github-pages` uses this published version and checksum.

Homebrew and documentation
--------------------------

The tap's `automatic import <https://github.com/insanai/homebrew-guidedog/actions/runs/38900622700>`_ selected beta.5 without an explicit tag. It verified the release metadata, asset digests and checksums, installed and tested the formula on Apple silicon and Intel, then committed ``b698d431f7160c1ffa6da5c6cdc95d633ba6823d``. The tracked formula is identical to the published ``guidedog.rb``. Hourly scheduled updates remain enabled.

We rebuilt the complete website with the released executable, including all seven manual PDF editions and all active discussion PDFs. The assembled site check covered 698 HTML pages and 170,564 internal links, with no missing files or anchors. The typography guide, discussion guide, template reference and custom-theme tutorial describe the shared modules and the upgrade path for existing projects. The manual's downloadable discussion README matches its source.
