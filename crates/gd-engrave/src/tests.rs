#![allow(clippy::unwrap_used)]

use crate::*;

mod bows;
mod native;
mod properties;
mod scratch;

#[test]
fn every_music_glyph_has_an_outline_and_finite_bounds() {
    let fonts = load_fonts(&FontOptions::default()).unwrap();
    for (name, _) in crate::fonts::GLYPHS {
        let glyph = fonts.glyph(name).unwrap();
        let data = fonts.glyph_data(glyph).unwrap();
        assert!(!data.outline.is_empty(), "{name}");
        assert!(data.bounds.left.is_finite() && data.bounds.bottom.is_finite());
    }
    assert!(fonts.glyph("not-a-glyph").is_err());
}

#[test]
fn fixed_elementary_functions_match_known_values() {
    for value in [0.001, 0.0625, 0.25, 0.9, 1.0, 1.1, 2.0, 3.0, 10.0, 1000.0] {
        assert!((crate::math::log2(value) - value.log2()).abs() < 1e-12, "{value}");
        assert!((crate::math::atan(value) - value.atan()).abs() < 1e-12, "{value}");
    }
}

#[test]
fn solver_satisfies_spanning_rods_and_energy_stationarity() {
    let springs = [
        Spring { ideal: 2.0, minimum: 1.0 },
        Spring { ideal: 3.0, minimum: 2.0 },
        Spring { ideal: 1.0, minimum: 0.5 },
    ];
    let rods = [Rod { first: 0, last: 2, minimum: 5.0 }, Rod { first: 1, last: 3, minimum: 4.0 }];
    for width in [10.0, 12.0, 20.0] {
        let solution = solve_spacing(&springs, &rods, width).unwrap();
        assert!(!solution.overfull);
        assert!((solution.gaps.iter().sum::<f64>() - width).abs() < 1e-9);
        for rod in rods {
            assert!(solution.gaps[rod.first..rod.last].iter().sum::<f64>() + 1e-9 >= rod.minimum);
        }
        for (gap, spring) in solution.gaps.iter().zip(springs) {
            assert!(*gap >= spring.minimum);
            assert!(*gap + 1e-9 >= solution.stretch * spring.ideal);
        }
    }
    let overfull = solve_spacing(&springs, &rods, 2.0).unwrap();
    assert!(overfull.overfull);
}

#[test]
fn forced_endpoint_allows_intermediate_lines_and_forbids_crossing() {
    let springs = [Spring { ideal: 10.0, minimum: 8.0 }; 6];
    let breaks: Vec<_> = (0..=6)
        .map(|gap| BreakPoint {
            gap,
            measure: gap as u32 + 1,
            forced: gap == 0 || gap == 6,
            ..BreakPoint::default()
        })
        .collect();
    let lines =
        break_lines(&springs, &[], &breaks, 20.0, Justify::All, &Limits::default()).unwrap();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].first, 0);
    assert_eq!(lines[2].last, 6);
    let mut breaks = breaks;
    breaks[3].forced = true;
    let lines =
        break_lines(&springs, &[], &breaks, 30.0, Justify::All, &Limits::default()).unwrap();
    assert!(lines.iter().any(|line| line.last == 3));
    assert!(!lines.iter().any(|line| line.first < 3 && line.last > 3));
}

#[test]
fn invalid_spacing_and_tiny_work_budgets_are_errors() {
    for spring in [Spring { ideal: 0.0, minimum: 1.0 }, Spring { ideal: 1.0, minimum: f64::NAN }] {
        assert!(solve_spacing(&[spring], &[], 12.0).is_err());
    }
    let springs = [Spring { ideal: 2.0, minimum: 1.0 }; 3];
    assert!(solve_spacing(&springs, &[Rod { first: 0, last: 4, minimum: 5.0 }], 10.0).is_err());
    let breaks = [BreakPoint::default(), BreakPoint { gap: 3, ..BreakPoint::default() }];
    assert!(
        break_lines(
            &springs,
            &[],
            &breaks,
            20.0,
            Justify::All,
            &Limits { work: 1, ..Limits::default() }
        )
        .is_err()
    );
}

#[test]
fn svg_is_standalone_escaped_and_transactional_under_limits() {
    let fonts = load_fonts(&FontOptions::default()).unwrap();
    let glyph = fonts.glyph("noteheadBlack").unwrap();
    let mut engraving = Engraving { width: 100.0, height: 30.0, ..Engraving::default() };
    engraving.glyphs.insert(glyph, fonts.glyph_data(glyph).unwrap().clone());
    engraving.systems.push(System {
        height: 30.0,
        first_measure: 1,
        last_measure: 1,
        marks: vec![PositionedMark {
            mark: Mark::Glyph { glyph, x: 5.0, y: 15.0, size: 4.5, role: Role::Notehead },
            staff: gd_score::StaffId(0),
            measure: 1,
            bounds: Rect::default(),
        }],
        ..System::default()
    });
    let mut out = String::new();
    write_svg(
        &mut out,
        &engraving,
        &SvgOptions { title: "<Music & score>", adaptive: true, ..SvgOptions::default() },
        None,
    )
    .unwrap();
    assert!(out.contains("&lt;Music &amp; score&gt;"));
    assert!(out.contains("data-measure=\"1\""));
    assert!(out.contains("prefers-color-scheme"));
    assert!(!out.contains(".000"));
    let mut out = "prefix".to_string();
    assert!(
        write_svg(
            &mut out,
            &engraving,
            &SvgOptions { max_bytes: 32, ..SvgOptions::default() },
            None
        )
        .is_err()
    );
    assert_eq!(out, "prefix");
    let mut refuse = |_: usize| false;
    assert!(write_svg(&mut out, &engraving, &SvgOptions::default(), Some(&mut refuse)).is_err());
    assert_eq!(out, "prefix");
}

#[test]
fn svg_rejects_a_missing_glyph_definition_without_changing_the_prefix() {
    let drawing = Engraving {
        width: 30.0,
        height: 20.0,
        systems: vec![System {
            height: 20.0,
            marks: vec![PositionedMark {
                mark: Mark::Glyph {
                    glyph: Glyph(65535),
                    x: 5.0,
                    y: 10.0,
                    size: 4.5,
                    role: Role::Notehead,
                },
                staff: gd_score::StaffId(0),
                measure: 1,
                bounds: Rect::default(),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut output = "prefix".to_string();
    assert!(write_svg(&mut output, &drawing, &SvgOptions::default(), None).is_err());
    assert_eq!(output, "prefix");
}
