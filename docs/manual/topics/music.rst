Music notation
==============

We keep a musical example beside its explanation and let Guidedog engrave it for the website and the book.
The ``lilypond`` directive reads a bounded LilyPond notation profile and produces self-contained SVG or PNG.
The reader and engraver are native Rust libraries.
A project build needs no installed LilyPond program.

Write a melody
--------------

We write pitches and durations inside braces.
In ``\relative c'``, each pitch takes the nearest diatonic position to the preceding pitch, then applies its apostrophes or commas.
The first pitch uses the reference ``c'``, which is middle C.
A number gives the duration's denominator; ``4`` is a quarter note and ``8`` is an eighth note.
An omitted duration repeats the preceding duration.
A vertical bar checks the measure boundary.

.. code-block:: rst

   .. lilypond::
      :caption: A short melody in G major.
      :name: music-melody
      :alt: A four-measure melody in G major and three-quarter time.

      \relative c'' {
        \key g \major \time 3/4
        d4 g,8( a b c) | d4 g, g |
        e' c8( d e fis) | g4 g, g |
      }

.. lilypond::
   :caption: A short melody in G major.
   :name: music-melody
   :alt: A four-measure melody in G major and three-quarter time.

   \relative c'' {
     \key g \major \time 3/4
     d4 g,8( a b c) | d4 g, g |
     e' c8( d e fis) | g4 g, g |
   }

We refer to the caption with ``:numref:`music-melody``` when ``numfig = true``.
A caption accepts inline markup.
Without a caption, the directive produces an image.

MyST uses the same directive and options.

.. code-block:: markdown

   ```{lilypond}
   :caption: A rising scale.
   :alt: An ascending C major scale in eighth notes.

   \relative c' { c8 d e f g a b c | }
   ```

An ordinary ``code-block:: lilypond`` or plain Markdown ``lilypond`` fence remains a source example.
A direct ``guidedog convert`` operation also retains directive source; engraving belongs to the project asset pipeline.

Choose the notation
-------------------

The default pitch language is Nederlands: ``cis`` is C sharp and ``des`` is D flat.
We select English names with ``\language "english"`` or ``\include "english.ly"``; examples include ``cs`` and ``df``.
Deutsch adds ``h`` for B natural and ``b`` for B flat.
These three language includes are built in and read no file.

Apostrophes raise octaves and commas lower them.
``\fixed c' { ... }`` fixes the reference octave instead of following preceding pitches.
``\transpose c ees { ... }`` transposes spelled pitches and key signatures.
Accidentals belong to the pitch name; a key signature does not change a written natural pitch into a sharp pitch.
An exclamation mark forces an accidental, and a question mark requests a cautionary accidental.
We put these marks after the pitch and before its duration: ``cis'!4`` and ``c'?4``.

Durations range from ``1`` through ``128``, with ``\breve`` for a double whole note.
Dots lengthen a duration, and ``*2/3`` scales its exact value.
``r`` is a rest, ``s`` is an invisible spacer, and ``R1*4`` requests four measures of rest in common time.
Angle brackets make chords: ``<c' e' g'>4``.
``q`` repeats the preceding chord.
A tilde ties equal pitches; parentheses make slurs, and ``\(`` with ``\)`` makes a phrasing slur.

.. list-table:: Supported notation families
   :header-rows: 1
   :widths: 25 75

   * - Family
     - Examples
   * - Staff context
     - ``\clef``, ``\key``, ``\time``, ``\partial``, ``\bar``; treble, bass, alto, tenor and soprano clefs, with supported octave clefs.
   * - Modes
     - ``\major``, ``\minor``, ``\ionian``, ``\dorian``, ``\phrygian``, ``\lydian``, ``\mixolydian``, ``\aeolian``, ``\locrian``.
   * - Rhythm and stems
     - ``\tuplet 3/2``, ``\times 2/3``, manual ``[`` and ``]`` beams, ``\autoBeamOn``, ``\autoBeamOff``, ``\stemUp``, ``\stemDown``, ``\stemNeutral``.
   * - Expression
     - Articulations, fingerings 1 through 5, dynamics from ``\pppp`` through ``\ffff``, ``\<``, ``\>``, ``\!``, literal scripts and single-item ``\markup`` with ``\bold`` or ``\italic``.
   * - Structure
     - Assignments, variable references, lexical ``\include``, ``\score``, ``\new Staff``, ``\new Voice``, ``PianoStaff``, ``GrandStaff``, ``StaffGroup``, ``ChoirStaff``.
   * - Voices and lyrics
     - At most two voices per staff; ``\addlyrics``, ``\lyricsto``, ``\lyricmode``, multiple verses, explicit syllable durations, hyphens and extenders.
   * - Grace and repetition
     - ``\grace``, ``\acciaccatura``, ``\appoggiatura``, ``\afterGrace``, ``\repeat volta``, ``\alternative``, ``\repeat unfold``.
   * - Labels and breaks
     - ``\tempo``, ``\mark``, ``\mark \default``, ``\new ChordNames \chordmode``, ``\break``, ``\noBreak``.

