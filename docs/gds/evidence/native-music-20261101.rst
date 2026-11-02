Native music implementation review, 1 November 2026
===================================================

We reviewed GDS 0015 before implementation, then reviewed the native reader, engraver and project integration against its complete three-phase contract.
The implementation uses ``gd-score``, ``gd-lilypond`` and ``gd-engrave`` with explicit options, typed indices, borrowed fonts and reusable per-thread workspaces.
The core libraries open no files, invoke no program and execute no Scheme.
The project adapter supplies confined includes, diagnostics, dependencies and image publication.

Scope and evidence
------------------

All three notation phases are implemented.
The reader's command ledger distinguishes implemented notation, ignored metadata and refused programming constructs.
Its profile tests exercise every accepted command and explicit refusals.
The 58-score corpus covers pitch languages, relative and fixed octaves, transposition, accidentals, signatures, courtesy clefs and signatures, compound and grouped beats, pickups, exact durations, rests, chords, beams, tuplets, ties, slurs, articulations, fingerings, dynamics, structured ``\markup``, lyrics, voices, staff groups, grace notes, ``\volta`` repeats, tempo and chord names.
There is no remaining implementation phase in this record.

``cargo run --locked -p music-check`` checks finite ink frames, actual ink collisions by semantic role, exposed primary beam quants and tie apexes on both the 420-point standard edition and the 225-point compact edition before comparing the SVG and PNG goldens.
SVG must match byte for byte.
PNG dimensions must match; at most 0.2 percent of pixels may differ by more than eight channel levels, and mean channel error must remain below one percent.
The alpha-mask collision check uses 288 pixels per inch after a bounding-box rejection, with the documented component contacts permitted.
The 58 cases pass these checks.

The deterministic fuzz run uses 12,000 hostile, mutated, Unicode and structured notation inputs under finite budgets.
It produces 4,257 successful parses and 3,729 successful engravings.
Spacing properties compare the solver with exhaustive active sets, preserve original spanning rods and verify monotone duration spacing.
Line-breaking properties compare the dynamic program with every partition of small generated problems.
Nine project tests exercise real HTML and PDF builds, captions, figure references, PNG, narrow editions, multi-system books, includes, missing-file invalidation, symbolic-link confinement, untrusted builds, localized descriptions and original source locations through MyST and included RST.

Critique and corrections
------------------------

* Simultaneous conflicting alterations updated staff state in event order.
  We now update a moment atomically and leave conflicting pitches ambiguous until a subsequent explicit note resolves them.
* Accidental columns used whole bounding rectangles, preventing useful nesting.
  They now use quarter-space ink bands refined by the SMuFL cut-outs.
* A column with unrelated ink could erase a previous band's clearance.
  We now retain the last occupied column separately in each band and include ledger and rest footprints.
* Cautionary parentheses touched ledger lines, and a flat signature touched its clef.
  Their actual glyph bounds now determine placement and prefatory clearance.
* Two voices needed consistent second and unison shifts, shared compatible heads and rests, free-space augmentation dots and stacked accidentals.
  Dedicated geometry regressions exercise these cases.
* Beam quantization constrained inner endpoints independently even though they share slope and spacing.
  Exposed primary ends remain hard constraints; named costs score secondary wedges and all actual segment collisions.
  Scoring and drawing share fractional endpoints.
* Whole-curve skyline translation detached slurs from their notes.
  Attached curves now retain their endpoints, with quarter-space attachment candidates, actual stem tips, sampled clearance and named scoring terms.
  Other outside marks move around their ink.
* Split spanners reused out-of-system note positions and ignored timed lyric anchors.
  Continuations now clamp to system edges, use the active clef and retain hairpin aperture and lyric extenders.
* Fixed lyric gaps and duplicate annotation baselines ignored shaped ink.
  We now use word-space advances, actual syllable bounds, metric-based verse spacing and annotation stacking.
* Long initial words could extend outside the horizontal frame.
  Each system now records its actual ink width, including per-system SVG and PNG export.
* PNG had no physical resolution, changing its natural PDF size.
  PNG now includes checked ``pHYs`` metadata at its declared resolution.
  HTML uses a 1.5x density descriptor for 144-dpi wide and narrow PNGs; a Chromium check confirms that their displayed width matches the SVG edition within pixel rounding.
* Direction-prefixed dynamics did not terminate a hairpin, and hairpin placement ignored its requested side.
  Directed marks now share the same termination path and preserve direction through the score model and geometry, with reader and drawing regressions.
