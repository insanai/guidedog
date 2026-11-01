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
