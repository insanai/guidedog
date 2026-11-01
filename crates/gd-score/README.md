# gd-score

Exact musical time, spelled pitches and a flat notation model for GDS 0015.
This crate has no dependencies and performs no file access or engraving.

We construct `Ratio` with `Ratio::new`, use its checked arithmetic for musical time, and call `check_score` before engraving a programmatically constructed `Score`.
Events, voices and staves have typed indices; ties and lyrics can identify individual chord heads.
Errors retain source spans and corrective hints for the application's diagnostic adapter.
