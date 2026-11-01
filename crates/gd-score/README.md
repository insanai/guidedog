# gd-score

Exact musical time, spelled pitches and a flat notation model for GDS 0015.
This crate has no dependencies and performs no file access or engraving.

We construct `Ratio` with `Ratio::new`, use its checked arithmetic for musical time, and call `check_score` before engraving a programmatically constructed `Score`.
Events, voices and staves have typed indices; ties and lyrics can identify individual chord heads.
Errors retain source spans and corrective hints for the application's diagnostic adapter.

At most four voices of a staff sound at once; a staff keeps at most sixty-four voice records.
Each event can carry a local `VoiceStyle` and an explicit `column_shift` (`\shiftOn` levels); `None` inherits its voice's policy and `Single` restores automatic directions.
A chord's optional direction overrides that policy; `Some(Direction::Auto)` requests neutral stems explicitly, while `None` inherits the policy.
Text marks carry structured `Markup` (`MarkupRole`, `ColumnAlign`, `Enclosure`, `StyleChange`) for layout and engraving.
