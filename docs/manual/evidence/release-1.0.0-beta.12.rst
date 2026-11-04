Release 1.0.0-beta.12
=====================

On 4 November 2026 we published `v1.0.0-beta.12 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.12>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``9a53d0888e1d9159e9e07be625383a0c01d5fe48``.
We prepared the candidate on 3 November and pushed the tag after Verify and Website passed for that exact commit.

What the release holds
----------------------

We merged ``nisaba-content-policy`` at ``71a645a3903dda3cf28b0d665d16fa3d756cb320`` into main with merge commit ``a7c4a6f``.
Nisaba already uses that branch for confined builds of collaborative documentation projects.

GDS 0018 separates the folders, network, plugins and graph/music files a build may access from the raw content and unsafe URLs it emits.
Library callers can impose content rules or supply defaults, and projects can state them with ``raw_content`` and ``unsafe_urls``.
``--untrusted`` still applies strict confinement and omits both kinds of content, while the embedded compiler now renders project Typst diagrams and uses templates and preambles inside the source folder.
A confined PDF build always uses the embedded compiler and ignores configured external programs, including ``--typst``.
Local intersphinx inventories respect the readable folders even when the network is allowed.

Images written as ``name.*`` prefer WebP over JPEG for HTML pages; PDF books continue to prefer JPEG.
The changelog records the Rust API changes, including ``Confinement``, ``ContentRules`` and ``PluginStart::access``.
GDS 0018 is committed and the catalog validates all eighteen records.

Review before the release
-------------------------

We corrected a stale compatibility paragraph that still said untrusted builds discard project Typst templates and preambles.
All six manual and website translations cover the new rules and image preference, and an audit checks that translated code literals match the source.

The full test run exposed quadratic resolution of a run of MyST targets: each label scanned the remaining labels to find the next content element.
A run of 20,000 labels exceeded its ten-second test limit twice, taking about twelve seconds for that input alone.
We now index the next element once per node, retain that storage between reads, and preserve comment skipping and unresolved trailing labels.
A new regression checks those semantics, and the complete nineteen-test MyST file passed in about a second locally.
Commit ``fb6d25c`` records the fix.

Qualification
-------------

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/41742544249>`_ passed on Linux x86-64 and macOS Apple silicon.
Each platform passed 2,216 workspace tests across 212 reports, with five ignored.
Compilation, strict Clippy, formatting, generated tables, the source-limit checker, the pinned music font, all 66 music scores and the 12,000-case fuzz run passed.
The large-site incremental benchmark, example programs and HTML/PDF, discussion and Pages smoke tests also passed.
Linux additionally passed the isolated Secret Service credential-store check.
Local qualification found zero hard source-limit violations across 1,262 files; six existing soft-width notices remain.

The `Website run <https://github.com/insanai/guidedog/actions/runs/41742543448>`_ built and deployed all seven HTML and PDF manual editions.
It checked 937 HTML pages and 211,262 internal links, with no missing files or anchors.
All six translated manual and website catalogs have zero fuzzy and zero untranslated messages.

A local debug build of the Spanish manual reached Typst 0.15.1's deliberate ``bounded inline layout is incomplete`` panic.
The upstream line breaker panics for that condition only with debug assertions; its release build retries without the bound.
The production website build passed, and the downloaded beta.12 executable compiled the same Spanish manual successfully on this host.
This remains an upstream Typst debug-build limitation.

Published archives
------------------

All sixteen jobs in the `release run <https://github.com/insanai/guidedog/actions/runs/41744241677>`_ passed.
The five native platform builds, fresh-runner archive smoke tests and release Homebrew installation passed before publication.

We downloaded all ten published assets and verified every GitHub API digest and size.
All nine entries in ``SHA256SUMS`` match.
The Linux x86-64 build manifest names beta.12 and the tagged commit, its program checksum matches the extracted executable, and the executable reports ``guidedog 1.0.0-beta.12`` with Graphviz 16.1.0 linked statically and Typst 0.15.1 embedded.

We verified signed build provenance for the five binary archives, the source archive and ``guidedog.rb`` with GitHub CLI 2.103.3.
Verification enforces the repository, the ``release.yml`` signer workflow, source commit ``9a53d0888e1d9159e9e07be625383a0c01d5fe48`` and source ref ``refs/tags/v1.0.0-beta.12``.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   26d8abb12889714e1f2840a372851e2582a5fc842dcc18ee0450371581519fba

Homebrew and documentation
--------------------------

The tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/41746960182>`_ installed and tested the published formula on Apple silicon and Intel macOS.
It committed ``81680055e44f5e508df1d12943fbdf7551081734``.
The tracked formula matches the published ``guidedog.rb`` byte for byte.
The shared workflow example in :doc:`../howto/github-pages` pins beta.12 and its verified published Linux checksum for every manual language.
The website article uses the actual publication date so it precedes the previous day's beta.11 article.
