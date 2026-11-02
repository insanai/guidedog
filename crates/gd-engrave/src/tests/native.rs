use crate::*;

const REGULAR: &[u8] =
    include_bytes!("../../../../docs/shared/_static/data/fonts/LibertinusSerif-Regular.otf");

fn draw(source: &str, width: f64) -> Engraving {
    let loaded =
        gd_lilypond::load_score(source, "native.ly", &mut gd_lilypond::Options::default())
            .unwrap();
    let fonts = load_fonts(&FontOptions {
        regular: vec![FaceData { data: REGULAR, ..FaceData::default() }],
        ..FontOptions::default()
    })
    .unwrap();
    Engraver::default()
        .engrave_score(
            &loaded.score,
            &fonts,
            &mut Options { line_width: width, ..Options::default() },
        )
        .unwrap()
}

#[test]
fn complete_phrase_has_signatures_beams_accidentals_and_slurs() {
    let engraving = draw(
        r"\relative c'' { \key g \major \time 3/4
        d4 g,8( a b c) | d4 g, g | e' c8( d e fis) | g4 g, g | }",
        350.0,
    );
    let roles: Vec<_> =
        engraving.systems.iter().flat_map(|s| &s.marks).map(|m| m.mark.role()).collect();
    assert!(roles.contains(&Role::Signature));
    assert!(roles.contains(&Role::Beam));
    assert!(roles.contains(&Role::Slur));
    assert_eq!(engraving.summary.measures, (1, 4));
}

#[test]
fn every_ink_bound_fits_its_system_with_lyrics_and_ledgers() {
    let engraving = draw(
        r#"\new PianoStaff <<
        \new Staff { c'''4 d''' e''' f''' } \addlyrics { Hel -- lo mu -- sic }
        \new Staff { \clef bass c,,1 }
    >>"#,
        280.0,
    );
    for system in &engraving.systems {
        for mark in &system.marks {
            assert!(mark.bounds.top >= -0.001, "{:?}", mark.mark);
            assert!(mark.bounds.bottom <= system.height + 0.001, "{:?}", mark.mark);
        }
    }
}

#[test]
fn a_reused_workspace_is_byte_deterministic() {
    let fonts = load_fonts(&FontOptions {
        regular: vec![FaceData { data: REGULAR, ..FaceData::default() }],
        ..FontOptions::default()
    })
    .unwrap();
    let a = gd_lilypond::load_score(
        r"{ c'4( d' e' f') | g'1 }",
        "a.ly",
        &mut gd_lilypond::Options::default(),
    )
    .unwrap();
    let b = gd_lilypond::load_score(
        r"{ c''8 d'' e'' f'' g'' a'' b'' c''' }",
        "b.ly",
        &mut gd_lilypond::Options::default(),
    )
    .unwrap();
    let mut workspace = Engraver::default();
    let first = workspace.engrave_score(&a.score, &fonts, &mut Options::default()).unwrap();
    let _ = workspace.engrave_score(&b.score, &fonts, &mut Options::default()).unwrap();
    let second = workspace.engrave_score(&a.score, &fonts, &mut Options::default()).unwrap();
    let mut first_svg = String::new();
    let mut second_svg = String::new();
    write_svg(&mut first_svg, &first, &SvgOptions::default(), None).unwrap();
    write_svg(&mut second_svg, &second, &SvgOptions::default(), None).unwrap();
    assert_eq!(first_svg, second_svg);
}

