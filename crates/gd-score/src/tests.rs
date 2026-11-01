#![allow(clippy::unwrap_used)]

use crate::*;

#[test]
fn fractions_are_reduced_and_compared_by_value() {
    assert_eq!(Ratio::new(2, 4).unwrap(), Ratio::new(1, 2).unwrap());
    assert!(Ratio::new(1, 2).unwrap() > Ratio::new(1, 3).unwrap());
    assert!(Ratio::new(-1, 2).unwrap() < Ratio::new(-1, 3).unwrap());
    assert_eq!(Ratio::new(1, -2).unwrap(), Ratio::new(-1, 2).unwrap());
    assert_eq!(Ratio::new(0, i64::MIN).unwrap(), Ratio::ZERO);
    assert_eq!(Ratio::new(i64::MIN, i64::MIN).unwrap(), Ratio::ONE);
    assert!(Ratio::new(1, i64::MIN).is_err());
    assert!(Ratio::new(1, 0).is_err());
}

#[test]
fn arithmetic_reduces_before_checking_the_result() {
    let large = Ratio::new(i64::MAX, 2).unwrap();
    assert_eq!(large.checked_add(large).unwrap(), Ratio::new(i64::MAX, 1).unwrap());
    let inverse = Ratio::new(2, i64::MAX).unwrap();
    assert_eq!(large.checked_mul(inverse).unwrap(), Ratio::ONE);
    assert_eq!(large.checked_div(large).unwrap(), Ratio::ONE);
    assert_eq!(large.checked_sub(large).unwrap(), Ratio::ZERO);
    assert!(large.checked_mul(Ratio::new(3, 1).unwrap()).is_err());
    assert!(Ratio::new(i64::MIN, 1).unwrap().checked_neg().is_err());
    assert!(large.checked_div(Ratio::ZERO).is_err());
}

#[test]
fn durations_keep_dots_and_nested_tuplets_exact() {
    let d = Duration { dots: 2, scale: Ratio::new(2, 3).unwrap(), ..Duration::default() };
    assert_eq!(d.time().unwrap(), Ratio::new(7, 24).unwrap());
    let d = Duration { base: Base::Breve, ..Duration::default() };
    assert_eq!(d.time().unwrap(), Ratio::new(2, 1).unwrap());
    assert!(Duration { dots: 9, ..d }.time().is_err());
    assert!(Duration { scale: Ratio::ZERO, ..d }.time().is_err());
}

#[test]
fn transposition_preserves_spelling_and_modal_keys() {
    let c = Pitch::default();
    let e_flat = Pitch { step: Step::E, alter: Alter::Flat, octave: 0 };
    let g = Pitch { step: Step::G, ..c };
    let b_flat = g.transpose(c, e_flat).unwrap();
    assert_eq!(b_flat, Pitch { step: Step::B, alter: Alter::Flat, octave: 0 });
    let signature = Key { tonic: e_flat, mode: Mode::Major }.alterations().unwrap();
    assert_eq!(
        signature,
        [
            Alter::Natural,
            Alter::Natural,
            Alter::Flat,
            Alter::Natural,
            Alter::Natural,
            Alter::Flat,
            Alter::Flat
        ]
    );
    let a = Pitch { step: Step::A, ..c };
    assert_eq!(Key { tonic: a, mode: Mode::Minor }.alterations().unwrap(), [Alter::Natural; 7]);
}

#[test]
fn clef_positions_count_c_prime_as_middle_c() {
    let middle_c = Pitch { octave: 1, ..Pitch::default() };
    assert_eq!(middle_c.diatonic() - Clef::Treble.middle(), -6);
    assert_eq!(middle_c.diatonic() - Clef::Bass.middle(), 6);
    assert_eq!(middle_c.diatonic() - Clef::Alto.middle(), 0);
    assert_eq!(middle_c.diatonic() - Clef::Tenor.middle(), 2);
}

fn score() -> Score {
    Score {
        staves: vec![Staff::default()],
        voices: vec![Voice { events: vec![EventId(0)], ..Voice::default() }],
        events: vec![Event {
            voice: VoiceId(0),
            at: Moment::default(),
            kind: EventKind::Chord(Chord { notes: vec![Note::default()], ..Chord::default() }),
            span: Span::default(),
            marks: Vec::new(),
        }],
        ..Score::default()
    }
}

#[test]
fn score_validation_refuses_dangling_and_duplicate_ids() {
    let mut s = score();
    check_score(&s).unwrap();
    s.voices[0].events.push(EventId(0));
    assert!(check_score(&s).is_err());
    s.voices[0].events = vec![EventId(8)];
    assert!(check_score(&s).is_err());
    s.voices[0].events = vec![EventId(0)];
    s.spanners.push(Spanner {
        start: NoteRef::default(),
        end: NoteRef { note: 16, ..NoteRef::default() },
        kind: SpannerKind::Tie,
        direction: Direction::Auto,
        span: Span::default(),
    });
    assert!(check_score(&s).is_err());
}
