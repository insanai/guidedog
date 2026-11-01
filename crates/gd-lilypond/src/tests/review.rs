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

#[test]
fn repeated_text_and_copied_state_consume_memory_before_expansion() {
    let text = "large ".repeat(512);
    let source = format!("\\repeat unfold 200 {{ c'4 ^\"{text}\" }}");
    let mut options = Options {
        limits: Limits { memory_bytes: 64 * 1024, ..Limits::default() },
        ..Options::default()
    };
    let failed = load_score(&source, "memory.ly", &mut options).unwrap_err();
    assert_eq!(failed.error.kind, ErrorKind::ReadLimit);
    assert!(failed.error.message.contains("memory"));
    let refuse = |_: usize| false;
    let failed = load_score(
        "{ c4 }",
        "refused.ly",
        &mut Options { reserve: Some(&refuse), ..Options::default() },
    )
    .unwrap_err();
    assert_eq!(failed.error.kind, ErrorKind::ReadLimit);
}

#[test]
fn independent_staves_do_not_inherit_polyphonic_stem_directions() {
    let loaded = load_score(
        r"\new PianoStaff <<
        \new Staff { c'1 } \new Staff { c1 }
    >>",
        "staves.ly",
        &mut Options::default(),
    )
    .unwrap();
    for event in &loaded.score.events {
        if let gd_score::EventKind::Chord(chord) = &event.kind {
            assert_eq!(chord.direction, None);
        }
    }
}

#[test]
fn articulation_shorthand_keeps_its_angle_outside_chord_delimiters() {
    let loaded =
        load_score(r"{ c'4-. d'-> e'-- f'-^ | }", "marks.ly", &mut Options::default()).unwrap();
    assert_eq!(loaded.score.events.iter().map(|e| e.marks.len()).sum::<usize>(), 4);
    assert!(load_score("{ <c' e'4 }", "chord.ly", &mut Options::default()).is_err());
}

#[test]
fn measure_expansion_consumes_the_same_memory_allowance_as_notes() {
    let mut options = Options {
        limits: Limits { memory_bytes: 4096, ..Limits::default() },
        ..Options::default()
    };
    let failed = load_score("{ R1*100 }", "rests.ly", &mut options).unwrap_err();
    assert_eq!(failed.error.kind, ErrorKind::ReadLimit);
    assert!(failed.error.message.contains("memory"));
}

#[test]
fn beat_group_lists_and_grace_state_copies_respect_reader_limits() {
    let source = format!("\\time {} 4/4 c'1", "1,".repeat(129));
    let failed = load_score(&source, "meter.ly", &mut Options::default()).unwrap_err();
    assert!(failed.error.message.contains("128 beat groups"));
    let mut options = Options {
        limits: Limits { memory_bytes: 2048, ..Limits::default() },
        ..Default::default()
    };
    let failed = load_score("\\grace { c'16 } d'4", "grace.ly", &mut options).unwrap_err();
    assert_eq!(failed.error.kind, ErrorKind::ReadLimit);
}

#[test]
fn directed_dynamics_close_hairpins_and_preserve_their_side() {
    let loaded = load(r"{ c'2^\< d'2^\f }");
    assert_eq!(loaded.score.spanners.len(), 1);
    assert_eq!(loaded.score.spanners[0].kind, SpannerKind::Crescendo);
    assert_eq!(loaded.score.spanners[0].direction, gd_score::Direction::Up);
    let loaded = load(r"{ c'2\< d'2_\p }");
    assert_eq!(loaded.score.spanners.len(), 1);
    assert_eq!(loaded.score.spanners[0].direction, gd_score::Direction::Down);
}

#[test]
fn polyphonic_volta_and_tuplet_attach_valid_same_voice_spanners() {
    let loaded = load(
        r"\repeat volta 3 { << { c'1 } \\ { e1 } >> }
        \alternative { { << { d'1 } \\ { f1 } >> } { << { e'1 } \\ { g1 } >> } } a'1",
    );
    let voltas: Vec<_> = loaded
        .score
        .spanners
        .iter()
        .filter_map(|s| match &s.kind {
            SpannerKind::Volta(label) => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(voltas, ["1.-2.", "3."]);
    let inline = load(r"\repeat volta 2 { c'1 \alternative { { d'1 } { e'1 } } }");
    assert_eq!(
        inline.score.spanners.iter().filter(|s| matches!(s.kind, SpannerKind::Volta(_))).count(),
        2
    );
    let tuplets = load(r"\tuplet 3/2 { << { c'8 d'8 e'8 } \\ { e8 f8 g8 } >> }");
    assert_eq!(
        tuplets
            .score
            .spanners
            .iter()
            .filter(|s| matches!(s.kind, SpannerKind::Tuplet(3, 2)))
            .count(),
        2
    );
}

#[test]
fn musical_semantics_transpose_chords_keys_and_relative_q() {
    let loaded = load(
        r"\transpose c d <<
            \chords { c1:maj7/g | r1 }
            \new Staff { \key g \major \relative c { c4 <e g c> g, q } | c'1 }
        >>",
    );
    let chords: Vec<_> = loaded
        .score
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::ChordName(name, _) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(chords, ["Dmaj7/A"]);
    let repeated = loaded
        .score
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Chord(c) => Some(&c.notes),
            _ => None,
        })
        .nth(3)
        .unwrap();
    assert_eq!(repeated.len(), 3);
    let _ = load(r"\transpose c'''' c { \key g \major c''''1 }");
    // The second branch continues from the first branch's last note (d''), and the
    // music after the block continues from the second branch (f').
    let rel = load(r"\relative c' { << { c'2 d } \\ { e,2 f } >> g2 }");
    let octaves: Vec<_> = rel
        .score
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Chord(c) => Some(c.notes[0].pitch.octave),
            _ => None,
        })
        .collect();
    assert_eq!(octaves, [2, 2, 1, 1, 1]);
}

