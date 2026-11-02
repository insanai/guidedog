Write a songbook
================

We can keep a songbook in Guidedog: the explanations in RST or Markdown, the music as text beside them, and one command that engraves the scores for a website and a printed book.
In this task we write a complete song for voice, piano and guitar, with words, chord names and guitar tablature, and explain each part for students.
We use *Mary Had a Little Lamb*: the tune is a traditional American melody, and the words are by Sarah Josepha Hale, published in 1830; both are in the public domain.
:doc:`../topics/music` describes every notation command we use.

Start the project
-----------------

We create a project and a folder for the scores.

.. code-block:: sh

   guidedog quickstart songbook
   mkdir songbook/scores

Each song gets a page, and each score a ``.ly`` file in ``scores``.
The page explains the music; the file holds it.
A score in its own file is easy to reuse on several pages, to compare in version control, and to open in LilyPond itself.
We list the song's page in the ``toctree`` of ``index.rst``:

.. code-block:: rst

   Songbook
   ========

   .. toctree::

      mary

We number the figures so that the text can refer to them, with this line in ``conf.toml``:

.. code-block:: toml

   numfig = true

Write the tune
--------------

A tune is a sequence of pitches, each with a duration.
We write it inside ``\fixed c' { ... }``, where a letter is a pitch in the octave of middle C: ``c`` is middle C, ``d`` the D above it, and so on up to ``b``.
An apostrophe raises a pitch by an octave, and a comma lowers it.

A number after a pitch is its duration: ``1`` is a whole note of four beats, ``2`` a half note of two beats, and ``4`` a quarter note of one beat.
A pitch without a number keeps the duration of the pitch before it.
``\key c \major`` gives the key, C major, which has no sharps or flats, and ``\time 4/4`` gives the time signature: four quarter-note beats in each bar.
A vertical bar ``|`` marks the end of a bar; Guidedog checks that each bar holds exactly four beats.
``\bar "|."`` draws the final double bar.

.. lilypond::
   :caption: The tune of *Mary Had a Little Lamb*.
   :name: songbook-tune
   :alt: Eight bars in C major and four-four time, starting E, D, C, D, and ending on a whole-note C.

   \fixed c' {
     \key c \major \time 4/4
     e4 d c d | e e e2 | d4 d d2 | e4 g g2 |
     e4 d c d | e e e e | d d e d | c1 \bar "|."
   }

:numref:`songbook-tune` has eight bars.
The tune moves mostly by step, between the neighboring notes C, D and E, and leaps once, from E up to G in bar four.
Count the beats in bar two: ``e`` and ``e`` take one beat each, and ``e2`` takes two, which makes four.

Add the words
-------------

A singer reads the words under the notes, one syllable under each note.
We write the words in ``\lyricmode``, with spaces between the syllables.
Two hyphens, ``--``, join the syllables of one word, and Guidedog draws a short hyphen between them.
The tune has 26 notes, so each verse needs exactly 26 syllables:

.. code-block:: lilypond

   Ma -- ry had a lit -- tle lamb, lit -- tle lamb, lit -- tle lamb,
   Ma -- ry had a lit -- tle lamb, its fleece was white as snow.

To attach the words, we name the voice that sings the tune, ``\new Voice = "tune"``, and give each verse its own ``\new Lyrics \lyricsto "tune"``.
Guidedog places the syllables under the notes of that voice in order, and puts the verses one below the other.

.. lilypond::
   :caption: The vocal part with two verses.
   :name: songbook-voice
   :alt: The tune on one staff with two lines of words below: Mary had a little lamb, and Everywhere that Mary went.

   <<
     \new Staff \with { instrumentName = "Voice" } {
       \new Voice = "tune" \fixed c' {
         \key c \major \time 4/4
         e4 d c d | e e e2 | d4 d d2 | e4 g g2 |
         e4 d c d | e e e e | d d e d | c1 \bar "|."
       }
     }
     \new Lyrics \lyricsto "tune" {
       Ma -- ry had a lit -- tle lamb, lit -- tle lamb, lit -- tle lamb,
       Ma -- ry had a lit -- tle lamb, its fleece was white as snow.
     }
     \new Lyrics \lyricsto "tune" {
       Ev -- ery -- where that Ma -- ry went, Ma -- ry went, Ma -- ry went,
       ev -- ery -- where that Ma -- ry went, the lamb was sure to go.
     }
   >>

Hale's poem has more verses, but the next, "It followed her to school one day", has five syllables where the first bar has four notes.
Singers divide a note for the extra syllable; a score shows that with a different rhythm, so our songbook keeps the two verses that fit the tune as written.

Name the chords
---------------

Guitarists and pianists who accompany a singer read chord names above the staff.
We write them in ``\chordmode``: ``c1`` is a C major chord lasting a whole bar, ``g1`` a G major chord, and ``g:7`` a G seventh chord.
The song needs only these three chords: C under the notes C, E and G, and G or G7 under D and B.
A ``\new ChordNames`` context draws the names above the next staff:

.. code-block:: lilypond

   \new ChordNames \chordmode { c1 | c | g | c | c | c | g:7 | c }

Write the piano part
--------------------

A piano part has two staves joined by a brace: ``\new PianoStaff``.
The upper staff is for the right hand, and the lower staff, with ``\clef bass``, for the left hand.

