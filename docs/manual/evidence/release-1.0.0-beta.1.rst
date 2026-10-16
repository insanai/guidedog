Release 1.0.0-beta.1
====================

On 15 October 2026 we published `v1.0.0-beta.1 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.1>`_ as a normal GitHub release. The version identifies its beta status. The annotated tag names commit ``fd349a74ba69b0ab345bdb85284a9827783aa138``.

Source qualification
--------------------

The `Verify run <https://github.com/insanai/guidedog/actions/runs/38731461733>`_ passed on Linux x86-64 and macOS arm64. It checked formatting, source limits, generated tables, the workspace build, Clippy, the full test suite, examples and CLI smoke tests. The workspace suite passed 1,893 tests with 4 ignored. One additional Linux Secret Service integration test passed against a real isolated keyring.

The `release run <https://github.com/insanai/guidedog/actions/runs/38732558360>`_ reused that successful verification for the exact tagged commit. It built native archives for Linux x86-64 and arm64, macOS Intel and Apple silicon, and Windows x86-64. Every archive passed its program checksum, version, HTML, PDF, discussion and Pages checks. Homebrew installation and formula tests passed before publication.

Downloads
---------

We downloaded all ten assets. All match the GitHub API asset digests, and the nine files listed in ``SHA256SUMS`` match the published manifest. The Linux executable reports ``guidedog 1.0.0-beta.1``, embedded Typst and static Graphviz. It occupies 90.2 MB. The Windows ZIP contains one executable, ``guidedog.exe``, with its runtime DLLs beside it.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   3dfbebc3c8189cbeff05a8ade620dee72a1400ed12c583b3f4df2d5679c8a9a8

At publication, the example in :doc:`../howto/github-pages` used this version and checksum. That example now follows the current release.

Homebrew and documentation
--------------------------

The tap's `automatic import <https://github.com/insanai/homebrew-guidedog/actions/runs/38737499042>`_ selected the new release without an explicit tag. It verified the release metadata, formula, manifest and four archive hashes, then installed and tested the formula on Apple silicon and Intel macOS before committing it. The tap's `final tests <https://github.com/insanai/homebrew-guidedog/actions/runs/38740554142>`_ passed on both architectures. The tracked formula is identical to the published ``guidedog.rb``. Scheduled updates check for new versions every hour and reject downgrades.

Guidedog builds its own `website <https://insanai.github.io/guidedog/>`_, manual, API reference and discussion catalog. The site includes seven language editions and their PDF books. The local site check covered 635 HTML pages and 166,070 internal links, with no missing files or anchors. We recorded the implementation decision with ``guidedog dsc`` in `GDS 0012 <https://insanai.github.io/guidedog/discussions/records/0012-single-executable-release.html>`_.
