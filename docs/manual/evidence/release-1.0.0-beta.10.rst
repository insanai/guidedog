Release 1.0.0-beta.10
=====================

On 2 November 2026 we published `v1.0.0-beta.10 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.10>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``8985e5c5b783fbe9f1f3625bcc0c60265f8995ba``.
We pushed the tag after both Verify and Website succeeded for that exact commit.

A first tag and two LilyPond defaults
-------------------------------------

We first tagged commit ``095f19543a654054da9e6a0bd3f8386a3508597e``.
Before tagging, we had compiled the songbook's *Mary Had a Little Lamb* with LilyPond 2.24.4 and compared it bar by bar with Guidedog's engraving: notes, lyrics, chord names, frets and strings agreed.
The comparison left two differences from LilyPond's defaults: LilyPond prints 4/4 as the symbol C, and it raises a chord's extensions, as in G with a raised 7.
The maintainer asked for both in this release, so we cancelled the `first release run <https://github.com/insanai/guidedog/actions/runs/41397837481>`_ while its platform builds were running, before it published anything, and deleted the unpublished tag.

Commit ``6791925`` gives ``gd_score::Meter`` a style: 4/4 prints as C and 2/2 as cut C, ``\numericTimeSignature`` asks for numbers, and ``\defaultTimeSignature`` restores the symbols.
Before, ``\numericTimeSignature`` was accepted but changed nothing.
Commit ``4a80e54`` makes chord names markup built as LilyPond's default chord namer builds them: extensions raised, a triangle for a major seventh, a raised o or slashed o for diminished and half-diminished chords, and accidentals as music glyphs.
We compared every supported chord-mode quality, with sharps, flats and a bass note, against LilyPond 2.24.4's own output.
The same comparison found that ``c:7sus4``, listed as supported, was split into ``7`` and a note name; a quality is now read as one unspaced run of tokens.
61 of the 66 corpus goldens changed, almost all by the C symbol alone, and we inspected the changed scores before accepting them.
We then tagged the new commit.

Qualification
-------------

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/41400172517>`_ passed on Linux x86-64 and macOS Apple silicon.
The workspace suites passed 2,138 tests on Linux and 2,137 on macOS, each with five ignored, including the three browser typography tests on each platform.
Formatting, Clippy with warnings denied, compilation, the nine generated tables, the source limits, the pinned music font, the 66-score music corpus and 12,000 music fuzz cases passed.

The `Website run <https://github.com/insanai/guidedog/actions/runs/41400172482>`_ built and deployed all seven language editions with their manual PDFs.
It checked 899 HTML pages and 206,949 internal links with no missing files or anchors.
All six translated manuals and website editions have zero fuzzy or untranslated messages, and every edition builds with warnings treated as errors.

Published archives
------------------

All sixteen jobs in the `release run <https://github.com/insanai/guidedog/actions/runs/41401484183>`_ passed.
The five native platform builds, fresh-runner archive smoke tests and Homebrew formula installation passed before publication.

We downloaded all ten published assets and verified their GitHub API digests and sizes, including ``SHA256SUMS`` itself.
All nine entries in that checksum manifest match.
The Linux x86-64 archive's build manifest names 1.0.0-beta.10 and the tagged commit, its program hash matches the archived executable, and the executable reports ``guidedog 1.0.0-beta.10``.

GitHub's attestation API holds an SLSA provenance statement for the Linux archive's digest, made by ``.github/workflows/release.yml`` at ``refs/tags/v1.0.0-beta.10`` from the tagged commit.
We did not verify the signature locally.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   122b24d59aff93e2bda6d2f4af130680259675b5220f9ff104033dac5b82afa7

Homebrew and documentation
--------------------------

The tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/41405469850>`_ installed and tested the published formula on Apple silicon and Intel macOS.
It committed ``72afd4cd76dc39be869ae9783232bce7ac63aa12``.
The tracked formula matches the published ``guidedog.rb`` byte for byte.
The shared workflow example in :doc:`../howto/github-pages` pins beta.10 and its verified published Linux checksum for every language edition.

Web typography
--------------

Issue `#4 <https://github.com/insanai/guidedog/issues/4>`_ reported body text set three ways on one page and a line breaker that measured the line after a hyphen too narrow; we closed it with this release.
In Chromium at a 1300-pixel window on the deployed manual, the five pages the issue measured set 107 of their 110 multi-line blocks with Knuth-Plass, none with greedy CSS justification, and no line overflows.
The three others, on ``autodoc.html``, are long lists of inline option names that no layout fits within the spacing limits; they keep the browser's ragged layout.
