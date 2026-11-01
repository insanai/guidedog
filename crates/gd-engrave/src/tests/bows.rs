use crate::*;

#[test]
fn sampled_curve_search_clears_inner_ink_and_honors_work_bounds() {
    let obstacle = PositionedMark {
        mark: Mark::Line {
            from: Point { x: 35.0, y: 18.0 },
            to: Point { x: 45.0, y: 18.0 },
            thickness: 3.0,
            role: Role::Beam,
        },
        bounds: Rect { left: 35.0, right: 45.0, top: 16.5, bottom: 19.5 },
        staff: gd_score::StaffId(0),
        measure: 1,
    };
    let request = crate::bows::Request {
        from: Point { x: 10.0, y: 24.0 },
        to: Point { x: 70.0, y: 24.0 },
        middle: 30.0,
        space: 4.5,
        up: true,
        tie: false,
        thickness: (0.4, 0.9),
        obstacles: &[obstacle],
        staff: gd_score::StaffId(0),
        polyphonic: false,
    };
    let (outer, _) = crate::bows::choose(&request, &mut 100_000).unwrap();
    for sample in 1..=16 {
        let point = outer.point(f64::from(sample) / 17.0);
        if (34.1..=45.9).contains(&point.x) {
            assert!(point.y < 15.6, "{point:?}");
        }
    }
    assert!(crate::bows::choose(&request, &mut 10).is_err());
}