#[test]
fn groups_repeats_bars_and_measure_rests_have_distinct_geometry() {
    let engraving = draw(
        r"\new StaffGroup <<
        \new Staff { \repeat volta 2 { c'1 | } R1*3 }
        \new Staff { \clef bass c1 | R1*3 }
    >>",
        350.0,
    );
    let marks: Vec<_> = engraving.systems.iter().flat_map(|s| &s.marks).collect();
    assert!(marks.iter().any(|m| m.mark.role() == Role::Instrument));
    let repeats = marks
        .iter()
        .filter(|m| {
            matches!(m.mark, Mark::Glyph { glyph, .. }
        if engraving.glyphs.get(&glyph).is_some_and(|g| g.name == "repeatDot"))
        })
        .count();
    assert_eq!(repeats, 4);
    assert!(marks.iter().any(|m| matches!(m.mark, Mark::Line {
        role: Role::Rest, thickness, .. } if thickness > 3.0)));
    for system in &engraving.systems {
        for mark in &system.marks {
            assert!(
                mark.bounds.top >= -0.001 && mark.bounds.bottom <= system.height + 0.001,
                "{:?}",
                mark.mark
            );
        }
    }
}

#[test]
fn a_group_name_stands_left_of_its_staff_names_and_between_its_staves() {
    let engraving = draw(
        r#"\new PianoStaff \with { instrumentName = "Piano" } <<
        \new Staff \with { instrumentName = "RH" } { c''1 }
        \new Staff \with { instrumentName = "LH" } { \clef bass c1 }
    >>"#,
        350.0,
    );
    let names: Vec<_> = engraving.systems[0]
        .marks
        .iter()
        .filter(|m| matches!(m.mark, Mark::Text { role: Role::Instrument, .. }))
        .map(|m| m.bounds)
        .collect();
    assert_eq!(names.len(), 3, "{names:?}");
    let group = names.iter().min_by(|a, b| a.left.total_cmp(&b.left)).unwrap();
    for staff in names.iter().filter(|b| b.left > group.left) {
        assert!(staff.left > group.right, "{group:?} {staff:?}");
    }
    let (top, bottom) = names
        .iter()
        .filter(|b| b.left > group.left)
        .fold((f64::MAX, f64::MIN), |(t, b), n| (t.min(n.top), b.max(n.bottom)));
    assert!(group.top > top && group.bottom < bottom, "{group:?}");
}

#[test]
fn lazy_text_faces_are_selected_only_during_shaping_and_record_their_source() {
    struct Source(std::sync::atomic::AtomicUsize);
    impl FaceSource for Source {
        fn data(&self) -> Result<&[u8], gd_score::Error> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(REGULAR)
        }
        fn id(&self) -> Option<&str> {
            Some("selected-font.otf")
        }
    }
    let source = Source(std::sync::atomic::AtomicUsize::new(0));
    let fonts = load_fonts(&FontOptions {
        regular: vec![FaceData { source: Some(&source), ..Default::default() }],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(source.0.load(std::sync::atomic::Ordering::Relaxed), 0);
    for _ in 0..2 {
        let run = crate::text::shape(
            &fonts,
            "Music",
            12.0,
            gd_score::TextStyle::Regular,
            &mut Vec::new(),
            &mut 100_000,
            &mut None,
        )
        .unwrap();
        assert_eq!(run.sources, ["selected-font.otf"]);
    }
    assert_eq!(source.0.load(std::sync::atomic::Ordering::Relaxed), 2);
}

#[test]
fn simultaneous_voices_share_unisons_and_separate_seconds() {
    let shared = draw(r"<< { c'4 d' e' f' } \\ { c'4 d' e' f' } >>", 280.0);
    assert_eq!(
        shared
            .systems
            .iter()
            .flat_map(|s| &s.marks)
            .filter(|mark| mark.mark.role() == Role::Notehead)
            .count(),
        4
    );
    let seconds = draw(r"<< { c'4 } \\ { d'4 } >>", 280.0);
    let xs: Vec<_> = seconds
        .systems
        .iter()
        .flat_map(|s| &s.marks)
        .filter_map(|mark| match mark.mark {
            Mark::Glyph { x, role: Role::Notehead, .. } => Some(x),
            _ => None,
        })
        .collect();
    assert_eq!(xs.len(), 2);
    assert!((xs[1] - xs[0]).abs() > 4.0);
}

#[test]
fn chord_accidentals_have_disjoint_ink_columns() {
    let drawing = draw(r"{ <cis' dis' eis' fis' gis' ais' bis'>1 }", 350.0);
    let accidentals: Vec<_> = drawing
        .systems
        .iter()
        .flat_map(|s| &s.marks)
        .filter(|mark| mark.mark.role() == Role::Accidental)
        .collect();
    assert_eq!(accidentals.len(), 7);
    for (index, mark) in accidentals.iter().enumerate() {
        for other in &accidentals[index + 1..] {
            assert!(!mark.bounds.overlaps(other.bounds, 0.0));
        }
    }
}

#[test]
fn tuplet_brackets_follow_the_complete_beam_group() {
    let beamed = draw(r"\relative c' { \tuplet 3/2 { c8 d e } f4 g2 | }", 350.0);
    let bracketed = draw(r"\relative c' { \tuplet 3/2 { c4 d e } f2 | }", 350.0);
    let lines = |drawing: &Engraving| {
        drawing
            .systems
            .iter()
            .flat_map(|s| &s.marks)
            .filter(|m| matches!(m.mark, Mark::Line { role: Role::Tuplet, .. }))
            .count()
    };
    assert_eq!(lines(&beamed), 0);
    assert_eq!(lines(&bracketed), 4);
}

#[test]
fn dots_choose_separate_spaces_and_equal_voice_rests_share_one_glyph() {
    let drawing = draw(r"{ <c' d' e'>2. r4 }", 350.0);
    let dots: Vec<_> = drawing
        .systems
        .iter()
        .flat_map(|s| &s.marks)
        .filter(|m| m.mark.role() == Role::Dot)
        .collect();
    assert_eq!(dots.len(), 3);
    for (index, dot) in dots.iter().enumerate() {
        for other in &dots[index + 1..] {
            assert!(!dot.bounds.overlaps(other.bounds, 0.0));
        }
    }
    let shared = draw(r"<< { r1 } \\ { r1 } >>", 350.0);
    assert_eq!(
        shared
            .systems
            .iter()
            .flat_map(|s| &s.marks)
            .filter(|m| m.mark.role() == Role::Rest)
            .count(),
        1
    );
}

#[test]
fn silent_measures_compress_and_context_changes_have_courtesies() {
    let four = draw("{ R1*4 }", 350.0);
    let forty = draw("{ R1*40 }", 350.0);
    assert!((four.width - forty.width).abs() < 0.01);
    let drawing = draw(r"{ c'1 | \break \clef bass \key d \major \time 3/4 d2. | }", 350.0);
    assert_eq!(drawing.systems.len(), 2);
    let first = &drawing.systems[0];
    assert!(first.marks.iter().any(|m| matches!(m.mark, Mark::Glyph { glyph, .. }
        if drawing.glyphs.get(&glyph).is_some_and(|g| g.name == "fClefSmall"))));
    assert!(drawing.systems[1].marks.iter().any(|m| m.mark.role() == Role::MeasureNumber));
    for system in &drawing.systems {
        assert!(system.marks.iter().all(|m| m.bounds.left >= -0.001
            && m.bounds.right <= drawing.width + 0.001
            && m.bounds.top >= -0.001
            && m.bounds.bottom <= system.height + 0.001));
    }
}

#[test]
fn mixed_durations_and_grace_use_consistent_beat_groups() {
    let fonts = load_fonts(&FontOptions::default()).unwrap();
    for (source, expected) in [
        (r"\relative c'' { c8 d16 e f8 g a16 b c8 d4 | }", vec![3, 2, 3]),
        (r"\relative c'' { \grace { c16 d } c8 d e f g a b c | }", vec![2, 4, 4]),
        (r"\relative c'' { \tuplet 3/2 { c8 d e } f8 g a2 | }", vec![3, 2]),
    ] {
        let loaded =
            gd_lilypond::load_score(source, "groups.ly", &mut gd_lilypond::Options::default())
                .unwrap();
        let mut workspace = Engraver::default();
        workspace.engrave_score(&loaded.score, &fonts, &mut Options::default()).unwrap();
        assert_eq!(
            workspace.plan.beams.iter().map(|group| group.events.len()).collect::<Vec<_>>(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn primary_beam_ends_have_valid_quants_with_dense_inner_beams() {
    let drawing = draw(r"\relative c'' { c16 d32 e64 f128 g128 a8 b4 c2 | }", 350.0);
    for system in &drawing.systems {
        let lines: Vec<_> = system
            .marks
            .iter()
            .filter_map(|m| match m.mark {
                Mark::Line { from, role: Role::Staff, .. } => Some(from.y),
                _ => None,
            })
            .collect();
        let middle = lines.iter().sum::<f64>() / lines.len() as f64;
        let polygons: Vec<_> = system
            .marks
            .iter()
            .filter_map(|m| match m.mark {
                Mark::Polygon { points, role: Role::Beam } => Some(points),
                _ => None,
            })
            .collect();
        assert!(polygons.len() >= 10);
        let left = polygons.iter().map(|p| p[0].x).fold(f64::INFINITY, f64::min);
        let right = polygons.iter().map(|p| p[1].x).fold(f64::NEG_INFINITY, f64::max);
        for (edge, a, b) in [(left, 0, 3), (right, 1, 2)] {
            // These notes have downward stems: the primary beam is the outermost lower strip.
            let strip = polygons
                .iter()
                .filter(|p| (p[a].x - edge).abs() < 1e-9)
                .max_by(|p, q| p[a].y.total_cmp(&q[a].y))
                .unwrap();
            let (top, bottom) = (strip[a].y.min(strip[b].y), strip[a].y.max(strip[b].y));
            if bottom < middle - 9.0 || top > middle + 9.0 {
                continue;
            }
            assert!([top, bottom, (top + bottom) / 2.0].iter().any(|y| {
                let position = (y - middle) / 4.5;
                (position - position.round()).abs() < 0.026
            }));
        }
    }
}

#[test]
fn text_outlines_charge_growth_and_work_before_they_accumulate() {
    let fonts = load_fonts(&FontOptions {
        regular: vec![FaceData { data: REGULAR, ..Default::default() }],
        ..Default::default()
    })
    .unwrap();
    let mut refuse = |_: usize| false;
    let failed = crate::text::shape(
        &fonts,
        "Music",
        12.0,
        gd_score::TextStyle::Regular,
        &mut Vec::new(),
        &mut 100_000,
        &mut Some(&mut refuse),
    )
    .unwrap_err();
    assert_eq!(failed.kind, gd_score::ErrorKind::EngraveLimit);
    let failed = crate::text::shape(
        &fonts,
        &"Music ".repeat(128),
        12.0,
        gd_score::TextStyle::Regular,
        &mut Vec::new(),
        &mut 2000,
        &mut None,
    )
    .unwrap_err();
    assert_eq!(failed.kind, gd_score::ErrorKind::EngraveLimit);
}

#[test]
fn positioned_mark_allowance_is_shared_by_every_system() {
    let fonts = load_fonts(&FontOptions::default()).unwrap();
    let loaded = gd_lilypond::load_score(
        r"{ c'1 | \break d'1 | }",
        "limits.ly",
        &mut gd_lilypond::Options::default(),
    )
    .unwrap();
    let mut workspace = Engraver::default();
    let drawing = workspace.engrave_score(&loaded.score, &fonts, &mut Options::default()).unwrap();
    let maximum = drawing.systems.iter().map(|s| s.marks.len()).max().unwrap();
    assert!(
        workspace
            .engrave_score(
                &loaded.score,
                &fonts,
                &mut Options {
                    limits: Limits { marks: maximum, ..Default::default() },
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn simultaneous_conflicting_accidentals_leave_the_pitch_ambiguous() {
    let drawing = draw(r"<< { cis'2 c'2 } \\ { c'2 s2 } >>", 350.0);
    let accidentals: Vec<_> =
        drawing.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Accidental).collect();
    assert_eq!(accidentals.len(), 3);
}

#[test]
fn long_initial_lyrics_and_annotations_fit_the_horizontal_frame() {
    let drawing = draw(
        r#"{ c'4^\markup "First" ^\markup "Second" d' e' f' }
        \addlyrics { extraordinary words fit here }"#,
        260.0,
    );
    for system in &drawing.systems {
        for mark in &system.marks {
            assert!(mark.bounds.left >= -0.001, "{:?}", mark.mark);
            assert!(mark.bounds.right <= system.width + 0.001, "{:?}", mark.mark);
        }
        let scripts: Vec<_> =
            system.marks.iter().filter(|m| m.mark.role() == Role::Script).collect();
        assert_eq!(scripts.len(), 2);
        assert!(!scripts[0].bounds.overlaps(scripts[1].bounds, 0.0));
    }
}

#[test]
fn lyric_extenders_and_hairpins_continue_on_a_system_without_local_text() {
    let drawing = draw(
        r"{ c'1(\p\< | \break d'1 | \break e'1)\f | }
        \addlyrics { Ah __ }",
        350.0,
    );
    assert_eq!(drawing.systems.len(), 3);
    for system in &drawing.systems[..2] {
        assert!(
            system.marks.iter().any(|m| matches!(m.mark, Mark::Line { role: Role::Dynamic, .. }))
        );
        assert!(
            system.marks.iter().any(|m| matches!(m.mark, Mark::Line { role: Role::Lyric, .. }))
        );
    }
}

#[test]
fn a_pickup_uses_the_full_instrument_name_only_on_its_first_system() {
    let drawing = draw(
        r#"\new Staff \with { instrumentName = "Piano" shortInstrumentName = "Pno." }
        { \partial 4 c'4 | \break d'1 | }"#,
        350.0,
    );
    assert_eq!(drawing.systems.len(), 2);
    let widths: Vec<_> = drawing
        .systems
        .iter()
        .map(|system| {
            system
                .marks
                .iter()
                .find_map(|mark| {
                    if let Mark::Text { run, role: Role::Instrument, .. } = &mark.mark {
                        Some(run.advance)
                    } else {
                        None
                    }
                })
                .unwrap()
        })
        .collect();
    assert!(widths[0] > widths[1]);
}

#[test]
fn a_complete_fallback_face_shapes_a_script_run_as_one_unit() {
    const HINDI: &[u8] =
        include_bytes!("../../../../docs/shared/fonts/NotoSerifDevanagari-Regular.ttf");
    let mixed = load_fonts(&FontOptions {
        regular: vec![
            FaceData { data: REGULAR, ..Default::default() },
            FaceData { data: HINDI, ..Default::default() },
        ],
        ..Default::default()
    })
    .unwrap();
    let direct = load_fonts(&FontOptions {
        regular: vec![FaceData { data: HINDI, ..Default::default() }],
        ..Default::default()
    })
    .unwrap();
    let shape = |fonts: &Fonts<'_>| {
        crate::text::shape(
            fonts,
            "संगीत की शिक्षा",
            12.0,
            gd_score::TextStyle::Regular,
            &mut Vec::new(),
            &mut 1_000_000,
            &mut None,
        )
        .unwrap()
    };
    let (mixed, direct) = (shape(&mixed), shape(&direct));
    assert_eq!(mixed.advance, direct.advance);
    assert_eq!(mixed.glyphs.len(), direct.glyphs.len());
    for (a, b) in mixed.glyphs.iter().zip(&direct.glyphs) {
        assert_eq!((a.x, a.y, &a.outline), (b.x, b.y, &b.outline));
    }
}

#[test]
fn directed_hairpins_stay_on_the_requested_side_of_the_staff() {
    let drawing = draw(r"{ c'2^\< d'2^\f }", 350.0);
    let system = &drawing.systems[0];
    let top = system
        .marks
        .iter()
        .filter(|m| m.mark.role() == Role::Staff)
        .map(|m| m.bounds.top)
        .fold(f64::INFINITY, f64::min);
    let lines: Vec<_> = system
        .marks
        .iter()
        .filter(|m| matches!(m.mark, Mark::Line { role: Role::Dynamic, .. }))
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|m| m.bounds.bottom < top));
}

#[test]
fn rich_markup_shapes_enclosures_columns_dynamics_and_music_glyphs() {
    let drawing = draw(
        r#"\new Staff \with {
          instrumentName = \markup \center-column { "Clarinet" \concat { "in B" \flat } }
        } \relative c'' {
          \tempo \markup { \bold "Allegro" \italic "con brio" }
          \mark \markup \box "A"
          c4^\markup { \dynamic sfz \italic "espr." }
          d4^\markup \circle "1"
          e4_\markup \underline "ten."
          f4^\markup \segno |
        }"#,
        380.0,
    );
    let marks = &drawing.systems[0].marks;
    assert!(marks.iter().any(|m| m.mark.role() == Role::Instrument));
    assert!(marks.iter().any(|m| m.mark.role() == Role::Tempo));
    assert!(marks.iter().any(|m| m.mark.role() == Role::Rehearsal));
    let mut svg = String::new();
    write_svg(&mut svg, &drawing, &SvgOptions::default(), None).unwrap();
    assert!(svg.contains("<path "));
}

#[test]
fn mixed_latin_and_devanagari_string_shapes_contiguous_runs_and_records_sources() {
    struct NamedFace(&'static str, &'static [u8]);
    impl FaceSource for NamedFace {
        fn data(&self) -> Result<&[u8], gd_score::Error> {
            Ok(self.1)
        }
        fn id(&self) -> Option<&str> {
            Some(self.0)
        }
    }
    const HINDI: &[u8] =
        include_bytes!("../../../../docs/shared/fonts/NotoSerifDevanagari-Regular.ttf");
    let latin = NamedFace("latin.otf", REGULAR);
    let hindi = NamedFace("hindi.ttf", HINDI);
    let fonts = load_fonts(&FontOptions {
        regular: vec![
            FaceData { source: Some(&latin), ..Default::default() },
            FaceData { source: Some(&hindi), ..Default::default() },
        ],
        ..Default::default()
    })
    .unwrap();
    let mut warnings = Vec::new();
    let run = crate::text::shape(
        &fonts,
        "αβγ संगीत की",
        12.0,
        gd_score::TextStyle::Regular,
        &mut warnings,
        &mut 1_000_000,
        &mut None,
    )
    .unwrap();
    assert!(warnings.is_empty());
    assert!(run.sources.iter().any(|s| s == "latin.otf"));
    assert!(run.sources.iter().any(|s| s == "hindi.ttf"));
}

#[test]
fn c_clef_key_signatures_stay_within_staff_lines() {
    for clef in ["alto", "tenor", "soprano"] {
        for (tonic, mode) in [("cis", "major"), ("ces", "major")] {
            let source = format!("\\clef {clef} \\key {tonic} \\{mode} c'1");
            let drawing = draw(&source, 350.0);
            let marks = &drawing.systems[0].marks;
            let staff_top = marks
                .iter()
                .filter(|m| m.mark.role() == Role::Staff)
                .map(|m| m.bounds.top)
                .fold(f64::INFINITY, f64::min);
            let staff_bottom = marks
                .iter()
                .filter(|m| m.mark.role() == Role::Staff)
                .map(|m| m.bounds.bottom)
                .fold(f64::NEG_INFINITY, f64::max);
            let space = (staff_bottom - staff_top) / 4.0;
            for sig in marks.iter().filter(|m| m.mark.role() == Role::Signature) {
                let center = (sig.bounds.top + sig.bounds.bottom) * 0.5;
                assert!(center >= staff_top - 0.6 * space, "{source}: {center} < {staff_top}");
                assert!(
                    center <= staff_bottom + 0.6 * space,
                    "{source}: {center} > {staff_bottom}"
                );
            }
        }
    }
}

#[test]
fn multi_voice_seconds_dots_and_accidentals_coordinate_horizontally() {
    let drawing = draw(r"<< \relative c' { d4. c4. } \\ \relative c' { c4. d4. } >>", 350.0);
    let heads: Vec<_> =
        drawing.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Notehead).collect();
    assert_eq!(heads.len(), 4);
    // Marks are ordered by column: [col0_v1, col0_v2, col1_v1, col1_v2].
    // Uncrossed second (d' above c'): lower voice shifts right.
    assert!(heads[1].bounds.left > heads[0].bounds.left + 2.0);
    // Crossed second (c' in voice 1 below d' in voice 2): upper-stem voice shifts right.
    assert!(heads[2].bounds.left > heads[3].bounds.left + 2.0);
    let dots: Vec<_> =
        drawing.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Dot).collect();
    assert_eq!(dots.len(), 4);
    assert!((dots[0].bounds.left - dots[1].bounds.left).abs() < 0.01);

    let acc = draw(r"<< \relative c' { fis2 } \\ \relative c' { f!2 } >>", 350.0);
    let accs: Vec<_> =
        acc.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Accidental).collect();
    assert_eq!(accs.len(), 2);
    assert!(
        accs[0].bounds.right <= accs[1].bounds.left || accs[1].bounds.right <= accs[0].bounds.left
    );
}

#[test]
fn broken_voltas_and_continuation_ties_respect_system_boundaries() {
    let drawing = draw(
        r"\relative c' {
          \key d \major
          \repeat volta 3 { c1 | }
          \alternative {
            \volta 1,2 { g'1~ | \break g1 | }
            \volta 3 { d1 \fine }
          }
        }",
        320.0,
    );
    assert_eq!(drawing.systems.len(), 2);
    let sys1_volta_lines = drawing.systems[0]
        .marks
        .iter()
        .filter(|m| matches!(m.mark, Mark::Line { role: Role::Volta, .. }))
        .count();
    let sys2_volta_lines = drawing.systems[1]
        .marks
        .iter()
        .filter(|m| matches!(m.mark, Mark::Line { role: Role::Volta, .. }))
        .count();
    // Open-ended broken volta on system 1 has left hook + top bar (2 lines);
    // system 2 has continuation volta (2 lines) + complete volta 3 (3 lines) = 5 lines.
    assert_eq!(sys1_volta_lines, 2);
    assert_eq!(sys2_volta_lines, 5);
    let sys2_sigs = drawing.systems[1]
        .marks
        .iter()
        .filter(|m| m.mark.role() == Role::Signature)
        .map(|m| m.bounds.right)
        .fold(0.0f64, f64::max);
    let sys2_tie = drawing.systems[1].marks.iter().find(|m| m.mark.role() == Role::Tie).unwrap();
    assert!(sys2_tie.bounds.left > sys2_sigs);
}

#[test]
fn concave_beams_stay_level_and_three_eight_groups_whole_measure() {
    let drawing = draw(r"\relative c' { \time 3/8 c16 d e f g a | \stemUp c,8 c'' c, | }", 350.0);
    let beams: Vec<_> = drawing.systems[0]
        .marks
        .iter()
        .filter_map(|m| match &m.mark {
            Mark::Polygon { points, role: Role::Beam } => Some((points[0].y, points[1].y)),
            _ => None,
        })
        .collect();
    // 3/8 with 6 sixteenths forms 5+5 polygons; bar 2 forms 2 concave polygons.
    assert_eq!(beams.len(), 12);
    assert!((beams[10].0 - beams[10].1).abs() < 0.01);
    assert!((beams[11].0 - beams[11].1).abs() < 0.01);
}

#[test]
fn reused_engraver_charges_each_ledger_and_svg_aria_is_not_duplicated() {
    let loaded =
        gd_lilypond::load_score("{ c'4 d' e' f' }", "m.ly", &mut gd_lilypond::Options::default())
            .unwrap();
    let fonts = load_fonts(&FontOptions {
        regular: vec![FaceData { data: REGULAR, ..FaceData::default() }],
        ..FontOptions::default()
    })
    .unwrap();
    let mut engraver = Engraver::default();
    let mut charged = Vec::new();
    for _ in 0..4 {
        let mut turn = 0usize;
        let mut reserve = |bytes: usize| {
            turn += bytes;
            true
        };
        let mut opts = Options { reserve: Some(&mut reserve), ..Options::default() };
        let _ = engraver.engrave_score(&loaded.score, &fonts, &mut opts).unwrap();
        charged.push(turn);
    }
    // Each turn supplies a fresh ledger. Warm capacity still belongs to that call.
    assert_eq!(charged[1], charged[3]);

    let drawing = draw("{ c'1 }", 200.0);
    let mut plain_svg = String::new();
    write_svg(&mut plain_svg, &drawing, &SvgOptions::default(), None).unwrap();
    assert!(!plain_svg.contains("aria-labelledby"));
    assert!(!plain_svg.contains("<title>"));

    let mut titled_svg = String::new();
    write_svg(
        &mut titled_svg,
        &drawing,
        &SvgOptions { title: "Melody", description: "Melody", ..SvgOptions::default() },
        None,
    )
    .unwrap();
    assert!(titled_svg.contains("aria-labelledby=\"title\""));
    assert!(!titled_svg.contains("<desc "));
}

#[test]
fn repeated_printed_measure_numbers_do_not_repeat_the_opening_signatures() {
    let mut loaded = gd_lilypond::load_score(
        r"{ c'1 | \break \key g \major g'1 | }",
        "numbers.ly",
        &mut gd_lilypond::Options::default(),
    )
    .unwrap();
    for measure in &mut loaded.score.measures {
        measure.number = 10;
    }
    let fonts = load_fonts(&FontOptions::default()).unwrap();
    let drawing =
        Engraver::default().engrave_score(&loaded.score, &fonts, &mut Options::default()).unwrap();
    assert_eq!(drawing.systems.len(), 2);
    let sharp = fonts.glyph("accidentalSharp").unwrap();
    let marks = &drawing.systems[1].marks;
    let signatures: Vec<_> = marks.iter().filter(|m| m.mark.role() == Role::Signature).collect();
    assert_eq!(signatures.len(), 1);
    assert!(matches!(signatures[0].mark, Mark::Glyph { glyph, .. } if glyph == sharp));
    assert!(marks.iter().any(|m| m.mark.role() == Role::MeasureNumber));
}

#[test]
fn four_unison_voices_keep_four_stems_and_separate_same_side_heads() {
    let drawing = draw(
        r"<< { \voiceOne c'4 } \\ { \voiceTwo c'4 } \\
             { \voiceThree c'4 } \\ { \voiceFour c'4 } >>",
        350.0,
    );
    let heads: Vec<_> =
        drawing.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Notehead).collect();
    assert_eq!(heads.len(), 3);
    for (index, head) in heads.iter().enumerate() {
        for other in &heads[index + 1..] {
            assert!(!head.bounds.overlaps(other.bounds, 0.0));
        }
    }
    let stems: Vec<_> = drawing.systems[0]
        .marks
        .iter()
        .filter_map(|m| match m.mark {
            Mark::Line { from, to, role: Role::Stem, .. } => Some(to.y < from.y),
            _ => None,
        })
        .collect();
    assert_eq!(stems, [true, false, true, false]);
    let neutral = draw(r"{ \voiceThree c'''4 \oneVoice c'''4 }", 350.0);
    let stems: Vec<_> = neutral.systems[0]
        .marks
        .iter()
        .filter_map(|m| match m.mark {
            Mark::Line { from, to, role: Role::Stem, .. } => Some(to.y < from.y),
            _ => None,
        })
        .collect();
    assert_eq!(stems, [true, false]);
}

#[test]
fn four_voice_columns_separate_all_dense_pitch_permutations() {
    let pitches = ["c'", "d'", "e'"];
    for pattern in 0..81 {
        let mut choice = pattern;
        let mut parts = Vec::new();
        for _ in 0..4 {
            parts.push(format!("{{ {}4 }}", pitches[choice % 3]));
            choice /= 3;
        }
        let source = format!("<< {} >>", parts.join(r" \\ "));
        let drawing = draw(&source, 350.0);
        let heads: Vec<_> =
            drawing.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Notehead).collect();
        for (index, head) in heads.iter().enumerate() {
            for other in &heads[index + 1..] {
                assert!(!head.bounds.overlaps(other.bounds, 0.0), "{source}");
            }
        }
    }
}

#[test]
fn four_equal_rests_merge_and_different_rest_durations_do_not_overlap() {
    let equal = draw(r"<< { r1 } \\ { r1 } \\ { r1 } \\ { r1 } >>", 350.0);
    assert_eq!(equal.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Rest).count(), 1);
    let drawing = draw(r"<< { r4 } \\ { r2 } \\ { r8 } \\ { r4. } >>", 350.0);
    let rests: Vec<_> =
        drawing.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Rest).collect();
    assert_eq!(rests.len(), 4);
    for (index, rest) in rests.iter().enumerate() {
        for other in &rests[index + 1..] {
            assert!(!rest.bounds.overlaps(other.bounds, 0.0));
        }
    }
}