* A tie search scanned all spanners for each notehead.
  An indexed lookup replaces it and consumes explicit work.
* Event and operation limits did not account for all transient allocations.
  Reader copies, measures and lyrics consume their expansion allowance; engraving reserves bounded scratch and retained capacities, outlines stop at 65,536 bytes per glyph, and the mark bound applies across the drawing.
* Grace state copies bypassed the allowance, and malformed programmatic beat lists could overflow a sum.
  Grace copies now use the same accounting helper, and beat lists are bounded before traversal or summation.
* Failed dependency candidates could be observed before confinement.
  Canonical confinement now precedes content observation, including symbolic-link targets.
* The repository's dependency and round-trip gates did not know the new crates and location metadata.
  The gate now records the native dependencies; semantic round-trip comparison ignores regenerated byte maps while source-location tests verify their original positions.
* A pickup's following system could repeat the full instrument name despite reserving only its short form.
  Full names now belong only to the first system, including measure zero.
* Per-cluster fallback could discard the shaping context of a script that one fallback face can draw completely.
  We now prefer a single covering face for the entire run before using cluster fallback, charge coverage scans by text length and compare Hindi fallback with direct whole-run shaping.
* A second review expanded ``\markup`` to a bounded LilyPond 2.24 tree (lists, columns, styles, small caps, sub/superscripts, sizes, enclosures, spacing, music symbols and markup variables) and added ``\volta`` pass lists, ``\fine``, ``\section``, ``\compressEmptyMeasures``, ``\compressFullBarRests`` and ``\numericTimeSignature``.
* The same review aligned pitchless ``\relative``, ``\transpose`` on ``\key`` tonics, implicit ``\acciaccatura`` and ``\appoggiatura`` slurs, manual-beam melismata, forward ``\lyricsto`` resolution, courtesy clefs and signatures, key cancellations, ``:..:`` bar merging, strictly level concave beams, 3/8 measure grouping, two-voice chord/dot placement, breakable PDF ``#figure`` blocks, 256-line template margin scanning across seven paper sizes, full CSS/print width units and external ``.ly`` fallback substitution.

Reference engraving review
--------------------------

We compiled all 48 inputs with the official LilyPond 2.24.4 Linux binary and inspected the four generated side-by-side SVG review pages.
The tool and source wrappers are reproducible with ``music-check --reference /path/to/lilypond``.
The wrappers create equivalent lyric contexts and request compressed multi-measure rests explicitly.
The oracle is used only during development.

The downloaded archive is ``lilypond-2.24.4-linux-x86_64.tar.gz`` from the official LilyPond GitLab generic package endpoint.
Its SHA-256 is ``53bc1df875ee56dabe56ae1f0b4550237e0c9acc837b60f0ef8c42fc35a68641``.
The endpoint is ``https://gitlab.com/api/v4/projects/lilypond%2Flilypond/packages/generic/lilypond/2.24.4/lilypond-2.24.4-linux-x86_64.tar.gz``.
Bravura remains unmodified at the commit and three file digests recorded in ``third_party/bravura/PROVENANCE.txt``.

.. list-table:: Reviewed differences in the house style
   :header-rows: 1
   :widths: 15 85

   * - Cases
     - Review
   * - 01-07
     - Pitch, spelling, octave and transposition meaning agrees; Bravura outlines and spring widths differ from LilyPond's font and spacing.
   * - 08-14
     - Cautionary parentheses, octave clefs and cancellation signs are present; clef clearance and optical sizes follow our measured glyph bounds. Common time prints numerically.
   * - 15-21
     - Meter groups, pickup, dots, exact scaled values and rests agree. Dense inner beam spacing and optical stem gaps follow the bounded scoring policy.
   * - 22-25
     - Compressed measure rests use a fixed-width H bar and count. Chord seconds and accidentals retain spelling; columns can differ because we use SMuFL cut-outs and a fixed outside-in order.
   * - 26-31
     - Ties and slurs remain attached and clear intervening ink. Curvature, endpoint clearance, beam slopes and tuplet number positions follow the bounded candidate search.
   * - 32-37
     - Articulations, fingerings, dynamic strings and lyrics are present. Libertinus text, lyric word spacing, hyphen positions and verse baselines differ from the oracle's text face and defaults.
   * - 38-42
     - Voice separation, staff groups and grace timing agree. Group braces are drawn geometrically; staff separation follows actual skylines rather than LilyPond's default spacing.
   * - 43-46
     - Written repeat structure and alternatives agree. Volta brackets have closing hooks. Tempo durations and jazz chord names use explicit text such as ``1/4 = 120`` and ``Cmaj7`` instead of a note glyph and triangle abbreviation.
   * - 47-48
     - Forced breaks and binding expansion agree. Guidedog justifies non-final systems by default while the reference wrapper keeps lines ragged right.
   * - 49-58
     - Added post-review regressions cover compound meters with multi-measure rests, courtesy clefs and key/time signatures across forced breaks, two-voice dotted seconds and unisons, concave beams, structured ``\markup`` columns/enclosures/glyphs, C-clef key signatures, ``\volta`` pass lists with ``\fine`` and ``:..:`` bars, beamed grace slurs, melismata with ``\lyricsto`` forward voices, and pitchless ``\relative`` with transposed keys.

