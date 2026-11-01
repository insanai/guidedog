use gd_score::{EventKind, Ratio, SpannerKind};

use crate::*;

fn load(source: &str) -> Loaded {
    load_score(source, "example.ly", &mut Options::default())
        .unwrap_or_else(|failed| panic!("{source}\n{}", failed.error))
}

fn pitches(source: &str) -> Vec<(gd_score::Step, i8)> {
    load(source)
        .score
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Chord(c) => Some(c.notes.iter().map(|n| (n.pitch.step, n.pitch.octave))),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn relative_octaves_chords_and_nested_modes_follow_lilypond() {
    use gd_score::Step::*;
    assert_eq!(pitches("\\relative c' { c g c f, }"), [(C, 1), (G, 0), (C, 1), (F, 0)]);
    assert_eq!(pitches("\\relative { gis''' a b c }"), [(G, 3), (A, 3), (B, 3), (C, 4)]);
    assert_eq!(pitches("\\relative c' { <c e g> c }"), [(C, 1), (E, 1), (G, 1), (C, 1)]);
    assert_eq!(
        pitches("\\relative c' { c \\relative c'' { c d } c }"),
        [(C, 1), (C, 2), (D, 2), (C, 1)]
    );
    assert_eq!(
        pitches("\\relative c' { c \\fixed c' { c g, } c }"),
        [(C, 1), (C, 1), (G, 0), (C, 1)]
    );
}

#[test]
fn time_is_exact_through_dots_tuplets_and_inheritance() {
    let loaded = load("{ c8. d \\tuplet 3/2 { e16 f g } a4 } ");
    let events: Vec<_> =
        loaded.score.events.iter().filter(|e| e.kind.duration().is_some()).collect();
    assert_eq!(events[1].at.main, Ratio::new(3, 16).unwrap());
    assert_eq!(events[2].at.main, Ratio::new(3, 8).unwrap());
    assert_eq!(events[3].at.main, Ratio::new(5, 12).unwrap());
    assert_eq!(events[5].at.main, Ratio::new(1, 2).unwrap());
}

#[test]
fn ties_attach_individual_heads_and_require_the_same_pitch() {
    let loaded = load("{ <c'~ e'>4 <c' f'>4 } ");
    let ties: Vec<_> =
        loaded.score.spanners.iter().filter(|s| s.kind == SpannerKind::Tie).collect();
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].start.note, 0);
    assert!(load_score("{ c'4~ d'4 }", "bad.ly", &mut Options::default()).is_err());
}

#[test]
fn pickup_meter_changes_and_measure_rests_are_checked() {
    let loaded = load("{ \\time 3/4 \\partial 4 c'4 | d'2. | R2.*2 } ");
    assert_eq!(loaded.score.measures[0].number, 0);
    assert_eq!(loaded.score.measures[0].start, Ratio::new(-1, 4).unwrap());
    assert!(loaded.warnings.is_empty());
    assert!(load_score("{ \\time 3/4 R1 }", "bad.ly", &mut Options::default()).is_err());
    assert!(load_score("{ c4 \\time 3/4 d2. }", "bad.ly", &mut Options::default()).is_err());
    assert!(load("{ c4 d | e f }").warnings.iter().any(|e| e.kind == ErrorKind::BarCheck));
}

#[test]
fn staff_groups_voices_and_multiple_lyric_verses_are_independent() {
    let loaded = load(
        r#"\new PianoStaff <<
        \new Staff \with { instrumentName = "Piano" shortInstrumentName = "Pno." }
        << \new Voice = "melody" { c'4( d') e' f' } \\ { c2 g } >>
        \new Staff { \clef bass c1 }
    >> \lyricsto "melody" { Hel -- lo world } \lyricsto "melody" { One two three }"#,
    );
    assert_eq!(loaded.score.staves.len(), 2);
    assert_eq!(loaded.score.voices.len(), 3);
    assert_eq!(loaded.score.groups.len(), 1);
    assert_eq!(loaded.score.lyrics.len(), 2);
    assert_eq!(loaded.score.lyrics[0].syllables.len(), 3);
}

#[test]
fn repeats_and_grace_notes_do_not_change_main_durations() {
    let loaded = load(r"\relative c' { \grace { c16 d } e4 \repeat unfold 3 { c4 g } a4 }");
    assert_eq!(loaded.score.measures.last().unwrap().end, Ratio::new(2, 1).unwrap());
    let notes: Vec<_> =
        loaded.score.events.iter().filter(|e| matches!(e.kind, EventKind::Chord(_))).collect();
    assert!(notes[0].at.grace < Ratio::ZERO);
    assert_eq!(notes[2].at.grace, Ratio::ZERO);
    let loaded = load(r"{ \repeat volta 2 { c'1 } \alternative { { d'1 } { e'1 } } }");
    assert_eq!(
        loaded.score.spanners.iter().filter(|s| matches!(s.kind, SpannerKind::Volta(_))).count(),
        2
    );
}

#[test]
fn unknown_and_refused_commands_keep_positions_and_suggestions() {
    let failed =
        load_score("% music\nc4\\stacato", "original.ly", &mut Options::default()).unwrap_err();
    let report = error_report(&failed.error, &failed.files);
    assert_eq!((report.line, report.column), (2, 3));
    assert!(report.hints.join(" ").contains("staccato"));
    for descriptor in COMMANDS.iter().filter(|c| c.status == CommandStatus::Refused) {
        let source = format!("c4 \\{}", descriptor.name);
        assert_eq!(
            load_score(&source, "bad.ly", &mut Options::default()).unwrap_err().error.kind,
            ErrorKind::Unsupported,
            "{}",
            descriptor.name
        );
    }
    for source in ["{ c4 #(system \"bad\") }", "{ c4 $foo }", "\\layout { indent = 0 } c4"] {
        assert!(load_score(source, "bad.ly", &mut Options::default()).is_err());
    }
}

