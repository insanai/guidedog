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
        )
        .unwrap();
        assert_eq!(run.sources, ["selected-font.otf"]);
    }
    assert_eq!(source.0.load(std::sync::atomic::Ordering::Relaxed), 2);
}
