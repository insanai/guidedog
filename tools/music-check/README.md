# Native music qualification

We run `cargo run --locked -p music-check` to compare all 48 scores in `tests/integration/data/music` with their golden SVG and PNG output.
The tool uses explicit embedded Libertinus faces, so system fonts cannot change its results.
SVG must match byte for byte.
PNG dimensions must match; at most 0.2 percent of pixels may differ by more than eight channel levels, and mean channel error must stay below one percent.

Before comparison, we check finite ink bounds, actual ink contacts against the role contact table, exposed primary beam quants and tie apexes.
For a possible forbidden contact, we rasterize each mark in the intersection and compare its alpha mask at 288 pixels per inch.
This avoids treating white space inside a glyph or a slur's bounding rectangle as ink.
The contact tolerance is 0.05 staff space at the corpus's 18-point staff size.
Composite rest bars, signatures, brackets and dynamic strings have permitted internal contacts.

`cargo run -p music-check -- --update` explicitly replaces goldens after a reviewed engraving change.
`cargo run -p music-check -- --fuzz 12000` drives both native entry points with deterministic hostile bytes, Unicode, mutated notation, recursive bindings, explosive repeats and structured valid scores under small source, expansion and work limits.
It reports the number of successful parses and engravings so a run cannot silently test only rejected syntax.

For visual comparison, `cargo run -p music-check -- --reference /path/to/lilypond` invokes a separately installed LilyPond 2.24 oracle and writes SVGs, source wrappers, a log and four HTML comparison pages under `target/music-reference`.
This program is a development oracle only; documentation builds never invoke it.
The wrappers normalize attached lyrics and compressed measure rests to equivalent LilyPond contexts.
The recorded review is in `docs/gds/evidence/native-music-20261101.rst`.

CI runs the corpus on Linux and macOS and the fuzz target on both.
Release builds run the same corpus on all five native release platforms before packaging.
