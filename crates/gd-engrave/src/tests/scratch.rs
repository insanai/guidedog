use std::cell::Cell;

use crate::skyline::{Segment, Skyline};
use crate::*;

#[test]
fn skyline_charges_live_buffers_and_refunds_replacements_and_failures() {
    let (held, total, limit) = (Cell::new(0usize), Cell::new(0usize), Cell::new(65_536usize));
    let reserve = |bytes| {
        if held.get() + bytes > limit.get() {
            return false;
        }
        held.set(held.get() + bytes);
        total.set(total.get() + bytes);
        true
    };
    let release = |bytes| held.set(held.get().checked_sub(bytes).unwrap());
    let scratch = Scratch { reserve: Some(&reserve), release: Some(&release) };
    {
        let mut skyline = Skyline::new(scratch);
        for i in 0..100 {
            let x = f64::from(i);
            skyline
                .add(
                    Segment { left: x, right: x + 1.0, first: x % 2.0, last: x % 2.0 },
                    true,
                    &mut 1_000_000,
                )
                .unwrap();
        }
        assert!(held.get() > 0);
        assert!(total.get() > limit.get());
        let before = held.get();
        limit.set(before);
        let segment = Segment { left: 0.0, right: 10.0, first: -1.0, last: -1.0 };
        assert!(skyline.add(segment, true, &mut 100_000).is_err());
        assert_eq!(held.get(), before);
        limit.set(usize::MAX);
        assert!(skyline.add(segment, true, &mut 1).is_err());
        assert_eq!(held.get(), before);
    }
    assert_eq!(held.get(), 0);
}

#[test]
fn spacing_and_breaking_refund_scratch_when_a_problem_is_invalid() {
    let held = Cell::new(0usize);
    let reserve = |bytes| {
        held.set(held.get() + bytes);
        true
    };
    let release = |bytes| held.set(held.get().checked_sub(bytes).unwrap());
    let scratch = Scratch { reserve: Some(&reserve), release: Some(&release) };
    let springs = [Spring { ideal: 2.0, minimum: 1.0 }; 4];
    let invalid = [Rod { first: 0, last: 8, minimum: 2.0 }];
    assert!(crate::spacing::solve(&springs, &invalid, 10.0, &mut 100_000, scratch).is_err());
    assert_eq!(held.get(), 0);
    let breaks: Vec<_> = (0..=4).map(|gap| BreakPoint { gap, ..Default::default() }).collect();
    assert!(
        crate::breaking::choose(
            &springs,
            &[],
            &breaks,
            (10.0, Justify::Auto),
            &Limits::default(),
            &mut 7,
            scratch
        )
        .is_err()
    );
    assert_eq!(held.get(), 0);
}
