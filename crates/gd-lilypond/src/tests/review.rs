use gd_score::{AnnotationKind, EventKind, Ratio, SpannerKind};

use crate::*;

fn load(source: &str) -> Loaded {
    load_score(source, "review.ly", &mut Options::default())
        .unwrap_or_else(|failed| panic!("{source}\n{}", failed.error))
}

#[test]
fn single_note_bindings_do_not_swallow_the_following_score() {
    let loaded = load(r"motif = c'4 \motif d'4");
    assert_eq!(loaded.score.events.len(), 2);
    let loaded = load(r"motif = { c'4 d' } \relative \motif");
    assert_eq!(loaded.score.events.len(), 2);
}

#[test]
fn string_bindings_work_in_marks_and_instrument_names() {
    let loaded = load(
        r#"label = "Flute" \new Staff \with { instrumentName = \label }
        { c'4 ^\label d'4 _\markup \italic \label }"#,
    );
    assert_eq!(loaded.score.staves[0].name, "Flute");
    assert!(
        loaded
            .score
            .events
            .iter()
            .flat_map(|e| &e.marks)
            .any(|m| { matches!(&m.kind, AnnotationKind::Text(s, _) if s == "Flute") })
    );
}

#[test]
fn grace_slurs_and_ties_can_attach_to_the_main_note() {
    let loaded = load(r"\clef treble \grace { c'16( d' } e'4) f'4");
    assert!(loaded.score.spanners.iter().any(|s| s.kind == SpannerKind::Slur));
    let loaded = load(r"\grace { c'16~ } c'4");
    assert_eq!(loaded.score.spanners[0].kind, SpannerKind::Tie);
    let loaded = load(r"\afterGrace c'4 { d'16 e' } f'4");
    let main: Vec<_> = loaded
        .score
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::Chord(_)) && e.at.grace == Ratio::ZERO)
        .collect();
    assert_eq!(main.len(), 2);
    assert_eq!(main[1].at.main, Ratio::new(1, 4).unwrap());
}

#[test]
fn tuplets_keep_the_written_number_instead_of_reducing_it() {
    let loaded = load(r"\tuplet 6/4 { c'16 d' e' f' g' a' }");
    assert!(loaded.score.spanners.iter().any(|s| s.kind == SpannerKind::Tuplet(6, 4)));
    let loaded = load(r"c'4*70000/70000");
    assert_eq!(loaded.score.measures[0].end, Ratio::new(1, 4).unwrap());
}

#[test]
fn named_octave_clefs_and_explicit_lyric_durations_are_read() {
    let loaded = load(
        r#"\new Staff << \new Voice = "v" { \clef treble_8 c'4 d'4 }
        \new Lyrics { Hel4 -- lo4 __ } >>"#,
    );
    assert_eq!(loaded.score.lyrics[0].syllables.len(), 2);
    assert!(loaded.score.lyrics[0].syllables[0].hyphen);
}

#[test]
fn key_changes_before_staff_contexts_do_not_make_a_phantom_staff() {
    let loaded = load(r"\key g \major \new Staff { c'1 }");
    assert_eq!(loaded.score.staves.len(), 1);
    let loaded = load(r"<< \new ChordNames \chordmode { c1:maj7 } \new Staff { c'1 } >>");
    assert_eq!(loaded.score.staves.len(), 1);
}
