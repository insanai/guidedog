use gd_score::Error;

use crate::limits::{charge, failure};
use crate::spacing::solve;
use crate::{Justify, Limits, Rod, Spring};

/// A legal measure boundary; the first and last entries delimit the complete score.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BreakPoint {
    /// Gap index at this boundary.
    pub gap: usize,
    /// Printed number of the following measure.
    pub measure: u32,
    /// Preferred break penalty; a negative value rewards this boundary.
    pub penalty: i32,
    /// No candidate may cross this boundary.
    pub forced: bool,
    /// No candidate may end here, except the final score boundary.
    pub forbidden: bool,
    /// Clef, signatures and instrument-name width at a line starting here.
    pub prefix: f64,
    /// Instrument-name indent before the staff lines begin.
    pub indent: f64,
    /// Courtesy signatures before a line ending here.
    pub suffix: f64,
}

/// One line in the chosen optimal breaking within the bounded candidate window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    /// Starting breakpoint index.
    pub first: usize,
    /// Ending breakpoint index.
    pub last: usize,
    /// Selected proportional stretch.
    pub stretch: f64,
    /// Actual width including prefatory and courtesy matter.
    pub width: f64,
    /// Whether a minimum-clearance line exceeds its available width.
    pub overfull: bool,
}

/// Borrows a spacing problem and returns its optimal bounded measure breaking.
/// Fails for invalid boundaries, exhausted work or system limits, or contradictory breaks.
pub fn break_lines(
    springs: &[Spring],
    rods: &[Rod],
    breaks: &[BreakPoint],
    width: f64,
    justify: Justify,
    limits: &Limits,
) -> Result<Vec<Line>, Error> {
    let mut work = limits.work;
    choose(springs, rods, breaks, (width, justify), limits, &mut work, crate::Scratch::default())
}

pub(crate) fn choose(
    springs: &[Spring],
    rods: &[Rod],
    breaks: &[BreakPoint],
    settings: (f64, Justify),
    limits: &Limits,
    work: &mut usize,
    scratch: crate::Scratch<'_>,
) -> Result<Vec<Line>, Error> {
    let (width, justify) = settings;
    if breaks.len() < 2
        || breaks[0].gap != 0
        || breaks.last().map(|b| b.gap) != Some(springs.len())
        || !width.is_finite()
        || width <= 0.0
        || limits.measures_per_line == 0
        || breaks.windows(2).any(|b| b[0].gap > b[1].gap)
        || breaks.iter().any(|b| {
            !b.prefix.is_finite() || b.prefix < 0.0 || !b.suffix.is_finite() || b.suffix < 0.0
        })
    {
        return Err(failure("Invalid measure boundaries or line width."));
    }
    charge(work, breaks.len())?;
    let _choices =
        scratch.hold(breaks.len().saturating_mul(size_of::<f64>() + size_of::<Option<Line>>()))?;
    let (mut costs, mut chosen) = choice_buffers(breaks.len())?;
    let rods = RodIndex::new(rods, work, scratch)?;
    costs[0] = 0.0;
    for last in 1..breaks.len() {
        if breaks[last].forbidden && last + 1 < breaks.len() {
            continue;
        }
        let lower = last.saturating_sub(limits.measures_per_line);
        for first in (lower..last).rev() {
            charge(work, 1)?;
            let (line, content) =
                candidate(springs, &rods, breaks, (width, justify), first..last, work, scratch)?;
            if costs[first].is_finite() {
                let allowed = !line.overfull || last == first + 1 || breaks[last].forced;
                if allowed {
                    let badness = if line.overfull { 10_000.0 } else { badness(line.stretch) };
                    let penalty = f64::from(breaks[last].penalty);
                    let cost = costs[first]
                        + (10.0 + badness).powi(2)
                        + if penalty < 0.0 { -penalty * penalty } else { penalty * penalty };
                    if cost < costs[last] {
                        costs[last] = cost;
                        chosen[last] = Some(line);
                    }
                }
            }
            // A line's music alone needs at least `content`, and starting earlier only
            // adds music, so once it exceeds the width every earlier start is overfull too.
            // Overfull lines of several measures are refused unless they end at a forced
            // break, so only then must the search go on.
            if breaks[first].forced || (content > width && !breaks[last].forced) {
                break;
            }
        }
    }
    let mut result = Vec::new();
    let mut last = breaks.len() - 1;
    while last != 0 {
        let line = chosen[last]
            .take()
            .ok_or_else(|| failure("No permitted breaking fits the measure window."))?;
        last = line.first;
        if result.len() >= limits.systems {
            return Err(failure("The score exceeds the system-count limit."));
        }
        result.push(line);
    }
    result.reverse();
    Ok(result)
}

