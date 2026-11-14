Release 1.0.0-beta.13
=====================

On 2026-11-14 we published `v1.0.0-beta.13 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.13>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``b209b01b8a6878f9de3e91d6c63429c2a52ddac3``.
We prepared the candidate on 2026-11-13 and tagged it after exact-commit CI, website deployment and an untagged five-platform release candidate passed.

Purpose and scope
-----------------

We support Rust 1.100 with Rust 1.100.0 pinned for builds and Rust 1.100 declared by every workspace crate.
The edition remains 2024.
Unix release archives link Graphviz 16.1.1 statically; Windows bundles its verified runtime.
The Graphviz patch fixes a possible heap overrun involving cluster names and several layout crashes.

The process allocator and workspace buffer reuse are unchanged.
We added no allocator model or dependency.
Measured performance differences between Rust 1.99 and 1.100 are insignificant overall; this release adopts the current toolchain and Graphviz correctness fixes.
The measurements and their limits are in :doc:`rust-1.100-graphviz-16.1.1`.

Clippy iterator cleanups preserve document semantics.
Test helpers locate public build outputs without assuming Cargo's old intermediate directory depth, and the manual's plugin example builds through Cargo's public artifact messages.
The installation guide, seven README editions, release metadata, licenses and notices use the new pins.
All six website translations describe the release and the allocation decision.

Qualification
-------------

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/43525433791>`_ passed on Linux x86-64 and macOS Apple silicon.
Each platform passed 2,216 workspace tests across 212 reports, with five ignored.
Build, strict Clippy, formatting, source limits, generated tables and the pinned Bravura files passed.
All 66 music scores, the 12,000-case fuzz corpus, the 1,000-article incremental benchmark, examples and HTML/PDF, discussion and Pages smoke tests passed.
Linux also passed the isolated Secret Service check.

The `Website run <https://github.com/insanai/guidedog/actions/runs/43525642782>`_ built and deployed all seven HTML/PDF manual editions and all eighteen discussion PDFs.
It checked 965 HTML pages and 213,152 internal links, with no missing files or anchors.
All 120 manual and eighteen website catalogs have zero fuzzy or untranslated messages.
The release-note review checks version numbers, links and rendered markup in every language.

A release-gate review found that GitHub's status-filtered workflow listing omitted a successful manual Verify run.
The lookup now selects completed successful run records and still requires the exact source commit.
A live query and six guard checks covered acceptance and refusal of wrong, unfinished, failed, cancelled or missing runs.

The untagged `native candidate <https://github.com/insanai/guidedog/actions/runs/43527053551>`_ built all five packages and passed fresh-runner archive and Homebrew tests before we created the tag.
All sixteen jobs of the tagged `release run <https://github.com/insanai/guidedog/actions/runs/43529740342>`_ passed.
The packages cover Linux x86-64 and ARM64, macOS Intel and Apple silicon, and Windows x86-64.

Published archives
------------------

We downloaded all ten published assets and verified every GitHub API digest and size.
All nine entries in ``SHA256SUMS`` match.
Each platform's build manifest names beta.13, the tagged commit, Rust 1.100.0 and Graphviz 16.1.1, and its program checksum matches the archived executable.
The source archive's workspace version and toolchain pins agree.
The Homebrew formula names the verified hashes of all four Unix archives.
The downloaded Linux executable reports beta.13, static Graphviz 16.1.1 and embedded Typst 0.15.1.

We verified signed provenance for the five binary archives, source archive and Homebrew formula with GitHub CLI 2.103.3.
Verification enforces the repository, the ``release.yml`` signer workflow, the tagged source commit and ``refs/tags/v1.0.0-beta.13``.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   82555c5f5a46bdc34c2b0616d1ff77e0cf20fe5901c154d5095e04cfa112c9dc

Homebrew and documentation
--------------------------

The official tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/43532593387>`_ installed and tested beta.13 on Apple silicon and Intel macOS.
It committed ``5fa48d15e3ee71171305b846bf0e5b7ccd3e4e85``.
The tracked formula matches the published ``guidedog.rb`` byte for byte.
The shared workflow in :doc:`../howto/github-pages` pins beta.13 and the verified published Linux checksum for every manual edition.
The website article uses the actual publication date.