Documentation and release checks
--------------------------------

The manual has a live music chapter beside the diagram chapter, plus directive and configuration references.
The crate map, AST, incremental-build and compatibility ledgers, API appendix, crate READMEs, seven project READMEs and changelog describe the implementation.
All six manual catalogs include translated descriptions, captions, options and limits, with no fuzzy or untranslated entries.
Korean inline boundaries use escaped whitespace so grammatical particles do not break RST delimiters.
Bravura's notice appears in ``NOTICE``, ``--licenses``, the manual's downloadable notice and release archives; the release smoke test checks it.
CI runs the corpus on Linux and macOS; every native release build runs it on its own platform before packaging, including Windows.
These workflow checks are configured here; this local qualification does not claim a remote five-platform run.

Final local qualification
-------------------------

The complete qualification ran on Linux x86-64 with Rust 1.99.0.
The implementation base is commit ``c898dbe``, including whole-run text shaping, directed hairpin termination, HTML PNG density and translated expression and repeat examples.
The following checks pass:

* ``cargo build --locked --workspace --all-targets``.
* ``cargo test --locked --workspace --no-fail-fast``: 2,082 passing tests across 205 test reports, including doctests.
* ``cargo clippy --locked --workspace --all-targets -- -D warnings``.
* ``cargo fmt --all --check``.
* ``cargo run -p check``: 1,219 source files, zero limit violations; six soft-width notices remain.
* ``cargo xtask check-music-font`` and ``cargo xtask release smoke target/debug/guidedog``, including native music in a real HTML site and PDF book and the bundled license notice.
* The 58-score SVG, PNG and geometry corpus (standard and compact editions) and the 12,000-case deterministic fuzz target.
* ``cargo test -p gd-typst --no-default-features`` for the standalone font and raster capability boundary.
* Complete HTML and PDF manual builds with ``-W`` in English, German, Spanish, Japanese, Korean, Hindi and Simplified Chinese, including the live music chapter and the API appendix.
* Catalog statistics for all six translated editions: zero fuzzy and zero untranslated entries.
* The GDS website with ``-W`` and the catalog check for all fifteen records.

We inspected the English book's music pages and all four LilyPond comparison pages.
The accepted three-phase implementation and documentation contract is complete.
The release matrices retain the platform qualification requirement before any subsequent release is packaged.

Follow-up review
----------------

The follow-up starts from ``f36e37b``, after the maintainer's reader, markup, layout and integration fixes.
The implementation is split into ``3be4c68`` (CJK qualification), ``0f0a869`` (conversion fallbacks), ``67ba13c`` (scratch accounting and layout cleanup) and ``bbf2dea`` (documentation and translations).

We reviewed the remaining findings and their error paths:

* The mixed-script score now contains Chinese and Devanagari lyrics.
  The corpus owns its downloaded CJK font bytes instead of depending on installed host fonts.
  Both download and load verify the pinned SHA-256 digest, and both staff sizes reject engraving warnings.
  We downloaded the face into a fresh directory and inspected the generated score image.
* Direct conversion describes a file argument without reading the score file.
  Integration tests cover RST and MyST input, HTML and PDF output, German localization and RST round trips.
  After a trusted project build reads the file, it removes the file-argument marker from the fallback code block so the renderer displays the actual source.
* Scoped reservations now follow skyline segment owners and the temporary spacing and breaking buffers.
  Replacement storage is charged while the old skyline is alive, and failed merges leave the original reservation intact.
  Regression tests exercise memory refusal, exhausted work, early errors and owner destruction.
  Repeated merges exceed the cumulative allocation allowance while staying within the live-memory allowance.
