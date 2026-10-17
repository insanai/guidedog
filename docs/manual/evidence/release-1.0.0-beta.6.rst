Release 1.0.0-beta.6
====================

On 17 October 2026 we published `v1.0.0-beta.6 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.6>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``08e657a5e9ab6b40527e6db0514a99604a2bafee``.

Complete static publishing
--------------------------

We reviewed GDS 0013 and implemented its complete static publishing contract in Rust.
The release includes publication scheduling, filtered collections, taxonomy archives, pagination, collection feeds, article PDFs, mounted projects, language editions, shared assets, pinned themes and three starter presets.
HTML navigation and template dependencies are traced for incremental builds.
The :doc:`ssg-review` record describes the review corrections, behavioral coverage, visual checks and 1,000-article measurement.
The :doc:`../howto/static-publishing` guide describes the commands and configuration.
No SSG feature in GDS 0013 remains deferred.

Qualification
-------------

The `Verify run <https://github.com/insanai/guidedog/actions/runs/38983782044>`_ passed on Linux and macOS for commit ``08e657a5e9ab6b40527e6db0514a99604a2bafee``.
The local workspace suite passed 1,950 tests with five ignored; we ran the ignored 1,000-article regression separately and it passed.
The workspace build, Clippy with warnings denied, formatting and all nine generated data tables passed.
The source checker reported no violations and six existing soft-limit warnings.
The final theme boundary regression verifies installation and offline validation at the maximum file count and path depth before the lock is selected.

Protocol compatibility against the released 0.6.0 host and SDK passed all five applicable cases.
The remaining direction is inapplicable because 0.6.0 has no build plugins.
Both publisher directions and the new Pages process adapter passed; the old host cleanly refused the new build plugin.

The exact commit's `Website run <https://github.com/insanai/guidedog/actions/runs/38983782058>`_ built and deployed the website through ``guidedog ssg build``.
It checked 761 HTML pages and 173,111 internal links with no missing files or anchors.
The site includes all seven manual PDF editions and all active discussion PDFs.

Published archives
------------------

All 16 jobs of the `release run <https://github.com/insanai/guidedog/actions/runs/38984837483>`_ passed.
Native archives for Linux x86-64 and ARM64, macOS Intel and Apple silicon, and Windows passed their fresh-runner smoke tests.
Each archive exercised the complete SSG scenario with an empty ``PATH``, including taxonomy, pagination, feeds, pinned themes, mounted projects, all three presets and a real article PDF.
Homebrew formula installation and tests passed before publication.

We downloaded all ten published assets and verified their GitHub API digests, including the checksum manifest itself.
All nine entries in ``SHA256SUMS`` match.
The Linux archive's GitHub provenance attestation verifies the tagged release workflow and commit.
All four Unix archives have matching beta.6 manifests, the tagged commit and verified executable hashes.
The published Linux x86-64 archive is identical to the workflow artifact we checked outside the checkout with an empty ``PATH``.
Its executable matches the build manifest and reports beta.6.
It passed the full SSG, discussion HTML/PDF and offline Pages smoke tests.
The published source archive matches the inspected workflow artifact, includes the SSG and theme implementation, article layout and documentation, and contains no agent instruction files.

The Linux x86-64 archive has SHA-256:

.. code-block:: text

   a67074a91d3de18056b0c5a0fc910cd2318d2c3d6ee42706b395df34123dc048

The example in :doc:`../howto/github-pages` uses this published version and checksum.

Homebrew and documentation
--------------------------

The tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/38986496878>`_ verified the published assets and installed and tested the formula on Apple silicon and Intel macOS.
It committed ``c4b9d555fcd4ddd2cbb76d9b7dab7761ac8a5710``.
The tracked formula is identical to the published ``guidedog.rb``.
Hourly scheduled updates remain enabled.

The website's workflow example now pins the published beta.6 Linux x86-64 archive and its verified checksum.
This record and the complete static publishing guide are linked from the manual.
