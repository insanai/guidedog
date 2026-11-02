Guitar tablature implementation review, 2 November 2026
=======================================================

We reviewed GDS 0016 against its implementation in ``gd-score``, ``gd-lilypond`` and ``gd-engrave`` and against LilyPond 2.24.4, and qualified it in the published 1.0.0-beta.10 release.
Placement lives in ``gd_score::tab_positions``, which the reader, ``check_score`` and the engraver share, so validation and drawing cannot disagree.

Scope and evidence
------------------

The record's whole specification is implemented: tab staves and voices, string numbers on notes and inside chords, LilyPond's default placement for a six-string guitar in standard tuning, placed diagnostics for every refusal, and the default tablature view.
The implementation is ``74d4a55`` (model and placement), ``14e1230`` (reader), ``caeccea`` (engraver), ``a275e6f`` (diagnostic hints), ``d56b357`` (corpus score and fuzz seeds) and ``efaeb6b`` (circled string numbers on standard staves).

The tests cover each rule of the record:

* Placement: single notes take the highest string that can play them; the chord ``<c e g c' e'>`` becomes frets 3, 2, 0, 1 and 0; named strings are kept; an out-of-range string, two notes on one string, a string that cannot reach its pitch and a chord with no free string all fail.
* Reader: ``\new TabStaff`` and ``\new TabVoice`` reach the score with their notes and string numbers, and a string number after a whole chord, an impossible string and an unplayable note are refused with a hint.
* Engraver: six strings a space and a half apart; fret numbers centered on their strings; no rests, scripts, ties, signatures or rhythm on the tab staff; a tie across a line break repeats its number in parentheses; a solo tab staff spaces its outer strings and leaves no gap for omitted signatures; breath marks stand above the top string; string numbers are circled on the staff and absent from the tablature.
* Corpus: score 64 is a guitar part in standard notation and tablature with two voices, a chord, a grace note, a string number and a tie across a line break; score 65 is the songbook's *Mary Had a Little Lamb* for voice, piano and guitar; score 66 adds breath marks and circled string numbers. Each is checked for geometry, ink collisions, SVG bytes and PNG pixels at both staff sizes.
* Fuzz: four tablature seeds join the 12,000-case deterministic run, which parsed 4,186 inputs and engraved 3,822 within their budgets.

Critique and corrections
------------------------

* The first hints for unplayable notes and ambiguous string numbers said what was wrong but not what to do.
  They now name the fix: use ``\1`` through ``\6``, or move the note between the lowest and highest playable pitch.
* Bar lines on a tab staff took a standard staff's collision band, so frets on the outer strings could crowd them.
  Bars now take the tab staff's height.
* A tied continuation that may open a line reserved no room for its parenthesized fret, and a staff group of tablature alone reserved room for the signatures it does not draw.
  The continuation now reserves the parenthesized width, and the solo tab staff leaves no signature gap.
* A breath mark could touch an augmentation dot, a flag, a breve or a shifted voice; it now clears the rightmost ink of its column.
* LilyPond's default tablature prints breath marks above the top string, while the first implementation dropped them.
  Tab staves now print them, the record says so, and a test covers it.

LilyPond comparison
-------------------

We compiled the songbook's score with LilyPond 2.24.4 and compared it bar by bar with Guidedog's engraving.
The notes, lyrics, chord names, breath marks and every fret and string agree.
The comparison also found two differences from LilyPond's defaults outside this record: 4/4 printed as numbers and G7 without a raised 7.
GDS 0015 now follows LilyPond for both, and the songbook shows C and G with a raised 7.

Three differences remain by design.
String lines break around fret numbers instead of hiding behind a white patch, so the numbers read in dark themes.
The TAB clef is the SMuFL ``6stringTabClef`` glyph from Bravura rather than LilyPond's own drawing.
Chord names use the text face, where LilyPond uses a sans-serif face.

Remaining gaps
--------------

Other tunings and instruments, ``\tabFullNotation``, fret diagrams, ``minimumFret`` and techniques such as bends, slides, hammer-ons and harmonics stay outside the profile, as the record states.

Qualification
-------------

The implementation was published in `Guidedog 1.0.0-beta.10 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.10>`_ from commit ``8985e5c``.
Its Verify run passed 2,138 tests on Linux x86-64 and 2,137 on macOS Apple silicon, with Clippy, formatting, the source limits, the 66-score corpus and the fuzz run.
The release's sixteen jobs passed, including smoke tests of the five platform archives, and the Homebrew tap installed and tested it on both macOS architectures.
The manual's music chapter and the songbook task teach tablature in all seven language editions, whose catalogs have no fuzzy or untranslated entries, and every edition builds with warnings treated as errors.
