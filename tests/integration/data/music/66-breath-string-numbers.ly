% Vocal breath marks and a metronome mark above a guitar part whose string numbers
% appear circled on the staff and choose the strings of the tablature.
guitar = { \time 4/4 e'4\2 d' c'\3 b | <c\5 e\4 g>2 << { e'4\2 d' } \\ { c2\5 } >> | }
<<
  \new Staff \with { instrumentName = "Voice" } {
    \new Voice = "song" \relative c'' {
      \tempo "Andante" 4 = 72
      \time 4/4 c4 d e2 \breathe | f4 e d2 \bar "|."
    }
  }
  \new Lyrics \lyricsto "song" { Sing a song, breathe and sing. }
  \new StaffGroup \with { instrumentName = "Guitar" } <<
    \new Staff { \clef "treble_8" \guitar }
    \new TabStaff { \guitar }
  >>
>>