The reader accepts version declarations, ignored header metadata and empty layout or MIDI blocks.
It refuses Scheme, music functions, arbitrary property changes, overrides and unsupported commands with a placed diagnostic.
The profile describes a fixed engraving style; it does not execute a LilyPond program.
The complete command ledger and engraving contract are recorded in GDS 0015 in the repository's ``docs/gds/records`` directory.

Combine staves and lyrics
-------------------------

We use simultaneous contexts for a score with several staves.
Instrument names use ``instrumentName`` and ``shortInstrumentName`` in a ``\with`` block.
Two voices on one staff use ``<< { ... } \\ { ... } >>``.
Lyrics attach to a named voice, and separate ``\lyricsto`` expressions make separate verses.

.. lilypond::
   :caption: A named melody with a verse and a bass staff.
   :alt: Two piano staves; the upper staff has four quarter notes and the words Sing a short song, while the bass holds C.

   \new PianoStaff <<
     \new Staff \with { instrumentName = "Piano" }
     \new Voice = "melody" { c'4 d' e' f' | }
     \new Staff { \clef bass c1 | }
   >>
   \lyricsto "melody" { Sing a short song }

Read local files
----------------

A directive can name a local file instead of containing an inline body.
We use exactly one source form.

.. code-block:: rst

   .. lilypond:: scores/example.ly
      :caption: A reusable score.
      :alt: A description of the musical example.

Nested ``\include`` paths resolve beside the including file, then through the source directory and configured ``include_roots``.
Resolved paths must stay inside the allowed roots, including after symbolic links are checked.
The build tracks source files, nested includes, missing local lookups and selected text font files by content.
Changing a dependency rebuilds the generated image.
Identical output bytes retain the same asset name.

``--untrusted`` permits inline notation because the reader runs no program or Scheme code.
It refuses file arguments and filesystem includes before opening them.
Text output retains source code and performs no engraving.
A failed drawing also retains source code with a warning; ``-W`` stops publication and preserves the preceding output.

Set size and output
-------------------

.. list-table:: Directive options
   :header-rows: 1
   :widths: 25 75

   * - Option
     - Meaning
   * - ``alt``
     - Musical description for the image; otherwise the build constructs a localized summary.
   * - ``caption``, ``name``, ``class``, ``align``
     - Figure caption, reference target, CSS classes and image alignment, following the diagram directives.
   * - ``format``
     - ``svg`` by default, or ``png`` at 144 pixels per inch.
   * - ``staff-size``
     - Height of a five-line staff, from 10 through 40 points; the default is 18 points.
   * - ``width``
     - Available system width, in ``pt``, ``mm``, ``cm``, ``in``, ``px`` or a percentage of the builder's default width.
   * - ``justify``
     - ``auto`` keeps a short final system at its natural width; ``all`` stretches every system; ``none`` avoids stretching.

The project defaults are ``lilypond_staff_size``, ``lilypond_html_width`` and ``lilypond_pdf_width``.
Their numeric values are typographic points.
See :doc:`../reference/configuration` for the setting contract.

HTML also receives a narrow edition engraved at 225 points, with a staff height no greater than 14 points.
The browser selects it below 40 rem through a ``picture`` element.
This edition has its own system breaks.
When its system limit prevents it from being drawn, the wide image scales to the screen.
SVG adapts its ink to the page's color scheme; PNG keeps a light background panel in dark mode.

PDF uses the selected template's text width and places music at its natural size.
Built-in templates declare their horizontal margin after the initial import with a comment such as ``// guidedog: margin-x = 31mm``.
A custom template without the declaration uses a 25 millimetre margin assumption.
``lilypond_pdf_width`` overrides the derived measure.
Oversized music scales to fit with a build warning.
A score with several systems can break between systems across PDF pages, with its caption after the last system.

Fonts, limits and references
----------------------------

Music uses the unmodified Bravura 1.482 font and SMuFL metadata under the SIL Open Font License.
Text follows ``pdf_fonts`` and ``pdf_font_paths``, with regular, bold and italic fallback faces.
Glyphs become outlines, so browsers and PDF readers need no installed music font.
PNG export requires the embedded renderer; SVG engraving remains available in builds without it.

One example accepts at most 256 KiB of source including includes, 32 included files, depth 64, 20,000 events, 24 staves, two voices per staff and 16 heads per chord.
The reader and engraver each have a work limit of eight million operations.
Expanded reader allocations have a separate 64 MiB allowance.
A drawing has at most 64 systems and a height of 20,000 points.
PNG is limited to 16 million pixels and 16,384 pixels on either side.
A limit diagnostic identifies the source and suggests splitting the example.

For notation syntax, we consult the `LilyPond 2.24 notation reference <https://lilypond.org/doc/v2.24/Documentation/notation/>`_.
For glyph definitions, we consult the `Standard Music Font Layout specification <https://www.w3.org/2021/03/smufl14/>`_ and the `Bravura project <https://github.com/steinbergmedia/bravura>`_.
Engraving references include Elaine Gould's *Behind Bars* and Ted Ross's *The Art of Music Engraving and Processing*.
For other kinds of drawing, we use :doc:`diagrams`.
