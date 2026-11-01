# gd-engrave

Native music geometry and standalone SVG output for GDS 0015.
The crate borrows a `gd-score::Score` and explicit immutable font tables.
It performs no file access, runs no program and has no global font registry.

We load music and text tables with `load_fonts`, keep one `Engraver` per rendering thread and call `engrave_score` with a plain `Options` value.
`engrave_svg` supplies the one-call path.
`write_svg` selects complete systems, escapes accessible text and writes outlined glyphs without browser font dependencies.
Its output and buffer growth have explicit limits.

Spacing uses checked spring and rod inputs, conservative spanning-rod conversion and an exact piecewise-linear solver.
Line breaking uses a bounded dynamic program and preserves forced endpoints without suppressing earlier lines.
Staff and outside-mark placement uses piecewise-linear upper and lower skylines.
The display list retains staff, measure and semantic mark identities for export and geometric audits.

Bravura 1.482 and its SMuFL metadata are embedded unmodified from `third_party/bravura`.
The build checks their recorded digests.
Text faces come from the caller, with explicit regular, bold and italic fallback lists.

The corpus tool (`cargo run -p music-check`) checks 58 outlined SVGs, tolerant PNG pixels, actual role-ink contacts, primary beam quants and tie apexes across standard and compact staff sizes.
The reader and engraver have separate finite work limits; text outline growth, temporary shaping and retained worker capacities consult the caller's reservation before allocation.
Cross-system curves, lyric extenders and hairpins keep their continuation geometry, and `System::width` records the actual ink frame for per-system PNG and SVG export.

`Options::scratch` supplies paired reserve and release callbacks for temporary layout buffers.
Spacing sorts, line-breaking choices, candidate rods and live skyline replacements reserve before allocation and return the charge when their scope ends, including failures.
The host connects these callbacks to `Ledger::charge` and `Ledger::discharge`.
Retained workspace capacities are included for every engraving because a reused worker can receive a fresh ledger.
