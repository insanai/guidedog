# Native music qualification

We run `cargo run --locked -p music-check` to compare all 63 scores in `tests/integration/data/music` with their golden SVG and PNG output.
We first run `cargo xtask fetch-fonts --music` to download the single Noto Serif CJK SC regular face used by the corpus.
Both the download task and the corpus loader verify its pinned SHA-256.
CI and every release-platform job fetch it before qualification.
The tool owns its CJK bytes explicitly and uses fixed Libertinus and Devanagari faces, so host fonts cannot change its results.
Score 58 combines Latin, Devanagari and Chinese lyrics without missing-glyph warnings.
SVG must match byte for byte.
PNG dimensions must match; at most 0.2 percent of pixels may differ by more than eight channel levels, and mean channel error must stay below one percent.

At both 18-point and 14-point staff sizes, we reject engraving warnings and check finite ink bounds, actual ink contacts against the role contact table, exposed primary beam quants and tie apexes.
For a possible forbidden contact, we rasterize each mark in the intersection and compare its alpha mask at 288 pixels per inch.
This avoids treating white space inside a glyph or a slur's bounding rectangle as ink.
The contact tolerance scales with the staff size.
Composite rest bars, signatures, brackets and parts of a single dynamic string have permitted internal contacts; barlines against lyrics or names and independent overlapping dynamics do not.

`cargo run -p music-check -- --update` explicitly replaces goldens after a reviewed engraving change.
`cargo run -p music-check -- --fuzz 12000` drives both native entry points with deterministic hostile bytes, Unicode, mutated notation, recursive bindings, explosive repeats and structured valid scores under small source, expansion and work limits.
It reports the number of successful parses and engravings so a run cannot silently test only rejected syntax.

For visual comparison, `cargo run -p music-check -- --reference /path/to/lilypond` invokes a separately installed LilyPond 2.24 oracle and writes SVGs, source wrappers, a log and six HTML comparison pages under `target/music-reference`.
This program is a development oracle only; documentation builds never invoke it.
The wrappers normalize attached lyrics and compressed measure rests to equivalent LilyPond contexts.
The recorded review is in `docs/gds/evidence/native-music-20261101.rst`.

CI runs the corpus on Linux and macOS and the fuzz target on both.
Release builds run the same corpus on all five native release platforms before packaging.

Scores 59-62 qualify third and fourth voices, including dense accidentals and dots, flags, mixed-duration rests, named voices, lyrics and ties across a system break.
Score 63 is the manual's four-bar Beethoven Joy theme, transposed to C major; a reader regression checks every pitch and exact duration against the instrumental opening.