#[test]
fn every_profile_command_has_an_interpretation_example() {
    for descriptor in COMMANDS.iter().filter(|c| c.status != CommandStatus::Refused) {
        let name = descriptor.name;
        let source = match name {
            "relative" => r"\relative c' { c d }".into(),
            "fixed" => r"\fixed c' { c d }".into(),
            "transpose" => r"\transpose c ees { c d }".into(),
            "language" => r#"\language "english" { cs4 df }"#.into(),
            "version" => r#"\version "2.24.4" { c4 }"#.into(),
            "header" => r#"\header { title = "Example" tagline = ##f } { c4 }"#.into(),
            "layout" | "midi" | "score" => {
                if name == "score" {
                    r"\score { c4 d }".into()
                } else {
                    format!("\\{name} {{ }} {{ c4 }}")
                }
            }
            "clef" => r"\clef bass { c4 d }".into(),
            "key" => r"\key g \major { c4 d }".into(),
            "major" | "minor" | "ionian" | "dorian" | "phrygian" | "lydian" | "mixolydian"
            | "aeolian" | "locrian" => {
                format!("\\key c \\{name} {{ c1 }}")
            }
            "time" => r"\time 2,2,3 7/8 { c8 d e f g a b }".into(),
            "partial" => r"\partial 4 c4 | d1".into(),
            "bar" => r#"c1 \bar "|." "#.into(),
            "breve" => r"c\breve".into(),
            "tuplet" => r"\tuplet 3/2 { c8 d e }".into(),
            "times" => r"\times 2/3 { c8 d e }".into(),
            "markup" | "bold" | "italic" => {
                if name == "markup" {
                    r#"c4 ^\markup { "Text" }"#.into()
                } else {
                    format!("c4 ^\\markup \\{name} \"Text\"")
                }
            }
            "addlyrics" => r"{ c4 d } \addlyrics { One two }".into(),
            "lyricsto" => r#"\new Voice = "v" { c4 d } \lyricsto "v" { One two }"#.into(),
            "lyricmode" => r"verse = \lyricmode { One two } { c4 d } \addlyrics \verse".into(),
            "include" => r#"\include "english.ly" { c4 d }"#.into(),
            "new" => r"\new Staff { c4 d }".into(),
            "with" => r#"\new Staff \with { instrumentName = "Flute" } { c4 d }"#.into(),
            "set" => r#"\set Staff.instrumentName = "Flute" { c4 d }"#.into(),
            "grace" | "acciaccatura" | "appoggiatura" => format!("\\{name} {{ c16 d }} e4"),
            "afterGrace" => r"\afterGrace c4 { d16 e } f4".into(),
            "repeat" => r"\repeat unfold 2 { c4 d }".into(),
            "alternative" => r"\repeat volta 2 { c1 } \alternative { { d1 } { e1 } }".into(),
            "volta" => {
                r"\repeat volta 3 { c1 } \alternative { \volta 1,2 { d1 } \volta 3 { e1 } }".into()
            }
            "fine" | "section" => format!("c1 \\{name}"),
            "compressEmptyMeasures" | "compressFullBarRests" => format!("\\{name} R1*2"),
            "numericTimeSignature" => r"\numericTimeSignature \time 4/4 c1".into(),
            "tempo" => r#"\tempo "Allegro" 4 = 120 c1"#.into(),
            "mark" | "default" => r"\mark \default c1".into(),
            "chordmode" => r"\new ChordNames \chordmode { c1:maj7 d:m7 g:7 }".into(),
            "chords" => r"\chords { c1:maj7 d:m7 g:7 }".into(),
            "<" | ">" => format!("c4 \\{name} d4 \\!"),
            "!" => r"c4 \< d4 \!".into(),
            "(" | ")" => r"c4\( d4\)".into(),
            "\\" => r"<< { c2 d } \\ { e2 f } >>".into(),
            _ => format!("c4 \\{name} d4"),
        };
        load(&source);
    }
}

#[test]
fn adversarial_corpus_is_bounded_and_never_panics() {
    let alphabet = b"{}<>[]()\\#$_ abcdefgrs012348.,'~|%\"*/=-\n";
    let mut seed = 17u64;
    for length in 0..1024 {
        let mut text = String::new();
        for _ in 0..length % 192 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push(char::from(alphabet[(seed >> 32) as usize % alphabet.len()]));
        }
        let mut options = Options {
            limits: Limits {
                work: 512,
                depth: 8,
                events: 64,
                source_bytes: 256,
                files: 2,
                memory_bytes: 65536,
            },
            ..Options::default()
        };
        let _ = load_score(&text, "fuzz.ly", &mut options);
    }
    let recursive = "a = { \\a } \\a";
    assert_eq!(
        load_score(recursive, "recursive.ly", &mut Options::default()).unwrap_err().error.kind,
        ErrorKind::ReadLimit
    );
    let mut options =
        Options { limits: Limits { events: 16, ..Limits::default() }, ..Options::default() };
    assert!(load_score(r"\repeat unfold 65535 { c128 }", "repeat.ly", &mut options).is_err());
}
