% A guitar part in standard notation and default tablature: two voices, a chord,
% a string number, a grace note, and a tie across a line break.
music = {
  \time 4/4
  << { e'4 d' c' d' } \\ { c2 g, } >> |
  <c e g>2 \grace a16 b4 c'\2 |
  << { e'2 g'4 g' } \\ { c1 } >> |
  e'1~ \break
  e'2 r2 \bar "|."
}
\new StaffGroup <<
  \new Staff \with { instrumentName = "Guitar" } { \clef "treble_8" \music }
  \new TabStaff { \music }
>>
