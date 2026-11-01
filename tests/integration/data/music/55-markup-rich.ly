\new Staff \with {
  instrumentName = \markup \center-column { "Clarinet" \concat { "in B" \flat } }
} \relative c'' {
  \tempo \markup { \bold "Allegro" \italic "con brio" }
  \mark \markup \box "A"
  c4^\markup { \dynamic sfz \italic "espr." } d4^\markup \circle "1" e4_\markup \underline "ten." f4^\markup \segno |
}