The right hand plays the tune and, below it, a second voice a third lower.
Two voices on one staff go inside ``<< ... >>``, separated by ``\\``.
Guidedog draws the first voice with stems up and the second with stems down, so a player can follow each line.
The left hand plays the bass: the root and the fifth of each chord, as half notes.

.. lilypond::
   :caption: The piano part: tune and harmony in the right hand, bass in the left.
   :name: songbook-piano
   :alt: Two piano staves; the right hand plays the tune with a second voice a third below, and the left hand plays half notes on C and G.

   \new PianoStaff \with { instrumentName = "Piano" } <<
     \new Staff \fixed c' {
       \key c \major \time 4/4
       << { e4 d c d | e e e2 | d4 d d2 | e4 g g2 |
            e4 d c d | e e e e | d d e d | c1 \bar "|." }
          \\
          { c4 b, a, b, | c c c2 | b,4 b, b,2 | c4 e e2 |
            c4 b, a, b, | c c c c | b, b, c b, | g,1 } >>
     }
     \new Staff \fixed c {
       \clef bass \key c \major \time 4/4
       c2 g, | c2 g, | g,2 d | c2 g, | c2 g, | c2 g, | g,2 b, | c1 \bar "|."
     }
   >>

Write the guitar part and its tablature
---------------------------------------

Guitar music is written an octave above the sound, on a staff with ``\clef "treble_8"``; the small 8 under the clef says so.
Our guitar part plays two voices on one staff, as a fingerstyle player does: the tune with stems up, and the bass with stems down.

Under the staff we add tablature with ``\new TabStaff``.
Its six lines are the six strings of the guitar, with the highest string at the top, and each number is the fret to press on that string; ``0`` is the open string.
We give the tablature the same music as the staff, and Guidedog chooses the strings.
It puts each note on the highest string that can play it, so the tune lies on the two highest strings and the bass on the two lowest, all in the first position of the left hand.

.. lilypond::
   :caption: The guitar part in standard notation and tablature.
   :name: songbook-guitar
   :alt: A guitar staff with the tune and a bass, above tablature; the first bar is frets 0, 3, 1 and 3 on the high strings with 3 and 3 on the low strings.

   tune = \fixed c' {
     \key c \major \time 4/4
     e4 d c d | e e e2 | d4 d d2 | e4 g g2 |
     e4 d c d | e e e e | d d e d | c1 \bar "|."
   }
   bass = \fixed c { c2 g, | c2 g, | g,2 d | c2 g, | c2 g, | c2 g, | g,2 b, | c1 }
   \new StaffGroup \with { instrumentName = "Guitar" } <<
     \new Staff { \clef "treble_8" << \tune \\ \bass >> }
     \new TabStaff << \tune \\ \bass >>
   >>

In the first bar of :numref:`songbook-guitar` the tune's E is the open first string, D the third fret of the second string, and C its first fret; the bass C is the third fret of the fifth string, and G the third fret of the sixth.
To play a note on another string, we add a string number after it: ``e4\2`` puts the E on the second string, at the fifth fret.
The tablature leaves out the rhythm, which the staff above shows.

Combine the parts in one score
------------------------------

The complete score stacks the parts from top to bottom: chord names, the voice with its verses, the piano and the guitar.
To write each line of music once, we give it a name, such as ``melody = ...``, and use it with ``\melody`` wherever a part needs it.
A line starting with ``%`` is a comment for the reader of the file.
We save the score as ``scores/mary-had-a-little-lamb.ly``:

.. literalinclude:: scores/mary-had-a-little-lamb.ly
   :language: lilypond

On the song's page, ``mary.rst``, the directive names the file instead of holding the music:

.. code-block:: rst

   Mary Had a Little Lamb
   ======================

   .. lilypond:: scores/mary-had-a-little-lamb.ly
      :caption: *Mary Had a Little Lamb*, for voice, piano and guitar.
      :name: mary
      :alt: A score for voice, piano and guitar with chord names and tablature, eight bars in C major.

.. lilypond:: scores/mary-had-a-little-lamb.ly
   :caption: *Mary Had a Little Lamb*, for voice, piano and guitar.
   :name: songbook-score
   :alt: A score for voice, piano and guitar with chord names and tablature, eight bars in C major.

Build the website and the book
------------------------------

.. code-block:: sh

   guidedog build html songbook
   guidedog build pdf songbook

Guidedog engraves each score for each output at that output's line width, so the website and the book each break the music into systems that fill their lines.
On a narrow phone screen, the website shows an edition engraved for the narrow width.
In the book, a long score breaks between systems across pages, with its caption after the last system.
When we change a score file, the next build engraves it again; unchanged scores are kept.

Practice
--------

These exercises extend the songbook.
Build after each change and look at the result.

#. Move the song to G major: wrap the whole score in ``\transpose c g << ... >>``. The key signature, the notes, the tablature and the chord names all move up a fifth.
#. Play the guitar tune higher on the neck: add ``\2`` after each note of the first bar, and read the new frets in the tablature.
#. Add dynamics to the tune: write ``e4\mf`` for its first note and ``c1\p`` for its last. A dynamic follows the note it belongs to.
#. Write a second song in its own file and page, and refer to its figure from this page with ``:numref:``.
