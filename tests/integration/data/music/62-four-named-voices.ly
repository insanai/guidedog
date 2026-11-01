\new Staff <<
  \new Voice = "top" { \voiceOne c''2 d''~ | \break d''1 | }
  \new Voice = "bottom" { \voiceTwo c'1 | b1 | }
  \new Voice = "inner-up" { \voiceThree g'8( a' b' c'') g'2 | g'1 | }
  \new Voice = "inner-down" { \voiceFour e'2 f' | d'1 | }
>>
\lyricsto "inner-up" { Sing line now }
