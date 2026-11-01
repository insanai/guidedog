# gd-lilypond

A bounded reader for the Guidedog LilyPond profile in GDS 0015.
The library performs no file access and executes no Scheme or external program.

We use `load_score(source, name, &mut Options::default())` for the common case.
`parse_lilypond` and `interpret_syntax` expose the two stages for tools and tests.
`Options::include` supplies an explicit file policy; nested requests name the including file by `FileId`.
Built-in language includes select note names without reading files.
Failures retain the files read so that `error_report` can place the original span.

The reader covers all three profile phases, including exact tuplets, relative and fixed pitches, spelled transposition, staves and voices, lyrics, grace notes, structured `\markup`, modern `\volta`/`\fine`/`\section` repeats and chord names.
The command table distinguishes implemented notation, ignored metadata and refused engraver programming.
Source, nesting, includes, events and work have explicit limits.
Engraving and image export belong to the separate `gd-engrave` adapter.

The reader accepts four voices sounding at once on a staff, with local `\voiceOne` through `\voiceFour` policies and `\oneVoice` to restore automatic directions.
A fifth voice is refused at its source span.
