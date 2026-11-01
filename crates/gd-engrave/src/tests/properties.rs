use crate::*;

#[test]
fn duration_space_is_monotone_and_continuous_at_the_shortest_duration() {
    for shortest in [1.0 / 128.0, 1.0 / 16.0, 3.0 / 16.0] {
        let mut previous = 0.0;
        for numerator in 1..=1024 {
            let duration = f64::from(numerator) / 512.0;
            let ideal = crate::columns::duration_space(duration, shortest);
            assert!(ideal >= previous);
            previous = ideal;
        }
        assert!((crate::columns::duration_space(shortest, shortest) - 2.4).abs() < 1e-12);
    }
}

#[test]
fn spacing_matches_exhaustive_active_sets() {
    for seed in 0..256usize {
        let springs: Vec<_> = (0..5)
            .map(|i| Spring {
                ideal: ((seed >> i) % 5 + 1) as f64,
                minimum: ((seed >> (i + 2)) % 7) as f64 / 2.0,
            })
            .collect();
        let width = springs.iter().map(|s| s.minimum).sum::<f64>() + (seed % 23 + 1) as f64;
        let solution = solve_spacing(&springs, &[], width).unwrap();
        let mut optimum = f64::INFINITY;
        for mask in 0..1usize << springs.len() {
            let (mut bound, mut free) = (0.0, 0.0);
            for (i, spring) in springs.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    bound += spring.minimum;
                } else {
                    free += spring.ideal;
                }
            }
            if free == 0.0 {
                continue;
            }
            let stretch = (width - bound) / free;
            let gaps: Vec<_> = springs
                .iter()
                .enumerate()
                .map(|(i, s)| if mask & (1 << i) != 0 { s.minimum } else { stretch * s.ideal })
                .collect();
            if gaps.iter().zip(&springs).all(|(gap, s)| *gap >= s.minimum - 1e-9) {
                optimum = optimum.min(energy(&springs, &gaps));
            }
        }
        assert!((solution.gaps.iter().sum::<f64>() - width).abs() < 1e-9);
        assert!((energy(&springs, &solution.gaps) - optimum).abs() < 1e-9, "{seed}");
        for span in 2..=5 {
            let rod = Rod { first: 0, last: span, minimum: width * 0.8 };
            let solved = solve_spacing(&springs, &[rod], width).unwrap();
            assert!(solved.gaps[..span].iter().sum::<f64>() >= rod.minimum - 1e-9);
        }
    }
}

fn energy(springs: &[Spring], gaps: &[f64]) -> f64 {
    springs.iter().zip(gaps).map(|(s, g)| (g - s.ideal).powi(2) / s.ideal).sum()
}

#[test]
fn dynamic_breaking_matches_every_partition_of_small_scores() {
    for seed in 0..128usize {
        let springs: Vec<_> = (0..7)
            .map(|i| Spring {
                ideal: ((seed >> i) % 5 + 4) as f64,
                minimum: ((seed >> (i + 1)) % 3 + 2) as f64,
            })
            .collect();
        let breaks: Vec<_> = (0..=7)
            .map(|gap| BreakPoint {
                gap,
                forced: gap == 0 || (seed % 5 == 0 && gap == 3),
                forbidden: seed % 7 == 0 && gap == 4,
                penalty: if gap == 5 { -5 } else { 0 },
                ..Default::default()
            })
            .collect();
        let width = (seed % 12 + 14) as f64;
        // Rods of one to three gaps, some binding, so the early stop in the breaker is
        // checked against lines whose minimum width comes from rods, not springs alone.
        let rods: Vec<_> = (0..6)
            .filter(|i| (seed >> i) % 3 != 0)
            .map(|i| {
                let last = (i + 1 + (seed + i) % 3).min(7);
                Rod { first: i, last, minimum: ((seed >> i) % 7 + 3) as f64 }
            })
            .collect();
        let chosen =
            break_lines(&springs, &rods, &breaks, width, Justify::Auto, &Limits::default())
                .unwrap();
        let mut optimum = f64::INFINITY;
        for mask in 0..64usize {
            let endpoints: Vec<_> = std::iter::once(0)
                .chain((1..7).filter(|i| mask & (1 << (i - 1)) != 0))
                .chain(std::iter::once(7))
                .collect();
            let mut cost = 0.0;
            for pair in endpoints.windows(2) {
                let (first, last) = (pair[0], pair[1]);
                if breaks[first + 1..last].iter().any(|b| b.forced)
                    || (last < 7 && breaks[last].forbidden)
                {
                    cost = f64::INFINITY;
                    break;
                }
                let index =
                    crate::breaking::RodIndex::new(&rods, &mut 100, Scratch::default()).unwrap();
                let (line, _) = crate::breaking::candidate(
                    &springs,
                    &index,
                    &breaks,
                    (width, Justify::Auto),
                    first..last,
                    &mut 100_000,
                    Scratch::default(),
                )
                .unwrap();
                if line.overfull && last > first + 1 && !breaks[last].forced {
                    cost = f64::INFINITY;
                    break;
                }
                cost += demerit(&line, &breaks);
            }
            optimum = optimum.min(cost);
        }
        let actual: f64 = chosen.iter().map(|l| demerit(l, &breaks)).sum();
        assert!((actual - optimum).abs() < 1e-6, "{seed}: {actual} != {optimum}");
    }
}

fn demerit(line: &Line, breaks: &[BreakPoint]) -> f64 {
    let badness = if line.overfull {
        10_000.0
    } else if line.stretch >= 1.0 {
        100.0 * (line.stretch - 1.0).powi(3)
    } else if line.stretch > 0.0 {
        200.0 * (1.0 / line.stretch - 1.0).powi(3)
    } else {
        10_000.0
    };
    let penalty = f64::from(breaks[line.last].penalty);
    (10.0 + badness).powi(2) + if penalty < 0.0 { -penalty * penalty } else { penalty * penalty }
}

#[test]
fn an_overfull_line_ending_at_a_forced_break_is_still_found() {
    // Inner boundaries are forbidden, so the only legal line holds all three measures;
    // it is overfull, which a forced end permits. The breaker's early stop must not
    // give up on it.
    let springs = vec![Spring { ideal: 10.0, minimum: 8.0 }; 3];
    let breaks: Vec<_> = (0..=3)
        .map(|gap| BreakPoint {
            gap,
            forced: gap == 0 || gap == 3,
            forbidden: gap == 1 || gap == 2,
            ..Default::default()
        })
        .collect();
    let lines =
        break_lines(&springs, &[], &breaks, 12.0, Justify::Auto, &Limits::default()).unwrap();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].overfull && lines[0].first == 0 && lines[0].last == 3);
}
