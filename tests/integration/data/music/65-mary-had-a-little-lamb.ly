% Mary Had a Little Lamb, for voice, piano and guitar.
% Traditional American tune; words by Sarah Josepha Hale (1830).

% The tune, in C major and four-four time, written once and used by every part,
% with a breath mark at the end of its first phrase.
melody = \fixed c' {
  \key c \major \time 4/4
  e4 d c d | e e e2 | d4 d d2 | e4 g g2 \breathe |
  e4 d c d | e e e e | d d e d | c1 \bar "|."
}

% A second voice for the pianist's right hand, mostly a third below the tune.
harmony = \fixed c' {
  c4 b, a, b, | c c c2 | b,4 b, b,2 | c4 e e2 |
  c4 b, a, b, | c c c c | b, b, c b, | g,1
}

% The bass, two beats a note: mostly the root and fifth of each chord, with B
% leading to the final C.
bass = \fixed c {
  c2 g, | c2 g, | g,2 d | c2 g, |
  c2 g, | c2 g, | g,2 b, | c1
}

% One syllable for each of the tune's 26 notes.
verseOne = \lyricmode {
  Ma -- ry had a lit -- tle lamb, lit -- tle lamb, lit -- tle lamb,
  Ma -- ry had a lit -- tle lamb, its fleece was white as snow.
}
verseTwo = \lyricmode {
  Ev -- ery -- where that Ma -- ry went, Ma -- ry went, Ma -- ry went,
  ev -- ery -- where that Ma -- ry went, the lamb was sure to go.
}

<<
  \new ChordNames \chordmode { c1 | c | g | c | c | c | g:7 | c }
  \new Staff \with { instrumentName = "Voice" } {
    \new Voice = "tune" { \tempo "Moderato" 4 = 100 \melody }
  }
  \new Lyrics \lyricsto "tune" \verseOne
  \new Lyrics \lyricsto "tune" \verseTwo
  \new PianoStaff \with { instrumentName = "Piano" } <<
    \new Staff << \melody \\ \harmony >>
    \new Staff { \clef bass \key c \major \time 4/4 \bass }
  >>
  \new StaffGroup \with { instrumentName = "Guitar" } <<
    \new Staff { \clef "treble_8" << \melody \\ \bass >> }
    \new TabStaff << \melody \\ \bass >>
  >>
>>