* Each engraving accounts for existing worker capacity because its caller can supply a new ledger.
  System setup uses the measure index rather than a possibly repeated printed number.
  Voice-column arrangement runs once, after beam directions are final.

The maintainer then extended the follow-up to complete third and fourth voices.
``a8f5e3e`` adds event-local voice policies and a four-voice model and reader limit.
``e5d2780`` records the expanded implementation and teaching-example contract.
``3aaa518`` adds bounded multi-column placement, shared-rest ownership, rest clearance and same-side tie endpoint clearance.
``fa5d455`` adds the checked Beethoven excerpt.

The implementation review found and corrected three cases beyond lifting the reader limit:

* When only voices one and three sound at a moment, their stems still need the multi-column collision search.
* Unattached flags need their own measured footprint when we place another voice's heads and stems.
* A tie on the stem side must attach beyond the actual stem edge; an interior-only curve audit can miss contact near its endpoint.

The new regressions cover all 81 assignments of three nearby pitches to four voices, four unisons with alternating stem directions, local policy reset, mixed rest durations and fifth-voice refusal.
Scores 59-62 add raster collision audits for three voices, dense four-voice seconds and accidentals, mixed rests and named voices with lyrics and a system-crossing tie.
Score 63 and a pitch-and-duration test check the manual's four-bar Beethoven excerpt against the instrumental Joy theme, finale, bars 92-95, in the CCARH score on page 8.
The manual gives copyable RST and MyST forms and explains the exact beat counts in all six translations.
The CJK download and corpus steps are configured for CI and native release jobs; local checks do not establish remote platform results.

The final direction-policy review found that an inherited policy could mask ``\stemNeutral``.
Commit ``481e87b`` separates an optional stem override from the voice's column policy, retains neutral as an explicit override, and adds a regression for neutral and forced directions in inner voices.

Independent Beethoven qualification
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

We created fresh RST and MyST projects in a temporary directory.
The RST project contains the actual manual section; the MyST project activates its published directive fence.
Both projects build HTML and PDF with warnings treated as errors.
Their complete sets of standard and compact SVGs are byte-identical.

We inspected the CCARH source score on page 8 and the generated English manual page, and compared the teaching engraving with LilyPond 2.24.4.
We transcribed the original cello melody in D major into a separate LilyPond score in the temporary directory.
An independent MIDI reader then compared that score with the teaching source: all thirteen pitches, every onset and every exact duration agree after moving down a whole tone and up an octave, a net ten semitones.
The four measures total sixteen quarter-note beats.
These working files were not kept. Score 63 and its pitch-and-duration test are the durable record, and the steps above reproduce the comparison from the cited CCARH/MuseData score.

The teaching review clarified the composition number, the change from D major to C major, and the distinction between the instrumental half-note opening and vocal versions with repeated quarters.
The lesson identifies an excerpt and its open ending; it does not present four bars as the complete symphony.

Final follow-up qualification
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The final implementation includes ``8d77245`` (covering ``481e87b``, ``61e531c`` and the subsequent polyphonic column-shift, linear beam/line-breaking, and stacked-dynamics refinements) plus the ``1.0.0-beta.10`` localized key-signature and SVG ``<title>``/``<desc>`` accessibility updates.
All of the following checks pass on Linux x86-64:

* The locked workspace build with all targets and the complete workspace test run: 2,099 passing tests across 205 reports, including doctests.
* Strict workspace Clippy, formatting and the source-limit checker: 1,223 files, zero limit violations and six existing soft-width notices.
* The Bravura digest and license check, release smoke test and standalone ``gd-typst`` tests without default features.
* All 63 golden scores at both staff sizes, including actual raster-ink collision checks, and 12,000 deterministic fuzz cases: 4,357 parsed and 3,829 engraved.
* LilyPond 2.24.4 compilation of all 63 reference scores; the comparison tool generates six review pages.
* Fresh HTML and PDF project builds containing all four new polyphonic regression scores, with warnings treated as errors.
* The independent Beethoven RST, MyST, MIDI, SVG and printed-page checks recorded above.
* Complete HTML and PDF manual builds in English, German, Spanish, Japanese, Korean, Hindi and Simplified Chinese, with warnings treated as errors.
* All six translation catalog statistics: zero fuzzy and zero untranslated entries.
* The GDS website, the selected GDS 0015 PDF and the catalog check for all fifteen records.

The four-voice extension, CJK qualification, conversion fallbacks, memory accounting, teaching example and documentation contract are complete.