#[test]
fn explicit_stem_overrides_and_neutral_survive_inner_voice_policies() {
    let drawing =
        draw(r"{ \voiceThree c'''4 \stemNeutral c'''4 \voiceFour c'4 \stemUp c'4 }", 350.0);
    let stems: Vec<_> = drawing.systems[0]
        .marks
        .iter()
        .filter_map(|m| match m.mark {
            Mark::Line { from, to, role: Role::Stem, .. } => Some(to.y < from.y),
            _ => None,
        })
        .collect();
    assert_eq!(stems, [true, false, false, true]);
}

fn marks_of(engraving: &Engraving, roles: &[Role]) -> Vec<Rect> {
    let marks = engraving.systems.iter().flat_map(|s| &s.marks);
    marks.filter(|m| roles.contains(&m.mark.role())).map(|m| m.bounds).collect()
}

#[test]
fn inner_voice_ties_and_slurs_keep_a_bounded_height() {
    let space = 18.0 / 4.0;
    let ties = draw(r"<< { c''2 c''2 } \\ { c'2 c'2 } \\ { g'2~ g'2 } \\ { e'2~ e'2 } >>", 350.0);
    for tie in marks_of(&ties, &[Role::Tie]) {
        assert!(tie.bottom - tie.top < 3.0 * space, "{tie:?}");
    }
    let slurs =
        draw(r"<< { c''4( d'' e'' f'') } \\ { c'4( d' e' f') } \\ { g'4( a' b' c'') } >>", 350.0);
    for slur in marks_of(&slurs, &[Role::Slur]) {
        assert!(slur.bottom - slur.top < 7.0 * space, "{slur:?}");
    }
}

