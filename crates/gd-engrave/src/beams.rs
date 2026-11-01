use gd_score::{Error, EventId};

use crate::drawing::{Draw, Request};
use crate::limits::{charge, failure};
use crate::{Mark, Point, Rect, Role};

const STEM_WEIGHT: f64 = 10.0;
const SLOPE_WEIGHT: f64 = 25.0;
const SHORT_STEM_WEIGHT: f64 = 100_000.0;
const COLLISION_WEIGHT: f64 = 1_000_000.0;
const QUANT_WEIGHT: f64 = 5.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Stem {
    id: EventId,
    pub(crate) x: f64,
    base: f64,
    ideal: f64,
    up: bool,
    pub(crate) flags: u8,
    scale: f64,
    pub(crate) beat: (usize, i64, i64),
}

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn beams(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        for group in &request.plan.beams {
            let mut stems = Vec::new();
            for id in &group.events {
                charge(self.work, 1)?;
                let event = &request.score.events[id.0 as usize];
                let index = request.columns.partition_point(|c| c.at < event.at);
                if index + 1 >= request.columns.len() || request.columns[index].at != event.at {
                    continue;
                }
                let info = &request.plan.events[id.0 as usize];
                self.staff = gd_score::StaffId(info.staff as u32);
                let prefix = if event.at.main == request.columns[0].at.main {
                    0.0
                } else {
                    info.prefix * space
                };
                let middle = (4.0 + info.staff as f64 * 9.0) * space;
                stems.push(Stem {
                    id: *id,
                    x: request.positions[index] + prefix + info.stem_x * space,
                    base: middle + info.stem_base * space,
                    ideal: middle + info.stem_tip * space,
                    up: info.up,
                    flags: info.flags,
                    scale: info.head_scale,
                    beat: info.beat,
                });
            }
            if stems.len() < 2 {
                continue;
            }
            let middle = (4.0 + f64::from(self.staff.0) * 9.0) * space;
            let scale = stems.iter().map(|s| s.scale).fold(1.0, f64::min);
            let thickness = self.fonts.default_value("beamThickness", 0.5) * space * scale;
            let segments = crate::beam_segments::collect(&stems, space, self.work)?;
            let distance =
                thickness + self.fonts.default_value("beamSpacing", 0.25) * space * scale;
            let metrics = Metrics { up: group.up, middle, space, thickness, distance };
            let mut obstacles = Vec::new();
            for mark in &self.system.marks {
                charge(self.work, 1)?;
                if mark.staff == self.staff
                    && matches!(mark.mark.role(), Role::Notehead | Role::Accidental)
                {
                    obstacles.push(mark.bounds);
                }
            }
            let (left, right) = choose(&stems, &segments, metrics, &obstacles, self.work)?;
            let first = stems[0].x;
            let width = (stems.last().map(|s| s.x).unwrap_or(first) - first).max(0.01);
            let at = |x: f64| left + (right - left) * (x - first) / width;
            for stem in &stems {
                if let Some(index) = self.stem_marks.get(&stem.id.0).copied() {
                    let mark = &mut self.system.marks[index];
                    if let Mark::Line { to, from, thickness, .. } = &mut mark.mark {
                        to.y = at(stem.x);
                        mark.bounds = Rect {
                            left: stem.x - *thickness / 2.0,
                            right: stem.x + *thickness / 2.0,
                            top: from.y.min(to.y),
                            bottom: from.y.max(to.y),
                        };
                    }
                }
            }
            self.measure = request.first;
            self.beam_segments(&stems, &segments, (left, right), metrics)?;
        }
        Ok(())
    }

    fn beam_segments(
        &mut self,
        stems: &[Stem],
        segments: &[crate::beam_segments::Segment],
        ends: (f64, f64),
        metrics: Metrics,
    ) -> Result<(), Error> {
        let first = stems[0].x;
        let width = (stems.last().map(|s| s.x).unwrap_or(first) - first).max(0.01);
        let at = |x| ends.0 + (ends.1 - ends.0) * (x - first) / width;
        let side = if metrics.up { 1.0 } else { -1.0 };
        for segment in segments {
            let offset = side * f64::from(segment.level) * metrics.distance;
            let points = [
                Point { x: segment.left, y: at(segment.left) + offset },
                Point { x: segment.right, y: at(segment.right) + offset },
                Point {
                    x: segment.right,
                    y: at(segment.right) + offset + side * metrics.thickness,
                },
                Point { x: segment.left, y: at(segment.left) + offset + side * metrics.thickness },
            ];
            let bounds = Rect {
                left: segment.left,
                right: segment.right,
                top: points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
                bottom: points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
            };
            self.add(Mark::Polygon { points, role: Role::Beam }, bounds)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Metrics {
    up: bool,
    middle: f64,
    space: f64,
    thickness: f64,
    distance: f64,
}

fn choose(
    stems: &[Stem],
    segments: &[crate::beam_segments::Segment],
    metrics: Metrics,
    obstacles: &[Rect],
    work: &mut usize,
) -> Result<(f64, f64), Error> {
    let Metrics { up, middle, space, thickness, distance } = metrics;
    let first = stems[0];
    let last = *stems.last().ok_or_else(|| failure("An empty beam has no endpoints."))?;
    let width = (last.x - first.x).max(0.01);
    let mut delta = (last.ideal - first.ideal).clamp(-1.5 * space, 1.5 * space);
    let concave = stems.iter().any(|s| {
        if up {
            s.ideal < first.ideal.min(last.ideal)
        } else {
            s.ideal > first.ideal.max(last.ideal)
        }
    });
    let ideal_left = if concave {
        delta = 0.0;
        let extreme = if up {
            stems.iter().map(|s| s.ideal).fold(first.ideal, f64::min)
        } else {
            stems.iter().map(|s| s.ideal).fold(first.ideal, f64::max)
        };
        (extreme / (0.25 * space)).round() * 0.25 * space
    } else {
        ((first.ideal + last.ideal - delta) / (0.5 * space)).round() * 0.25 * space
    };
    let ideal_right = ideal_left + delta;
    let force_level = concave || (first.ideal - last.ideal).abs() < 0.01;
    let mut best = None;
    for a in -8..=8 {
        let b_range = if force_level { a..=a } else { -8..=8 };
        for b in b_range {
            charge(work, stems.len() * 6)?;
            let left = ideal_left + f64::from(a) * 0.25 * space;
            let right = ideal_right + f64::from(b) * 0.25 * space;
            if forbidden(left, up, middle, space, thickness)
                || forbidden(right, up, middle, space, thickness)
            {
                continue;
            }
            let side = if up { 1.0 } else { -1.0 };
            let slope = (right - left) / width;
            charge(work, segments.len() * 2)?;
            let inner_quants = segments
                .iter()
                .filter(|segment| segment.level > 0)
                .flat_map(|segment| {
                    [segment.left, segment.right].map(|x| {
                        left + slope * (x - first.x) + side * f64::from(segment.level) * distance
                    })
                })
                .filter(|y| forbidden(*y, up, middle, space, thickness))
                .count();
            let mut cost = SLOPE_WEIGHT * (slope - delta / width).powi(2)
                + QUANT_WEIGHT * inner_quants as f64
                + stem_cost(stems, first.x, left, slope, space);
            cost += ink_cost(segments, first.x, (left, slope), metrics, obstacles, work)?;
            let tie = (slope.abs(), left + right);
            if best.is_none_or(|(old, old_tie, _, _)| cost < old || (cost == old && tie < old_tie))
            {
                best = Some((cost, tie, left, right));
            }
        }
    }
    best.map(|(_, _, a, b)| (a, b)).ok_or_else(|| failure("No valid beam quantization was found."))
}

fn stem_cost(stems: &[Stem], first_x: f64, left: f64, slope: f64, space: f64) -> f64 {
    let mut cost = 0.0;
    for stem in stems {
        let y = left + slope * (stem.x - first_x);
        let length = if stem.up { stem.base - y } else { y - stem.base };
        let natural = (stem.base - stem.ideal).abs().max(space);
        cost += STEM_WEIGHT * ((length - natural) / natural).powi(2);
        let minimum = (2.5 + f64::from(stem.flags.saturating_sub(2)) * 0.5) * space * stem.scale;
        if length < minimum {
            cost += SHORT_STEM_WEIGHT;
        }
        if length <= 0.75 * space {
            cost += COLLISION_WEIGHT;
        }
    }
    cost
}

fn forbidden(y: f64, up: bool, middle: f64, space: f64, thickness: f64) -> bool {
    let top = if up { y } else { y - thickness };
    let bottom = top + thickness;
    if bottom < middle - 2.0 * space || top > middle + 2.0 * space {
        return false;
    }
    ![top, bottom, (top + bottom) / 2.0].iter().any(|v| {
        let line = (*v - middle) / space;
        (line - line.round()).abs() < 0.025
    })
}

fn ink_cost(
    segments: &[crate::beam_segments::Segment],
    origin: f64,
    line: (f64, f64),
    metrics: Metrics,
    obstacles: &[Rect],
    work: &mut usize,
) -> Result<f64, Error> {
    let side = if metrics.up { 1.0 } else { -1.0 };
    let mut cost = 0.0;
    for segment in segments {
        let at = |x| {
            line.0 + line.1 * (x - origin) + side * f64::from(segment.level) * metrics.distance
        };
        for obstacle in obstacles {
            charge(work, 1)?;
            let left = segment.left.max(obstacle.left);
            let right = segment.right.min(obstacle.right);
            if left >= right {
                continue;
            }
            let a = at(left);
            let b = at(right);
            let top = a.min(b) + if metrics.up { 0.0 } else { -metrics.thickness };
            let bottom = a.max(b) + if metrics.up { metrics.thickness } else { 0.0 };
            if top < obstacle.bottom && bottom > obstacle.top {
                cost += COLLISION_WEIGHT;
            }
        }
    }
    Ok(cost)
}
