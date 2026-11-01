use gd_score::Error;

use crate::limits::{charge, failure};

/// One gap's preferred length and non-negotiable collision clearance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    /// Positive preferred length in staff spaces.
    pub ideal: f64,
    /// Nonnegative minimum length in staff spaces.
    pub minimum: f64,
}

/// A clearance constraint spanning one or more adjacent gaps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rod {
    /// First gap, inclusive.
    pub first: usize,
    /// Last gap, exclusive.
    pub last: usize,
    /// Required sum of those gaps.
    pub minimum: f64,
}

/// The unique spring-energy minimizer after conservative spanning-rod conversion.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solution {
    /// Nonnegative gap lengths satisfying every original rod.
    pub gaps: Vec<f64>,
    /// Proportional spring stretch; zero on an overfull line.
    pub stretch: f64,
    /// Whether the minimum width exceeds the requested width.
    pub overfull: bool,
    /// Sum of converted minimum clearances.
    pub minimum: f64,
    /// Sum at stretch one, including bound rods.
    pub natural: f64,
}

/// Borrows gaps and rods and solves for a positive finite width.
/// Fails for invalid dimensions, rod indices, memory refusal or the default work limit.
pub fn solve_spacing(springs: &[Spring], rods: &[Rod], width: f64) -> Result<Solution, Error> {
    solve(springs, rods, width, &mut crate::Limits::default().work)
}

pub(crate) fn solve(
    springs: &[Spring],
    rods: &[Rod],
    width: f64,
    work: &mut usize,
) -> Result<Solution, Error> {
    charge(work, springs.len().saturating_add(rods.len()))?;
    if !width.is_finite() || width <= 0.0 || springs.len() > 100_000 {
        return Err(failure("Invalid spacing width or gap count."));
    }
    let mut gaps = Vec::new();
    gaps.try_reserve_exact(springs.len()).map_err(|_| failure("Cannot reserve spacing gaps."))?;
    for spring in springs {
        if !spring.ideal.is_finite()
            || spring.ideal <= 0.0
            || !spring.minimum.is_finite()
            || spring.minimum < 0.0
        {
            return Err(failure(
                "Springs need positive finite ideals and nonnegative finite rods.",
            ));
        }
        gaps.push(spring.minimum);
    }
    convert_rods(springs, rods, &mut gaps, work)?;
    let minimum: f64 = gaps.iter().sum();
    let natural: f64 = gaps.iter().zip(springs).map(|(r, s)| r.max(s.ideal)).sum();
    if !minimum.is_finite() || !natural.is_finite() {
        return Err(failure("The spacing widths overflowed."));
    }
    let overfull = minimum > width;
    let stretch =
        if overfull || width == minimum { 0.0 } else { find_stretch(springs, &gaps, width)? };
    for (gap, spring) in gaps.iter_mut().zip(springs) {
        *gap = gap.max(stretch * spring.ideal);
    }
    Ok(Solution { gaps, stretch, overfull, minimum, natural })
}

fn convert_rods(
    springs: &[Spring],
    rods: &[Rod],
    gaps: &mut [f64],
    work: &mut usize,
) -> Result<(), Error> {
    let mut order = Vec::new();
    order.try_reserve_exact(rods.len()).map_err(|_| failure("Cannot reserve spacing rods."))?;
    for rod in rods {
        if rod.first >= rod.last
            || rod.last > gaps.len()
            || !rod.minimum.is_finite()
            || rod.minimum < 0.0
        {
            return Err(failure("A spacing rod has invalid indices or clearance."));
        }
        order.push(*rod);
    }
    order.sort_by(|a, b| {
        (a.last - a.first, a.first)
            .cmp(&(b.last - b.first, b.first))
            .then_with(|| a.minimum.total_cmp(&b.minimum))
    });
    for rod in order {
        charge(work, 3 * (rod.last - rod.first))?;
        let current: f64 = gaps[rod.first..rod.last].iter().sum();
        let missing = rod.minimum - current;
        if missing <= 0.0 {
            continue;
        }
        let ideal: f64 = springs[rod.first..rod.last].iter().map(|s| s.ideal).sum();
        for (gap, spring) in
            gaps[rod.first..rod.last].iter_mut().zip(&springs[rod.first..rod.last])
        {
            *gap += missing * spring.ideal / ideal;
        }
    }
    Ok(())
}

fn find_stretch(springs: &[Spring], rods: &[f64], width: f64) -> Result<f64, Error> {
    if springs.is_empty() {
        return Ok(1.0);
    }
    let mut thresholds = Vec::new();
    thresholds
        .try_reserve_exact(springs.len())
        .map_err(|_| failure("Cannot reserve spring breakpoints."))?;
    for (index, (spring, rod)) in springs.iter().zip(rods).enumerate() {
        thresholds.push((rod / spring.ideal, index));
    }
    thresholds.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut bound: f64 = rods.iter().sum();
    let mut free = 0.0;
    for (threshold, index) in thresholds {
        if free > 0.0 && bound + free * threshold >= width {
            return Ok(((width - bound) / free).max(0.0));
        }
        bound -= rods[index];
        free += springs[index].ideal;
    }
    Ok(((width - bound) / free).max(0.0))
}
