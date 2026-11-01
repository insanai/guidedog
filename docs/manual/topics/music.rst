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
Conversion to RST retains the directive and its file argument.
Direct HTML and PDF conversion keeps inline notation as code and names an unengraved file argument in a localized sentence, without opening the score file.
Engraving belongs to the project asset pipeline.

Learn from Beethoven's Joy theme
--------------------------------

We use four bars of the Joy theme from Beethoven's Symphony No. 9, Op. 125, finale, bars 92-95.
We transpose the melody from D major to C major and place it in the treble register, keeping its rhythm and omitting the orchestration, slurs and dynamics.
The `CCARH/MuseData score, printed page 8 <https://williamwieland.com/piano/reading/Beethoven/OdetoJoy.pdf>`_, shows the instrumental opening with half notes; some familiar vocal versions divide them into repeated quarter notes.

.. code-block:: rst

   .. lilypond::
      :caption: Beethoven's Joy theme, four bars in C major.
      :alt: A four-bar melody beginning E, F, G and ending with dotted-quarter E, eighth-note D and half-note D.

      \fixed c' {
        \clef treble
        \key c \major
        \time 4/4
        e2 f4 g | g4 f e d | c2 d4 e | e4. d8 d2 |
      }

.. lilypond::
   :caption: Beethoven's Joy theme, four bars in C major.
   :alt: A four-bar melody beginning E, F, G and ending with dotted-quarter E, eighth-note D and half-note D.

   \fixed c' {
     \clef treble
     \key c \major
     \time 4/4
     e2 f4 g | g4 f e d | c2 d4 e | e4. d8 d2 |
   }

In ``\fixed c'``, ``c`` is middle C and ``e`` is the E above it.
The letters name pitches; ``\key c \major`` selects a key with no sharps or flats.

In ``\time 4/4``, each bar holds four quarter-note beats.
The first bar has ``e2`` for two beats, then ``f4`` and ``g`` for one beat each; ``g`` inherits the preceding duration.
In the last bar, ``e4.`` lasts one and a half beats, ``d8`` half a beat, and ``d2`` two beats.
The final D leaves the phrase open because this is an excerpt.

For MyST, we put the same music inside the directive fence:

.. code-block:: markdown

   ```{lilypond}
   :caption: Beethoven's Joy theme, four bars in C major.
   :alt: A four-bar melody beginning E, F, G and ending with dotted-quarter E, eighth-note D and half-note D.

   \fixed c' {
     \clef treble
     \key c \major
     \time 4/4
     e2 f4 g | g4 f e d | c2 d4 e | e4. d8 d2 |
   }
   ```

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
     - ``\clef``, ``\key``, ``\time``, ``\numericTimeSignature``, ``\partial``, ``\bar``; treble, bass, alto, tenor and soprano clefs, with supported octave clefs.
   * - Modes
     - ``\major``, ``\minor``, ``\ionian``, ``\dorian``, ``\phrygian``, ``\lydian``, ``\mixolydian``, ``\aeolian``, ``\locrian``.
   * - Rhythm and stems
     - ``\tuplet 3/2``, ``\times 2/3``, manual ``[`` and ``]`` beams, ``\autoBeamOn``, ``\autoBeamOff``, ``\stemUp``, ``\stemDown``, ``\stemNeutral``.
   * - Expression
     - Articulations, fingerings 1 through 5, dynamics from ``\pppp`` through ``\ffff``, ``\<``, ``\>``, ``\!``, slur/tie/dynamic direction commands, literal scripts and structured ``\markup``.
   * - Structure
     - Assignments, variable references, lexical ``\include``, ``\score``, ``\new Staff``, ``\new Voice``, ``PianoStaff``, ``GrandStaff``, ``StaffGroup``, ``ChoirStaff``.
   * - Voices and lyrics
     - At most four voices sounding at once on a staff; ``\addlyrics``, ``\lyricsto``, ``\lyricmode``, multiple verses, explicit syllable durations, hyphens and extenders.
   * - Grace and repetition
     - ``\grace``, ``\acciaccatura``, ``\appoggiatura``, ``\afterGrace``, ``\repeat volta``, ``\alternative``, ``\volta``, ``\fine``, ``\section``, ``\repeat unfold``, ``\compressEmptyMeasures``, ``\compressFullBarRests``.
   * - Labels and breaks
     - ``\tempo``, ``\mark``, ``\mark \default``, ``\chords``, ``\new ChordNames \chordmode``, ``\break``, ``\noBreak``.

