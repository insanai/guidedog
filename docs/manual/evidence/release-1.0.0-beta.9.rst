Release 1.0.0-beta.9
====================

On 2 November 2026 we published `v1.0.0-beta.9 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.9>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``6a449916346590481ff22a5d92f7afdce1056cfe``.
We pushed the tag after both Verify and Website succeeded for that exact commit.

A first tag and a frozen website
--------------------------------

We first tagged commit ``e0007abc426660eaf5e8022a4c10490d00cd775b``.
While its release run was building, the deployed website froze the browser with a "Page Unresponsive" warning.
We cancelled that run before it published anything.

Paused in headless Chromium, the page was inside ``typography.js``.
The hyphenator returned its stored arrays for exception words, and the line breaker appended each occurrence's punctuation to them.
After ``project.`` and ``project,`` the stored syllable read ``project.,``, longer than the text, and ``spanWidth`` looped forever.
The defect shipped with beta.8; the new landing page repeated "project" with punctuation and so triggered it.
Long pages had a second problem: each paragraph was measured right after the previous one was rewritten, so the browser laid out the whole page once per paragraph, and the largest API page blocked for sixteen seconds.

Commit ``37d5f62`` copies the hyphenator's arrays, makes ``spanWidth`` always advance, and measures every paragraph before rewriting any; that API page now typesets in about 1.4 seconds.
Commit ``b124a3f`` adds two headless-browser tests to ``tests/integration``: a page of repeated exception words and a page of 1,200 paragraphs must each finish loading within 45 seconds and typeset their paragraphs.
With either defect restored, its test fails at the time limit.
Verify sets ``GUIDEDOG_REQUIRE_BROWSER``, so the tests cannot be skipped there.
We then moved the unpublished tag to the fixed commit.

Native music and translations
-----------------------------

This release adds native LilyPond engraving through ``gd-lilypond``, ``gd-score`` and ``gd-engrave``; :doc:`../topics/music` describes it, and GDS 0015 records its design and reviews.
Generated score descriptions name keys in each user-interface language's own orthography, and score SVGs carry ``<title>`` and ``<desc>`` elements.
All six translated manuals, the website editions and the READMEs were reviewed; every catalog has zero fuzzy or untranslated messages.
Every manual and website edition builds with warnings treated as errors.

Qualification
-------------

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/41366912152>`_ passed on Linux x86-64 and macOS Apple silicon.
Each workspace suite passed 2,115 tests, with five ignored, including both browser tests on each platform.
Formatting, Clippy with warnings denied, compilation, the nine generated tables, the source limits, the pinned music font, the 63-score music corpus and 12,000 music fuzz cases passed.

The `Website run <https://github.com/insanai/guidedog/actions/runs/41366912148>`_ built and deployed all seven language editions with their manual PDFs.
It checked 870 HTML pages and 204,313 internal links with no missing files or anchors.
After the release's refresh we loaded the live landing page, the music chapter, the largest API page, the Japanese edition and the release article in headless Chromium; each finished in under two seconds with no script errors.

Published archives
------------------

All sixteen jobs in the `release run <https://github.com/insanai/guidedog/actions/runs/41368093965>`_ passed.
The five native platform builds, fresh-runner archive smoke tests and Homebrew formula installation passed before publication.

We downloaded all ten published assets and verified their GitHub API digests and sizes, including ``SHA256SUMS`` itself.
All nine entries in that checksum manifest match.
All five native archives name 1.0.0-beta.9 and the tagged commit in their build manifests, and every program hash in the four Unix manifests matches its archived executable.
The published Linux archive reports ``guidedog 1.0.0-beta.9``.

GitHub's attestation API holds an SLSA provenance statement for the Linux archive's digest, made by ``.github/workflows/release.yml`` at ``refs/tags/v1.0.0-beta.9`` from the tagged commit.
The GitHub CLI on the checking machine (2.46.0) cannot verify attestation signatures, so we did not verify the signature locally.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   f9fb00c6b62c3abf8b5991dab329f4591a868604bbcbf7d7bcaada91b7135efd

Homebrew and documentation
--------------------------

The tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/41370543264>`_ installed and tested the published formula on Apple silicon and Intel macOS.
It committed ``254a10c22812cf06b2eb0bf8f16bc5ee0281b14a``.
The tracked formula matches the published ``guidedog.rb`` byte for byte.
The shared workflow example in :doc:`../howto/github-pages` pins beta.9 and its verified published Linux checksum for every language edition.
