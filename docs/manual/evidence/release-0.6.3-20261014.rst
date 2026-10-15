.. _guidedog-063-release-checks:

Guidedog 0.6.3 release checks
=============================

Guidedog 0.6.3 was published on 14 October 2026 from annotated tag ``v0.6.3``, at commit
``d0be2f9b0d26c285dba3c7bad93bbd4952620784``. The tag names the same commit that passed the
full candidate run. No published tag or asset was replaced.

Qualification
-------------

-  `Pre-release CI <https://github.com/insanai/guidedog/actions/runs/38511730686>`__ passed
   on Linux, macOS and Windows.
-  `Full candidate <https://github.com/insanai/guidedog/actions/runs/38512031064>`__ passed
   every gate without publishing.
-  `Tagged release <https://github.com/insanai/guidedog/actions/runs/38513788774>`__ passed
   every gate and published the
   `release <https://github.com/insanai/guidedog/releases/tag/v0.6.3>`__.

Both full runs checked every library and tool suite, integration tests, source limits, and
AddressSanitizer on Linux x86-64 and macOS Apple silicon. They built the embedded-Typst
programs for Linux x86-64 and ARM64, macOS Apple silicon and Intel, and Windows x86-64. All
five archives passed fresh-machine smoke tests; the Intel archive ran on an Intel Mac. The
formula passed installation and tests. Native credential-store qualification passed as
required by the release workflow.

This patch adds adaptive Graphviz SVGs and corrects local HTML downloads and portable PDF
download URLs. The new theme tutorial's exact files are exercised across HTML, directory
HTML, single-page HTML and PDF. The preceding :doc:`documentation
review <visual-docs-review-20261014>` records the diagram, translation and book-layout
checks. This patch qualification did not rerun the CPython, Django and Flask baselines. It
does not claim universal native memory safety.

Published downloads
-------------------

The release has five Guidedog archives, five standalone dscc archives, a source archive, a
Homebrew formula and ``SHA256SUMS``. All twelve manifest entries match GitHub's published
asset digests. The downloaded Apple silicon Guidedog and dscc archives, Linux x86-64
Guidedog archive and formula match their manifest checksums.

.. list-table::
   :header-rows: 1
   :widths: auto

   * - Download
     - SHA-256
   * - ``guidedog-macos-arm64.tar.gz``
     - ``d7dddca37d4c94bfdb97dfe30562581d930d9e6d48dd772e9e85ce89c3815dae``
   * - ``dscc-macos-arm64.tar.gz``
     - ``611a016e6abc20a3a1480a740bae9d0787acca74f42c037f14beae919035ed7d``
   * - ``guidedog-linux-amd64.tar.gz``
     - ``35eca388ac7a4061076bfa688102127acdb2f1db30e183c8a5b8002362e359d0``
   * - ``guidedog.rb``
     - ``59cd87c8cd9090f71d8dfdf13ae1bc2c43aa793bbe26c0b493d04b431fdb7b5e``

The published Apple silicon programs all report 0.6.3: guidedog, dsc, dscc and
guidedog-pages. Guidedog also passed the smoke script locally with an empty ``PATH``:
quickstart, Graphviz, equations, Markdown, static API extraction, HTML and PDF. It produced
a 59,483-byte PDF without an installed interpreter, compiler, Typst program or Graphviz
program on that path.

``gh attestation verify`` validated the published Apple silicon Guidedog archive. The
verified certificate names the repository's ``release.yml``, tag ``v0.6.3``, the commit
above and tagged run ``38513788774``.

Homebrew and documentation
--------------------------

The official tap's `Update
run <https://github.com/insanai/homebrew-guidedog/actions/runs/38515507139>`__ verified the
release, installed it, built HTML and PDF, and committed the tested formula as
``e9aecfcf0fcad94ac348d363f5822ec208a049d4``. It installs 0.6.3.

The manual's shared GitHub Actions example now pins 0.6.3 and the verified Linux x86-64
archive checksum. The code block is shared by all manual languages. The release preparation
updated all program versions and four documentation projects together. Guidedog's gettext
and intl commands refreshed the availability notes; all six maintained translation catalogs
had zero fuzzy or untranslated active messages.

These results qualify the documented beta surface. They do not freeze the beta plugin SDK or
add a sandbox. GDS 0002 still retains its unfinished Typst obligations.
