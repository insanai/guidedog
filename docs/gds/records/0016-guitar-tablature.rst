:gds-number: 0016
:gds-title: Guitar tablature
:gds-state: accepted
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-11-02
:gds-updated: 2026-11-02
:gds-discussion: No external discussion URL assigned
:gds-labels: ["parser","renderer"]

Guitar tablature
================

Abstract
--------

We extend the native LilyPond profile of GDS 0015 with guitar tablature.
``\new TabStaff`` draws six string lines, a TAB clef and a fret number for each note, and ``\1`` to ``\6`` name the string that plays a note.
Guidedog places each note on a string as LilyPond does by default, and draws the staff as LilyPond's default tablature: strings, fret numbers and bar lines, without rhythm.
A guitar part can therefore show standard notation and tablature together, as most guitar books do.

Motivation and scope
--------------------

GDS 0015 listed tablature among its non-goals.
Guitar teaching material, songbooks and method books, however, almost always pair the staff with tablature, and a reader who learns from tablature cannot use a book that lacks it.
A Guidedog songbook that teaches a song for voice, piano and guitar needs it.

The proposal covers a six-string guitar in standard tuning (E2, A2, D3, G3, B3, E4) and LilyPond's default tablature view.
Other tunings and instruments, ``\tabFullNotation`` (stems and rhythm in the tab staff), fret diagrams, ``minimumFret`` and techniques such as bends, slides, hammer-ons and harmonics remain outside the profile.

Specification
-------------

*Model.*
``gd_score::Staff`` gains ``kind: StaffKind``, either ``Standard`` (the default) or ``Tablature``.
``gd_score::Note`` gains ``string: Option<u8>``, the string that plays the note, 1 for the highest.
``gd_score::GUITAR_TUNING`` lists the open strings from string 1 to string 6.
``gd_score::tab_positions(&[Note])`` returns a ``TabPosition { string, fret }`` per note.
The reader uses it to refuse unplayable notes, ``check_score`` uses it to validate tab staves, and the engraver uses it to draw them, so all three agree.

*Placement.*
As LilyPond's default does, a note with a string number keeps that string, and every other note of the chord, from the highest pitch down, takes the highest-numbered free string, counting from string 1, whose fret lies between 0 and 24.
For example, the chord ``<c e g c' e'>`` becomes frets 3, 2, 0, 1 and 0 on strings 5 to 1, and ``e''`` alone becomes fret 12 on string 1.
Placement fails, with a placed diagnostic, when a string number is outside 1 to 6, when two notes of a chord name one string, when the named string cannot reach the pitch, or when no free string can.

*Reader.*
``\new TabStaff`` opens a staff of kind ``Tablature`` and accepts ``\with`` instrument names like ``\new Staff``; ``\new TabVoice`` opens a voice in it.
A string number follows a note (``c'4\2``) or a note inside a chord (``<c\5 e\4>2``).
A string number after a whole chord is refused as ambiguous, with a hint to move it inside the angle brackets.
``q`` repeats a chord's pitches without their string numbers.
On a standard staff a string number prints as a circled digit, as LilyPond prints it: above the staff, or below it in a voice with downward stems, stacked outward from the staff.

*Engraving.*
A tab staff has six lines a staff space and a half apart, a TAB clef scaled to the staff, and bar lines and repeat dots at its height.
Each fret number is set in the text face and centered on its string, over the middle of where a notehead would stand, so that it lines up with the standard staff above.
The string lines break around each number instead of being covered by a background patch, so the numbers read in light and dark themes alike.
As in LilyPond's default tablature, the staff shows no key or time signature, stems, beams, flags, dots, rests, ties, slurs, hairpins, tuplet brackets, articulations, fingerings or dynamics; tempo and rehearsal marks belong to the whole score and remain, and a breath mark stands above the top string, as LilyPond prints it.
The note that continues a tie is not repeated, except at the start of a line, where its number stands in parentheses.
Grace notes have smaller numbers.
Voices on a tab staff share strings, so no note column moves aside.
Fret numbers take part in horizontal spacing and vertical skylines like noteheads.

Rationale and alternatives
--------------------------

We follow LilyPond's defaults so that a file written for LilyPond engraves the same way here, and so that we need no new configuration.
Deriving frets in ``gd-score``, rather than in the reader or the engraver alone, keeps one rule for validation and drawing.
A white patch behind each number, as LilyPond draws, would show as a box in a dark theme; broken lines need no background color.
Showing rhythm in the tab staff is ``\tabFullNotation`` in LilyPond and remains a possible extension; the paired standard staff already shows rhythm.

Security and operational considerations
---------------------------------------

The input surface is two context names and single-digit commands.
Placement is linear in the chord's notes with six strings, and all work, events and memory stay under GDS 0015's limits.
The music corpus, the fuzz seeds and the property tests cover tablature, including unplayable input.

Backwards compatibility
-----------------------

Existing scores engrave as before; every staff defaults to ``StaffKind::Standard``.
``Staff`` and ``Note`` gain public fields and ``Role`` gains ``Fret``, so library callers that build these types with struct literals or match ``Role`` exhaustively must add them; the 1.0 beta versioning permits this.

Acceptance criteria and implementation status
---------------------------------------------

Implemented for 1.0.0-beta.10 and awaiting release review:

* Placement unit tests, including an open C major chord, named strings and every refusal.
* Reader tests for ``TabStaff``, ``TabVoice``, string numbers and diagnostics.
* Engraver tests for six strings, centered fret numbers, the omitted marks and tie continuations across a line break.
* Score 64 of the music corpus, a guitar part in standard notation and tablature with two voices, a chord, a grace note, a string number and a tie across a line break, checked for geometry, SVG and PNG pixels.
* Tablature seeds in the 12,000-case fuzz run.
* The manual's music chapter and a practical task that writes a songbook for voice, piano and guitar with tablature, in all six translations.

Open questions
--------------

Other tunings and ``\tabFullNotation`` are candidates for later revisions.

Review history
--------------

On 2 November 2026 the maintainer asked for tablature beside the piano and vocal parts of a songbook chapter; this record proposes it and records the implementation.
The same day the maintainer asked to close the gaps the first implementation left: string numbers now print as circled digits on standard staves, and GDS 0015's profile gains breath marks and metronome marks with note symbols.
Checking the songbook against LilyPond 2.24 then showed two more differences from LilyPond's defaults, and GDS 0015 now follows them: 4/4 prints as C and 2/2 as cut C, and chord names raise their extensions, as in G with a raised 7.

References
----------

* LilyPond Notation Reference 2.24, section 2.4.1, Common notation for fretted strings: default tablatures, string number indications and automatic fret assignment.
* SMuFL 1.4, glyph ``6stringTabClef`` (U+E06D), and the Bravura font.
* GDS 0015, Engraved LilyPond music.

.. rubric:: Lifecycle event

::

   2026-11-02: prediscussion -> discussion. Assigned a permanent number and
   opened for discussion.

.. rubric:: Lifecycle event

::

   2026-11-02: discussion -> accepted. The maintainer asked for tablature beside
   the piano and vocal parts of a songbook; the design follows LilyPond's
   default tablature and is implemented, awaiting release review.