#[test]
fn pickups_marks_and_tempo_keep_their_moments() {
    // Music after a pickup in the same staff is not moved back by the pickup.
    let pickup = load(r"{ \partial 4 g'4 << { c''1 } \\ { e'1 } >> }");
    let starts: Vec<_> = pickup.score.events.iter().map(|e| e.at.main).collect();
    assert_eq!(starts, [Ratio::new(-1, 4).unwrap(), Ratio::ZERO, Ratio::ZERO]);
    // A mark at the end of a branch moves to the next note, not to a new bar line.
    let mark = load(r#"{ << { c'2 \mark "A" } \\ { e2 } >> d'2 }"#);
    assert!(!mark.score.events.iter().any(|e| matches!(e.kind, EventKind::Bar(_))));
    assert!(mark.score.events.iter().any(|e| {
        e.marks.iter().any(|m| matches!(&m.kind, AnnotationKind::Rehearsal(s) if s == "A"))
    }));
    let tempo = load(r"{ \tempo 4. = 80 c'2. }");
    assert!(tempo.score.events[0].marks.iter().any(
        |m| matches!(&m.kind, AnnotationKind::Tempo(s) if s.contains("4.") || s.contains("1/4."))
    ));
    let ending = load(r"{ \repeat volta 2 { c'1 } \alternative { { d'1 } { e'1 } } f'1 }");
    assert!(
        !ending
            .score
            .events
            .iter()
            .any(|e| matches!(e.kind, EventKind::Bar(gd_score::BarStyle::Final)))
    );
    let piano = load(
        r#"\new PianoStaff \with { instrumentName = "Piano" } <<
            \new Staff \with { instrumentName = "RH" } { c'1 }
            \new Staff { c1 }
        >>"#,
    );
    assert_eq!(
        (piano.score.groups[0].name.as_str(), piano.score.staves[0].name.as_str()),
        ("Piano", "RH")
    );
    // A variable keeps the default duration and language of its definition.
    let motif = load(r"motif = { c' d' } { e'8 \motif f' }");
    let lengths: Vec<_> =
        motif.score.events.iter().filter_map(|e| e.kind.duration()).map(|d| d.base).collect();
    use gd_score::Base::{Eighth, Quarter};
    assert_eq!(lengths, [Eighth, Quarter, Quarter, Eighth]);
    let german = load(r#"\language "deutsch" m = { h'4 } \language "nederlands" { \m }"#);
    assert!(matches!(&german.score.events[0].kind,
        EventKind::Chord(c) if c.notes[0].pitch.alter == gd_score::Alter::Natural));
    let refused = load_score(
        r"\new Staff \with { instrumentName = #Flute } { c'1 }",
        "bad.ly",
        &mut Options::default(),
    );
    assert!(refused.is_err());
}

#[test]
fn languages_and_multistaff_pickups_work() {
    let de = load(r#"\language "deutsch" { b'4 heses' h' his' asas' es' }"#);
    let alters: Vec<_> = de
        .score
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Chord(c) => Some(c.notes[0].pitch.alter),
            _ => None,
        })
        .collect();
    assert_eq!(
        alters,
        [
            gd_score::Alter::Flat,
            gd_score::Alter::DoubleFlat,
            gd_score::Alter::Natural,
            gd_score::Alter::Sharp,
            gd_score::Alter::DoubleFlat,
            gd_score::Alter::Flat,
        ]
    );
    for wrong in ["bis'4", "bes'4", "hes'4", "aes'4", "ees'4"] {
        let source = format!("\\language \"deutsch\" {{ {wrong} }}");
        assert!(load_score(&source, "bad.ly", &mut Options::default()).is_err(), "{wrong}");
    }
    let en = load(r#"\language "english" { cx'4 }"#);
    assert!(en.score.events.iter().any(|e| matches!(&e.kind,
        EventKind::Chord(c) if c.notes[0].pitch.alter == gd_score::Alter::DoubleSharp)));
    let pickup = load(
        r"\new PianoStaff <<
            \new Staff { \partial 4 g'4 | c''1 }
            \new Staff { \clef bass \partial 4 g4 | c1 }
        >>",
    );
    assert_eq!(pickup.score.measures[0].start, Ratio::new(-1, 4).unwrap());
    let incomplete = load(r"{ c'4 d'4 e'4 | }");
    assert!(incomplete.warnings.iter().any(|w| w.kind == ErrorKind::BarCheck));
}

#[test]
fn with_blocks_voice_reuse_greedy_angles_and_lyrics_ergonomics_work() {
    let group = load(
        r#"\new PianoStaff \with { instrumentName = "Piano" } <<
            \new Staff = "upper" <<
                \new Voice = "soprano" { \voiceOne c''2 d'' }
                \new Voice = "alto" { \voiceTwo e'2 f' }
            >>
            \new Staff { \clef bass << { g2 a } \\ { c2 d } >> << { b2 c' } \\ { e2 f } >> }
        >>"#,
    );
    assert_eq!(group.score.groups[0].name, "Piano");
    assert_eq!(
        load_score(
            r"\new Voice \with { \override NoteHead.color = #red } { c'4 }",
            "bad.ly",
            &mut Options::default(),
        )
        .unwrap_err()
        .error
        .kind,
        ErrorKind::Unsupported
    );
    let _ = load(r"<< \new Staff <c'-> e'-3>>>");
    let _ = load(r"<< c'4->>>");
    let grace = load(r"{ c'2~ \acciaccatura { d'16 e'16 } c'2 }");
    assert!(grace.score.spanners.iter().any(|s| s.kind == SpannerKind::Tie));
    assert!(grace.score.spanners.iter().any(|s| s.kind == SpannerKind::Slur));
    let slashed: Vec<_> = grace
        .score
        .events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::Chord(c) if e.at.grace < Ratio::ZERO => Some(c.grace_slash),
            _ => None,
        })
        .collect();
    assert_eq!(slashed, [true, false]);
    let lyrics = load(
        r"{ c'4~ c'4~ c'4 d'4 | e'2 f'2 }
        \addlyrics { A __ Don't | stop, world. }",
    );
    assert_eq!(lyrics.score.lyrics[0].syllables.len(), 4);
    assert_eq!(lyrics.score.lyrics[0].syllables[1].text, "Don't");
    assert_eq!(lyrics.score.lyrics[0].syllables[2].text, "stop,");
    assert_eq!(lyrics.score.lyrics[0].syllables[3].text, "world.");
    assert_eq!(
        lyrics.score.lyrics[0].syllables[0].extender,
        Some(gd_score::Anchor::Note(gd_score::NoteRef { event: gd_score::EventId(2), note: 0 }))
    );
}

#[test]
fn lilypond_224_markup_compositions_contexts_and_refusals_work() {
    let loaded = load(
        r#"dolce = \markup \italic { dolce espressivo }
        \header { title = \markup \center-column { "Suite" \small "Op. 1" } }
        \new Staff \with {
            instrumentName = \markup \concat { "Clarinet in B" \flat }
        } {
            \tempo \markup { \italic "Andante" } 4. = 80
            \mark \markup \box "A"
            c'2\markup \bold \italic "marcato"
            d'2^\dolce
            e'2_\markup \center-column { \dynamic "fp" \musicglyph "scripts.segno" }
            f'2^\markup { "6" \super "+" }
            \mark "Fine"
        }"#,
    );
    assert_eq!(loaded.score.staves[0].name, "Clarinet in Bb");
    assert!(loaded.score.staves[0].name_markup.is_some());
    let has_tempo = loaded
        .score
        .events
        .iter()
        .flat_map(|e| &e.marks)
        .any(|m| matches!(&m.kind, AnnotationKind::Markup(_, gd_score::MarkupRole::Tempo)));
    assert!(has_tempo);
    let has_fine = loaded
        .score
        .events
        .iter()
        .flat_map(|e| &e.marks)
        .any(|m| matches!(&m.kind, AnnotationKind::Rehearsal(s) if s == "Fine"));
    assert!(has_fine);
    for refused in [
        r#"c'4^\markup \with-color #red "x""#,
        r#"c'4^\markup \score { c'4 }"#,
        r#"\markuplist { "a" } c'4"#,
    ] {
        assert_eq!(
            load_score(refused, "bad.ly", &mut Options::default()).unwrap_err().error.kind,
            ErrorKind::Unsupported,
            "{refused}"
        );
    }
}

#[test]
fn markup_refusals_point_at_their_command_and_styles_nest() {
    // Every required refusal, directly and inside a variable, names the command's text.
    let refused = [
        "\\markuplist",
        "\\score",
        "\\override",
        "\\fromproperty",
        "\\on-the-fly",
        "\\postscript",
        "\\epsfile",
        "\\path",
        "\\with-url",
        "\\with-color",
        "\\fill-line",
        "\\wordwrap",
        "\\justify",
        "\\semisharp",
        "\\semiflat",
        "#(",
    ];
    for command in refused {
        let tail = if command == "#(" { r#"markup "x")"# } else { r#"#red "x""# };
        for source in [
            format!("c'4^\\markup {command}{tail}"),
            format!("x = \\markup {command}{tail}\n{{ c'4^\\x }}"),
        ] {
            let failed = load_score(&source, "bad.ly", &mut Options::default()).unwrap_err();
            assert_eq!(failed.error.kind, ErrorKind::Unsupported, "{source}");
            let span = failed.error.span;
            let named = &source[span.start..span.end];
            assert!(command.starts_with(named) || named.starts_with(command), "{source}: {named}");
        }
    }
    let standalone = load_score(r#"\markup "Title" { c'4 }"#, "bad.ly", &mut Options::default());
    assert_eq!(standalone.unwrap_err().error.kind, ErrorKind::Unsupported);
    let styles = load(r#"{ c'4^\markup \bold \italic "a" d'^\markup \bold \upright "b" }"#);
    let found: Vec<_> = styles
        .score
        .events
        .iter()
        .flat_map(|e| &e.marks)
        .filter_map(|m| match &m.kind {
            AnnotationKind::Markup(markup, _) => Some(markup.clone()),
            _ => None,
        })
        .collect();
    use gd_score::{Markup, StyleChange, TextStyle};
    let applied: Vec<_> = found
        .iter()
        .map(|m| match m {
            Markup::Style(outer, inner) => match inner.as_ref() {
                Markup::Style(change, _) => change.apply(outer.apply(TextStyle::Regular)),
                _ => outer.apply(TextStyle::Regular),
            },
            _ => TextStyle::Regular,
        })
        .collect();
    assert_eq!(applied, [TextStyle::BoldItalic, TextStyle::Bold]);
    assert_eq!(StyleChange::Medium.apply(TextStyle::BoldItalic), TextStyle::Italic);
}

#[test]
fn included_file_unclosed_braces_and_dangling_ties_keep_include_spans() {
    let mut include = |request: IncludeRequest<'_>| match request.name {
        "unclosed.ily" => Ok(IncludedFile { name: "unclosed.ily".into(), text: "{ c'4".into() }),
        "tie.ily" => Ok(IncludedFile { name: "tie.ily".into(), text: "c'4~".into() }),
        _ => Err(IncludeRefusal { message: "missing".into(), hint: "fix".into() }),
    };
    let failed = load_score(
        "\\include \"unclosed.ily\"\nd'4",
        "main.ly",
        &mut Options { include: Some(&mut include), ..Options::default() },
    )
    .unwrap_err();
    assert_eq!(error_report(&failed.error, &failed.files).path, "unclosed.ily");
    let failed = load_score(
        "\\include \"tie.ily\"",
        "main.ly",
        &mut Options { include: Some(&mut include), ..Options::default() },
    )
    .unwrap_err();
    assert_eq!(error_report(&failed.error, &failed.files).path, "tie.ily");
}

#[test]
fn four_implicit_voices_keep_time_styles_and_reject_a_fifth() {
    let loaded = read_voices(r"<< { c''1 } \\ { c'1 } \\ { g'1 } \\ { e'1 } >>").unwrap();
    assert_eq!(loaded.score.voices.len(), 4);
    let styles: Vec<_> = loaded.score.events.iter().map(|event| event.voice_style).collect();
    assert_eq!(
        styles,
        [
            Some(gd_score::VoiceStyle::One),
            Some(gd_score::VoiceStyle::Two),
            Some(gd_score::VoiceStyle::Three),
            Some(gd_score::VoiceStyle::Four),
        ]
    );
    assert!(loaded.score.events.iter().all(|event| event.at.main == gd_score::Ratio::ZERO));
    let failed = read_voices(r"<< { c1 } \\ { d1 } \\ { e1 } \\ { f1 } \\ { g1 } >>").unwrap_err();
    assert!(failed.error.message.contains("four voices sounding at once"));
    // Finished voices do not count: five differently named voices, at most three at once.
    let sequential = read_voices(
        r#"\new Staff {
            << \new Voice = "a" { \voiceOne c''1 } \new Voice = "b" { \voiceTwo c'1 } >>
            << \new Voice = "c" { \voiceOne d''1 } \new Voice = "d" { \voiceTwo d'1 }
               \new Voice = "e" { \voiceThree f'1 } >>
        }"#,
    )
    .unwrap();
    assert_eq!(sequential.score.voices.len(), 5);
    assert!(gd_score::check_score(&sequential.score).is_ok());
}

#[test]
fn explicit_inner_voice_styles_are_local_and_one_voice_restores_neutral_policy() {
    let loaded = read_voices(r"{ \voiceThree c'4 r4 \voiceFour d'4 \oneVoice e'4 }").unwrap();
    let styles: Vec<_> = loaded.score.events.iter().map(|event| event.voice_style).collect();
    use gd_score::VoiceStyle::{Four, Single, Three};
    assert_eq!(styles, [Some(Three), Some(Three), Some(Four), Some(Single)]);
    assert_eq!(Three.direction(), gd_score::Direction::Up);
    assert_eq!(Four.direction(), gd_score::Direction::Down);
}

fn read_voices(source: &str) -> Result<Loaded, Failed> {
    load_score(source, "voices.ly", &mut Options::default())
}

#[test]
fn beethoven_teaching_excerpt_keeps_the_instrumental_pitches_and_beat_counts() {
    let loaded = load(include_str!("../../../../tests/integration/data/music/63-ode-to-joy.ly"));
    assert!(loaded.warnings.is_empty());
    assert_eq!(loaded.score.measures.len(), 4);
    let notes: Vec<_> = loaded
        .score
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Chord(chord) => {
                Some((chord.notes[0].pitch, chord.duration.time().unwrap()))
            }
            _ => None,
        })
        .collect();
    use gd_score::Step::{C, D, E, F, G};
    let pitches = [E, F, G, G, F, E, D, C, D, E, E, D, D];
    let times = [
        (1, 2),
        (1, 4),
        (1, 4),
        (1, 4),
        (1, 4),
        (1, 4),
        (1, 4),
        (1, 2),
        (1, 4),
        (1, 4),
        (3, 8),
        (1, 8),
        (1, 2),
    ];
    assert_eq!(notes.len(), pitches.len());
    for ((pitch, time), (step, (num, den))) in notes.iter().zip(pitches.into_iter().zip(times)) {
        assert_eq!(pitch.step, step);
        assert_eq!(pitch.octave, 1);
        assert_eq!(pitch.alter, gd_score::Alter::Natural);
        assert_eq!(*time, Ratio::new(num, den).unwrap());
    }
    assert_eq!(loaded.score.measures.last().unwrap().end, Ratio::new(4, 1).unwrap());
}

#[test]
fn voice_styles_leave_dynamics_and_later_stems_alone() {
    // LilyPond moves no dynamics with a voice: DynamicLineSpanner is not polyphonic.
    let loaded = read_voices(r"<< { c''4\p d'' } \\ { c'4 d' } >>").unwrap();
    let dynamic = loaded.score.events[0]
        .marks
        .iter()
        .find(|m| matches!(m.kind, AnnotationKind::Dynamic(_)))
        .unwrap();
    assert_eq!(dynamic.direction, gd_score::Direction::Auto);
    // The block's first voice styles only its own notes; later notes have neutral stems.
    let after = read_voices(r"{ << { c''4 } \\ { e'4 } >> c'''4 d'''4 }").unwrap();
    assert_eq!(after.score.voices[0].direction, gd_score::Direction::Auto);
    let last = after.score.events.last().unwrap();
    assert!(last.voice_style.is_none_or(|s| s == gd_score::VoiceStyle::Single), "{last:?}");
}

#[test]
fn shift_commands_set_explicit_column_levels_until_a_voice_policy() {
    let loaded = read_voices(
        r"{ \voiceThree c'4 \shiftOff d' \shiftOnn e' \shiftOnnn f' \voiceFour g' \shiftOn a' }",
    )
    .unwrap();
    let shifts: Vec<_> = loaded.score.events.iter().map(|event| event.column_shift).collect();
    assert_eq!(shifts, [None, Some(0), Some(2), Some(3), None, Some(1)]);
}