fn choice_buffers(count: usize) -> Result<(Vec<f64>, Vec<Option<Line>>), Error> {
    let (mut costs, mut chosen) = (Vec::new(), Vec::new());
    costs.try_reserve_exact(count).map_err(|_| failure("Cannot reserve line costs."))?;
    chosen.try_reserve_exact(count).map_err(|_| failure("Cannot reserve line choices."))?;
    costs.resize(count, f64::INFINITY);
    chosen.resize(count, None);
    Ok((costs, chosen))
}

/// Rods ordered by their first gap, so a line finds its own rods without scanning the
/// whole score for every candidate.
pub(crate) struct RodIndex<'r> {
    rods: &'r [Rod],
    order: Vec<usize>,
    _held: crate::scratch::Reservation<'r>,
}

impl<'r> RodIndex<'r> {
    /// Sorts the rods once; the reservation lasts as long as the index.
    pub(crate) fn new(
        rods: &'r [Rod],
        work: &mut usize,
        scratch: crate::Scratch<'r>,
    ) -> Result<Self, Error> {
        charge(work, rods.len().saturating_mul(rods.len().max(2).ilog2() as usize + 1))?;
        let held = scratch.hold(rods.len().saturating_mul(size_of::<usize>()))?;
        let mut order = Vec::new();
        order.try_reserve_exact(rods.len()).map_err(|_| failure("Cannot reserve rod order."))?;
        order.extend(0..rods.len());
        order.sort_by_key(|&index| (rods[index].first, rods[index].last, index));
        Ok(Self { rods, order, _held: held })
    }

    /// The rods that lie wholly within gaps `start..end`.
    fn within(&self, start: usize, end: usize) -> impl Iterator<Item = &'r Rod> + '_ {
        let from = self.order.partition_point(|&index| self.rods[index].first < start);
        self.order[from..]
            .iter()
            .map(|&index| &self.rods[index])
            .take_while(move |rod| rod.first < end)
            .filter(move |rod| rod.last <= end)
    }
}

/// Lays out one candidate line; also returns the least width its music needs without the
/// prefatory and courtesy matter.
pub(crate) fn candidate(
    springs: &[Spring],
    rods: &RodIndex<'_>,
    breaks: &[BreakPoint],
    settings: (f64, Justify),
    range: std::ops::Range<usize>,
    work: &mut usize,
    scratch: crate::Scratch<'_>,
) -> Result<(Line, f64), Error> {
    let (width, justify) = settings;
    let (first, last) = (range.start, range.end);
    let (start, end) = (breaks[first].gap, breaks[last].gap);
    let prefix = breaks[first].prefix + breaks[last].suffix;
    let count = rods.within(start, end).count();
    charge(work, count)?;
    let _local = scratch.hold(
        count
            .saturating_mul(size_of::<Rod>())
            .saturating_add((end - start).saturating_mul(size_of::<f64>())),
    )?;
    let mut local = Vec::new();
    local.try_reserve_exact(count).map_err(|_| failure("Cannot reserve line rods."))?;
    for rod in rods.within(start, end) {
        local.push(Rod { first: rod.first - start, last: rod.last - start, minimum: rod.minimum });
    }
    let solution = solve(&springs[start..end], &local, (width - prefix).max(0.01), work, scratch)?;
    let ragged = justify == Justify::None
        || (justify == Justify::Auto
            && last + 1 == breaks.len()
            && solution.natural + prefix <= width);
    let stretch = if ragged { solution.stretch.min(1.0) } else { solution.stretch };
    let actual = if ragged {
        solution.natural.min(width - prefix).max(solution.minimum)
    } else {
        solution.gaps.iter().sum()
    };
    let line = Line {
        first,
        last,
        stretch,
        width: actual + prefix,
        overfull: solution.overfull || prefix >= width,
    };
    Ok((line, solution.minimum))
}

fn badness(stretch: f64) -> f64 {
    if stretch >= 1.0 {
        100.0 * (stretch - 1.0).powi(3)
    } else if stretch > 0.0 {
        200.0 * (1.0 / stretch - 1.0).powi(3)
    } else {
        10_000.0
    }
}