The reader accepts version declarations, ignored header metadata and empty layout or MIDI blocks.
It refuses Scheme, music functions, arbitrary property changes, overrides and unsupported commands with a placed diagnostic.
The profile describes a fixed engraving style; it does not execute a LilyPond program.
The complete command ledger and engraving contract are recorded in GDS 0015 in the repository's ``docs/gds/records`` directory.

Add expression and repeats
--------------------------

We attach articulations and fingerings after a note: ``-.`` is staccato, ``->`` is an accent, and ``-1`` selects the first finger.
Dynamics set the intensity, and a hairpin continues until ``\!`` or the next dynamic.
We place a small grace group before a main note with ``\grace``; ``\acciaccatura`` adds a slash, while ``\afterGrace`` places the group after its main note.
Grace time does not extend the measure.

.. code-block:: lilypond

   \relative c'' {
     \tempo "Andante" 4 = 88
     \grace { d16 e } f4-1-. g-> a\p\< b | c1\f |
   }

We write a volta repeat once and give each ending in ``\alternative``; ``\volta 1,2 { ... }`` labels combined endings, and ``\fine`` or ``\section`` marks a structural double bar.
The example has two passes with different final measures.
``\repeat unfold`` expands its body instead of drawing repeat signs, and ``\compressEmptyMeasures`` or ``\compressFullBarRests`` is accepted for multi-measure rests.
Chord names use a separate ``\new ChordNames \chordmode`` context beside the staff, with spellings such as ``c1:maj7``, ``d:m7`` and ``g:7``.

.. code-block:: lilypond

   \relative c' {
     \repeat volta 2 { c4 d e f | }
     \alternative { { g1 | } { c1 | } }
   }

Format text with ``\markup``
----------------------------

Scripts, ``\tempo``, ``\mark`` and instrument names accept a bounded LilyPond 2.24 ``\markup`` subset.
Commands nest freely within the reader's depth limit, and top-level markup variables expand where a markup block expects text.

.. list-table:: Supported ``\markup`` commands
   :header-rows: 1
   :widths: 28 72

   * - Group
     - Commands
   * - Lists and columns
     - ``\line``, ``\concat``, ``\column``, ``\left-column``, ``\center-column``, ``\right-column``, ``\dir-column`` and markup variable references (such as ``\codaMark``).
   * - Style and case
     - ``\bold``, ``\italic``, ``\upright``, ``\normal-text``, ``\caps``, ``\smallCaps``, ``\sub`` and ``\super``.
   * - Size
     - ``\teeny``, ``\tiny``, ``\small``, ``\smaller``, ``\large``, ``\larger``, ``\huge``, ``\fontsize #n`` and ``\abs-fontsize #pt``.
   * - Enclosures and spacing
     - ``\box``, ``\rounded-box``, ``\circle``, ``\underline``, ``\parenthesize``, ``\bracket``, ``\hspace #n``, ``\vspace #n``, ``\raise #n`` and ``\lower #n``.
   * - Music symbols
     - ``\sharp``, ``\natural``, ``\flat``, ``\doublesharp``, ``\doubleflat``, ``\fermata``, ``\segno``, ``\coda``, ``\varcoda``, ``\dynamic`` and ``\musicglyph`` for mapped Feta glyph names.

.. code-block:: lilypond

   codaMark = \markup \rounded-box \bold { "To Coda " \coda }

   \relative c'' {
     \mark \codaMark
     c4^\markup \concat { "sul " \italic "G" } d_\markup \dynamic "sfz" e f |
   }

Markup commands that run Scheme, alter layout properties, draw arbitrary paths or embed nested scores (such as ``\score``, ``\override``, ``\postscript``, ``\draw-line``, ``\path``, ``\epsfile`` and ``\markuplist``) are refused with a placed diagnostic.

Combine staves and lyrics
-------------------------

