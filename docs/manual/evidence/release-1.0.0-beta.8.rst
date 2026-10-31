Release 1.0.0-beta.8
====================

On 31 October 2026 we published `v1.0.0-beta.8 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.8>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``e7deea392b8cd43148821dc3b5baf38fdb9c0c08``.
We pushed the tag after both Verify and Website succeeded for that exact commit.

Network preview and diagrams
----------------------------

The preview server accepts ``--host`` with an IPv4 or IPv6 address.
Its default remains ``127.0.0.1``.
We verified a real HTTP request through the host's non-loopback address with ``--host 0.0.0.0``.

The ``typst-diagram`` directive accepts inline Typst or a local source file in RST and MyST.
The embedded compiler exports one-page SVG or PNG images for HTML and PDF, including mounted-site PDF builds whose cache is outside the source tree.
Imported files participate in incremental dependency tracking.
Package and font configuration is shared with PDF compilation.
Untrusted builds retain the source without invoking the compiler.
We reproduced the sibling jlavici project's CeTZ 0.3.4 and Fletcher 0.5.8 diagrams offline in a temporary project copy.

The :doc:`../topics/diagrams` chapter covers Graphviz and Typst.
The :doc:`../howto/typst-diagrams` guide documents directive options, packages, fonts and diagnostics.
All six translated manual catalogs have zero fuzzy or untranslated messages.
Each translated HTML edition builds with warnings treated as errors.

GDS states
----------

The dated status review in ``docs/gds/evidence/status-review-20261031.rst`` covers all fourteen records.
We promoted the diagram proposal to GDS 0014 and recorded its accepted and committed transitions with implementation evidence.
We also corrected GDS 0013's stale discussion state to committed after reviewing the published static-publishing and rich-excerpt implementations.
Thirteen records are committed.
GDS 0002 remains published because hostile-native-Typst worker isolation and native Typst chapter metadata remain unimplemented.

Qualification
-------------

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/41177167506>`_ passed on Linux x86-64 and macOS Apple silicon.
Each workspace suite passed 1,984 tests, with five ignored.
The ignored 1,000-article regression passed separately.
Formatting, Clippy with warnings denied, compilation, all nine generated tables and source limits passed.
The source checker reported no violations and six existing soft-width warnings.
The diagram compiler also passed its tests without the embedded feature.

The `Website run <https://github.com/insanai/guidedog/actions/runs/41177167390>`_ built and deployed all seven language editions, their manual PDFs and the discussion PDFs.
It checked 819 HTML pages and 177,526 internal links with no missing files or anchors.
The mounted-site SVG and PNG regression failed before the cache-directory fix and passed after it.
The fix grants the compiler access to generated image directories without granting access to unrelated cache directories.

Published archives
------------------

All jobs in the `release run <https://github.com/insanai/guidedog/actions/runs/41177963905>`_ passed.
The five native platform builds, fresh-runner archive smoke tests and Homebrew formula installation passed before publication.
The smoke tests exercise embedded SVG and PNG diagram generation without external programs or package downloads.

We downloaded all ten published assets and verified their GitHub API digests and sizes, including ``SHA256SUMS`` itself.
All nine entries in that checksum manifest match.
All five native archives contain the beta.8 version and tagged commit in their build manifests.
Every program hash in those manifests matches its archived executable.
The source archive includes the diagram exporter and promoted GDS and contains no agent instruction files.

GitHub CLI 2.103.3 verified the published Linux archive's provenance attestation against ``insanai/guidedog``.
The verified statement identifies the tagged release workflow and commit.
The published Linux archive matches the workflow artifact that passed the complete SSG, discussion HTML/PDF and offline Pages smoke tests outside the checkout.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   0ca5f11cc62c5ac199e470f032f425d52e6f45393b7d21a6998094e23958ded7

Homebrew and documentation
--------------------------

The tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/41180040735>`_ installed and tested the published formula on Apple silicon and Intel macOS.
It committed ``9aa4956bbf9014979414c206d367f39b2bcb1d26``.
The tracked formula matches the published ``guidedog.rb`` byte for byte.
The shared workflow example in :doc:`../howto/github-pages` pins beta.8 and its verified published Linux checksum for every language edition.