#[test]
fn marks_of_voices_on_one_side_do_not_print_over_each_other() {
    let engraving = draw(
        r"<< { c''4-. d''-> e''\p f''-1 } \\ { c'4-. d'-> e'\p f'-1 }
             \\ { g'4-. a'-> b'\f c''-2 } \\ { e'4-. f'-> g'\f a'-3 } >>",
        350.0,
    );
    let space = 18.0 / 4.0;
    let marks = marks_of(&engraving, &[Role::Articulation, Role::Fingering]);
    for (i, a) in marks.iter().enumerate() {
        for b in &marks[i + 1..] {
            let wide = a.right.min(b.right) - a.left.max(b.left) > 0.25 * space;
            let tall = a.bottom.min(b.bottom) - a.top.max(b.top) > 0.25 * space;
            assert!(!(wide && tall), "{a:?} {b:?}");
        }
    }
}

#[test]
fn shifted_voices_keep_their_columns_in_time_order() {
    let engraving =
        draw(r"<< { c''4 d''4 } \\ { c'4 d'4 } \\ { e''4 f''4 } \\ { g4 a4 } >>", 350.0);
    let mut heads = marks_of(&engraving, &[Role::Notehead]);
    heads.sort_by(|a, b| a.left.total_cmp(&b.left));
    let (first, second) = heads.split_at(heads.len() / 2);
    let right = first.iter().map(|h| h.right).fold(f64::MIN, f64::max);
    let left = second.iter().map(|h| h.left).fold(f64::MAX, f64::min);
    assert!(right < left, "{first:?} {second:?}");
}

#[test]
fn measure_rests_and_tuplets_follow_their_voice() {
    let engraving = draw(r"<< { d''1 } \\ { R1 } >>", 350.0);
    let head = marks_of(&engraving, &[Role::Notehead])[0];
    for rest in marks_of(&engraving, &[Role::Rest]) {
        assert!(!rest.overlaps(head, 0.0), "{rest:?} {head:?}");
    }
    let tuplets = draw(r"<< { c''2 c''2 } \\ { \tuplet 3/2 { a8 b c' } d'4 e'2 } >>", 350.0);
    let middle = 4.0 * 18.0 / 4.0;
    for number in marks_of(&tuplets, &[Role::Tuplet]) {
        assert!(number.top > middle, "{number:?}");
    }
}

#[test]
fn long_scores_fit_the_default_work_limit() {
    // Line breaking and beam quantization once scanned the whole score or system for each
    // candidate, so sixty bars of beamed eighths exhausted the default work limit.
    let bars = vec!["c8 d e f g f e d |"; 120].join(" ");
    let engraving = draw(&format!("\\relative c' {{ {bars} }}"), 350.0);
    assert!(engraving.systems.len() >= 20);
}

#[test]
fn dynamics_and_hairpins_of_two_voices_stack_instead_of_overprinting() {
    let engraving = draw(r"<< { c''2\p d''\< e''1\! } \\ { c'2\f d'\> e'1\! } >>", 350.0);
    let space = 18.0 / 4.0;
    let signs = marks_of(&engraving, &[Role::Dynamic]);
    // Each of these dynamics is one glyph and each hairpin line one mark, so any two
    // marks that overlap substantially come from different voices.
    for (i, a) in signs.iter().enumerate() {
        for b in &signs[i + 1..] {
            let wide = a.right.min(b.right) - a.left.max(b.left) > 0.25 * space;
            let tall = a.bottom.min(b.bottom) - a.top.max(b.top) > 0.25 * space;
            assert!(!(wide && tall), "{a:?} {b:?}");
        }
    }
}

#[test]
fn explicit_shift_levels_order_columns_away_from_the_stem() {
    let engraving = draw(
        r"\new Staff << \new Voice { \voiceOne g''4 } \new Voice { \voiceThree e''4 }
           \new Voice { \voiceOne \shiftOnn c''4 } \new Voice { \voiceTwo c'4 } >>",
        350.0,
    );
    let mut upper: Vec<_> =
        marks_of(&engraving, &[Role::Notehead]).into_iter().filter(|h| h.bottom < 40.0).collect();
    upper.sort_by(|a, b| a.top.total_cmp(&b.top));
    let [g, e, c] = upper[..] else { panic!("{upper:?}") };
    // Levels 0, 1 and 2 stand half a column width apart, rightward for upward stems.
    assert!(g.left < e.left && e.left < c.left, "{upper:?}");
    assert!(((e.left - g.left) - (c.left - e.left)).abs() < 0.01, "{upper:?}");
}

fn staff_marks(engraving: &Engraving, staff: u32, roles: &[Role]) -> Vec<Rect> {
    let marks = engraving.systems.iter().flat_map(|s| &s.marks);
    marks
        .filter(|m| m.staff == gd_score::StaffId(staff) && roles.contains(&m.mark.role()))
        .map(|m| m.bounds)
        .collect()
}

#[test]
fn tab_staves_put_fret_numbers_on_six_strings() {
    let engraving = draw(r"\new TabStaff { e4 c'\2 <c e g>2 }", 350.0);
    let space = 18.0 / 4.0;
    let mut lines: Vec<f64> = staff_marks(&engraving, 0, &[Role::Staff])
        .iter()
        .map(|line| ((line.top + line.bottom) / 2.0 * 100.0).round() / 100.0)
        .collect();
    lines.sort_by(f64::total_cmp);
    lines.dedup();
    assert_eq!(lines.len(), 6, "{lines:?}");
    for pair in lines.windows(2) {
        assert!((pair[1] - pair[0] - 1.5 * space).abs() < 0.05, "{lines:?}");
    }
    let frets = staff_marks(&engraving, 0, &[Role::Fret]);
    assert_eq!(frets.len(), 5, "{frets:?}");
    // Every number sits centered on a string line.
    for fret in &frets {
        let center = (fret.top + fret.bottom) / 2.0;
        assert!(lines.iter().any(|line| (line - center).abs() < 0.3 * space), "{fret:?}");
    }
    assert!(staff_marks(&engraving, 0, &[Role::Notehead, Role::Stem]).is_empty());
}

#[test]
fn tab_staves_omit_rests_scripts_ties_and_signatures() {
    let music = r"{ \time 3/4 e8( f) g4-. r\p | e2.~ | e2. }";
    let engraving = draw(
        &format!(
            r"\new StaffGroup << \new Staff {{ \clef treble_8 {music} }} \new TabStaff {music} >>"
        ),
        350.0,
    );
    let hidden = [
        Role::Rest,
        Role::Articulation,
        Role::Dynamic,
        Role::Tie,
        Role::Slur,
        Role::Signature,
        Role::Notehead,
        Role::Stem,
        Role::Beam,
    ];
    assert!(staff_marks(&engraving, 1, &hidden).is_empty());
    assert!(!staff_marks(&engraving, 0, &[Role::Rest, Role::Tie]).is_empty());
    // The tied continuation is not repeated: e8 f g and one tied E.
    assert_eq!(staff_marks(&engraving, 1, &[Role::Fret]).len(), 4);
}

#[test]
fn tab_ties_across_a_line_break_repeat_the_number_in_parentheses() {
    let engraving = draw(r"\new TabStaff { e'1~ \break e'1 }", 350.0);
    assert_eq!(engraving.systems.len(), 2);
    let second = engraving.systems[1].marks.iter().filter(|m| m.mark.role() == Role::Fret);
    let widths: Vec<f64> = second.map(|m| m.bounds.right - m.bounds.left).collect();
    let first = engraving.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Fret);
    let plain: Vec<f64> = first.map(|m| m.bounds.right - m.bounds.left).collect();
    assert_eq!((plain.len(), widths.len()), (1, 1));
    assert!(widths[0] > plain[0] * 2.0, "{plain:?} {widths:?}");
}

#[test]
fn breath_marks_stand_above_the_staff_and_make_room() {
    let plain = draw(r"{ \time 4/4 c''2 d''2 | }", 350.0);
    let breathed = draw(r"{ \time 4/4 c''2 \breathe d''2 | }", 350.0);
    let commas = marks_of(&breathed, &[Role::Articulation]);
    assert_eq!(commas.len(), 1, "{commas:?}");
    let lines = marks_of(&breathed, &[Role::Staff]);
    let top = lines.iter().map(|line| line.top).fold(f64::MAX, f64::min);
    assert!(commas[0].bottom <= top + 0.5, "{commas:?} above {top}");
    // The comma stands between the two notes, and the second note moves right for it.
    let heads = marks_of(&breathed, &[Role::Notehead]);
    assert!(heads[0].right < commas[0].left && commas[0].right < heads[1].left, "{heads:?}");
    let before = marks_of(&plain, &[Role::Notehead]);
    assert!(heads[1].left - heads[0].left > before[1].left - before[0].left, "{before:?}");
}

#[test]
fn string_numbers_are_circled_on_the_staff_and_absent_from_tablature() {
    let music = r"{ \time 4/4 e'4\2 d' c'\3 b | << { e'1\2 } \\ { c1\5 } >> | }";
    let engraving = draw(
        &format!(
            r"\new StaffGroup << \new Staff {{ \clef treble_8 {music} }} \new TabStaff {music} >>"
        ),
        350.0,
    );
    let numbers = staff_marks(&engraving, 0, &[Role::Fingering]);
    assert_eq!(numbers.len(), 4, "{numbers:?}");
    assert!(staff_marks(&engraving, 1, &[Role::Fingering]).is_empty());
    // The lower voice's number goes below the staff, the others above it.
    let lines = staff_marks(&engraving, 0, &[Role::Staff]);
    let top = lines.iter().map(|line| line.top).fold(f64::MAX, f64::min);
    let bottom = lines.iter().map(|line| line.bottom).fold(f64::MIN, f64::max);
    assert_eq!(numbers.iter().filter(|n| n.bottom < top).count(), 3, "{numbers:?}");
    assert_eq!(numbers.iter().filter(|n| n.top > bottom).count(), 1, "{numbers:?}");
}

#[test]
fn metronome_marks_draw_a_note_symbol() {
    let engraving = draw(r#"{ \tempo "Allegro" 4. = 60 c'2. }"#, 350.0);
    let tempo =
        engraving.systems.iter().flat_map(|s| &s.marks).find(|m| m.mark.role() == Role::Tempo);
    let Some(PositionedMark { mark: Mark::Text { run, .. }, .. }) = tempo else {
        panic!("no tempo text");
    };
    // "Allegro (", a note, a dot and " = 60)" shape into one run with the note glyphs.
    assert!(run.glyphs.len() >= "Allegro(=60)".len() + 2, "{}", run.glyphs.len());

    // In `\tempo 4.. = 60`, the note and both augmentation dots share the lifted baseline
    // and keep positive horizontal gaps between the note and each dot.
    let double_dotted = draw(r"{ \tempo 4.. = 60 c'2.. r16 }", 350.0);
    let Some(PositionedMark { mark: Mark::Text { run: met, .. }, .. }) =
        double_dotted.systems.iter().flat_map(|s| &s.marks).find(|m| m.mark.role() == Role::Tempo)
    else {
        panic!("no double-dotted tempo");
    };
    assert!((met.glyphs[0].y - met.glyphs[1].y).abs() < 1e-6);
    assert!((met.glyphs[1].y - met.glyphs[2].y).abs() < 1e-6);
    assert!(met.glyphs[0].y < -1.0, "{}", met.glyphs[0].y);
    assert!(met.glyphs[1].x > met.glyphs[0].x + 2.0);
    assert!(met.glyphs[2].x > met.glyphs[1].x + 1.0);
}

#[test]
fn solo_tab_staves_space_outer_strings_tied_breaks_and_omit_signature_gaps() {
    // A solo TabStaff with a wide key signature does not reserve blank signature space.
    let c_tab = draw(r"\new TabStaff { \key c \major e4 f g a }", 350.0);
    let b_tab = draw(r"\new TabStaff { \key b \major e4 fis gis a }", 350.0);
    let first_c = marks_of(&c_tab, &[Role::Fret])[0].left;
    let first_b = marks_of(&b_tab, &[Role::Fret])[0].left;
    assert!((first_c - first_b).abs() < 0.01, "{first_c} vs {first_b}");

    // Two-digit frets on string 1 and string 6 clear internal bar lines and tied breaks.
    let outer = draw(
        r"\new TabStaff { f4\6 e'''4\1 f4\6 e'''4\1 | e'''1~\1 \break e'''8\1 d'''8\1 r2. }",
        350.0,
    );
    assert_eq!(outer.systems.len(), 2);
    let sys0_bars = outer.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Bar);
    let sys0_frets: Vec<_> =
        outer.systems[0].marks.iter().filter(|m| m.mark.role() == Role::Fret).collect();
    for bar in sys0_bars {
        for fret in &sys0_frets {
            let overlap =
                fret.bounds.left < bar.bounds.right && bar.bounds.left < fret.bounds.right;
            assert!(!overlap, "{fret:?} overlaps {bar:?}");
        }
    }
    let sys1_clef =
        outer.systems[1].marks.iter().find(|m| m.mark.role() == Role::Clef).unwrap().bounds;
    let sys1_frets: Vec<_> =
        outer.systems[1].marks.iter().filter(|m| m.mark.role() == Role::Fret).collect();
    assert!(sys1_frets[0].bounds.left > sys1_clef.right);
    assert!(sys1_frets[0].bounds.right < sys1_frets[1].bounds.left);
}