We use simultaneous contexts for a score with several staves.
Staves and staff groups accept ``instrumentName`` and ``shortInstrumentName`` in a ``\with`` block, and staves also accept ``\set Staff.instrumentName`` and ``\set Staff.shortInstrumentName``.
We separate up to four voices on one staff with ``\\`` inside ``<< ... >>``.
Voices one and three have upward stems; voices two and four have downward stems.
``\voiceOne``, ``\voiceTwo``, ``\voiceThree`` and ``\voiceFour`` select those policies explicitly, and ``\oneVoice`` restores automatic single-voice directions.
An inner voice beside another voice with the same stem direction stands at least half a notehead aside, and further when needed.
Equal rests share a glyph only where no other voice sounds.
``\shiftOn``, ``\shiftOnn`` and ``\shiftOnnn`` move a voice's note columns one, two or three levels aside, and ``\shiftOff`` sets the level to zero; a voice command restores its own level.
Among voices with one stem direction, each lower level puts a column another half notehead aside: right for upward stems, left for downward ones.
Dynamics stay below the staff by default in every voice, as in LilyPond (unless ``\dynamicUp`` or ``^`` places them above); when two voices place signs on the same side at one moment, the later one stacks further out.
At most four voices may sound at once on a staff; a fifth is refused.
Lyrics attach to a named voice, and separate ``\lyricsto`` expressions make separate verses.

We can override stems with ``\stemUp``, ``\stemDown`` and ``\stemNeutral`` without changing the voice's column policy.

.. code-block:: lilypond

   <<
     { \voiceOne c''1 } \\
     { \voiceTwo c'1 } \\
     { \voiceThree g'1 } \\
     { \voiceFour e'1 }
   >>

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
Trusted text output and fallback code blocks substitute external ``.ly`` source without engraving. When the file cannot be read, as in an untrusted build, the output names the score file in a sentence instead of showing its path as code.
A failed drawing retains source code with a warning; ``-W`` stops publication and preserves the preceding output.

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
     - Available system width, in ``pt``, ``pc``, ``mm``, ``cm``, ``Q``, ``in``, ``px``, ``em``, ``rem``, ``ex``, ``ch`` or a percentage of the builder's default width.
   * - ``justify``
     - ``auto`` keeps a short final system at its natural width; ``all`` stretches every system; ``none`` avoids stretching.

The project defaults are ``lilypond_staff_size``, ``lilypond_html_width`` and ``lilypond_pdf_width``.
Their numeric values are typographic points.
See :doc:`../reference/configuration` for the setting contract.

When a drawing is wider than 225 points, HTML also receives a narrow edition engraved at 225 points with a staff height no greater than 14 points.
The browser selects it below 40 rem through a ``picture`` element.
This edition has its own system breaks.
When its system limit prevents it from being drawn, the wide image scales to the screen.
SVG adapts its ink to the page's color scheme; PNG keeps a light background panel in dark mode.

PDF derives its measure from ``pdf_paper_size`` (``a4`` or ``letter``; any other value renders as A4) and the selected template's horizontal margin, then places music at its natural size.
Built-in templates declare their horizontal margin in their leading 256 lines with a comment such as ``// guidedog: margin-x = 31mm``.
A custom template without the declaration uses a 25 millimeter margin assumption.
``lilypond_pdf_width`` overrides the derived measure.
Oversized music scales to fit with a build warning.
A score with several systems can break between systems across PDF pages, with its caption after the last system.

Fonts, limits and references
----------------------------

Music uses the unmodified Bravura 1.482 font and SMuFL metadata under the SIL Open Font License.
Text follows ``pdf_fonts`` and ``pdf_font_paths``, with regular, bold and italic fallback faces.
Glyphs become outlines, so browsers and PDF readers need no installed music font.
PNG export requires the embedded renderer; SVG engraving remains available in builds without it.

For CJK lyrics, we include a suitable CJK text face through ``pdf_font_paths``.
When building this repository, ``cargo xtask fetch-fonts --music`` installs the pinned Noto Serif CJK face used by the mixed-script music corpus in ``docs/shared/fonts``.
We can use that directory as a font path in our project.

One example accepts at most 256 KiB of source including includes, 32 included files, depth 64, 20,000 events, 24 staves, four voices sounding at once on a staff and 16 heads per chord.
The reader and engraver each have a work limit of eight million operations.
Expanded reader allocations have a separate 64 MiB allowance.
A drawing has at most 64 systems and a height of 20,000 points.
PNG is limited to 16 million pixels and 16,384 pixels on either side.
A limit diagnostic identifies the source and suggests splitting the example.

For notation syntax, we consult the `LilyPond 2.24 notation reference <https://lilypond.org/doc/v2.24/Documentation/notation/>`_.
For glyph definitions, we consult the `Standard Music Font Layout specification <https://www.w3.org/2021/03/smufl14/>`_ and the `Bravura project <https://github.com/steinbergmedia/bravura>`_.
Engraving references include Elaine Gould's *Behind Bars* and Ted Ross's *The Art of Music Engraving and Processing*.
For other kinds of drawing, we use :doc:`diagrams`.
