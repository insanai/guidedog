Release 1.0.0-beta.4
====================

On 16 October 2026 we published `v1.0.0-beta.4 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.4>`_ as a normal GitHub release and marked it latest. The annotated tag names commit ``0933a32d84e60f1900ad179d4f8aa41392e3189d``.

Discussion PDF layout
---------------------

Discussion records use a dedicated RFC-style layout. The first page holds the identifier, title, metadata and the start of the discussion. Numbered sections, restrained typography, running headers and page counts support longer papers. Ordinary books keep their own template. The :doc:`discussion-pdf-layout` record explains the root cause, shared metadata contract and layout regression coverage.

The binary embeds the layout. ``guidedog dsc init`` copies that same source into ``_templates/discussion.typ`` for customization. Existing catalogs without a copy use the embedded layout. Initialization writes the template as part of its staged transaction, so a failed initialization leaves no partial catalog.

Source and archive qualification
---------------------------------

The `Verify run <https://github.com/insanai/guidedog/actions/runs/38890137704>`_ passed on Linux and macOS. The local workspace suite passed 1,909 tests with 4 ignored. The workspace build, Clippy, formatting, source limits, generated tables and discussion smoke test passed.

All 16 jobs of the `release run <https://github.com/insanai/guidedog/actions/runs/38891112189>`_ passed. Native archives for Linux x86-64 and ARM64, macOS Intel and Apple silicon, and Windows passed their fresh-runner smoke tests. Each archive initialized a catalog, installed its discussion template and built a PDF whose generated source imported that template with structured metadata. Homebrew formula installation and tests passed before publication.

We downloaded all ten published assets and verified their GitHub API digests. All nine entries in ``SHA256SUMS`` match. Each platform archive carries one executable and a manifest naming beta.4. The source archive contains no agent instruction files.

The published Linux x86-64 archive is identical to the workflow artifact we tested outside the checkout with an empty ``PATH``. It initialized an ADR catalog, installed a template identical to the embedded source, and built HTML and PDFs. The process draft and numbered record both placed their abstract on the first page. After we removed the copied template, a selected-record PDF still built with the embedded fallback.

The Linux x86-64 archive has SHA-256:

.. code-block:: text

   dee3f4c7dbed415cd64c2b924dac8f33015a3c2f218c8e43bb85a47de0969059

The example in :doc:`../howto/github-pages` uses this published version and checksum.

Homebrew and documentation
--------------------------

The tap's `automatic import <https://github.com/insanai/homebrew-guidedog/actions/runs/38893660014>`_ selected beta.4 without an explicit tag. It verified the release assets and checksums, installed and tested the formula on Apple silicon and Intel, then committed ``36e0457401af4546b6f2939189ada02a0ba770d4``. The tracked formula is identical to the published ``guidedog.rb``. Hourly scheduled updates remain enabled.

We rebuilt the complete website with the released executable, including all seven manual PDF editions and all active discussion PDFs. The assembled site check covered 684 HTML pages and 169,493 internal links, with no missing files or anchors. We also downloaded GDS 0012 from the live website and confirmed its two-page RFC layout, with metadata and the abstract on the first page.
