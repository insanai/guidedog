use gd_score::{Error, EventId};

use crate::drawing::{Draw, Request};
use crate::limits::{charge, failure};
use crate::{Mark, Point, Rect, Role};

const STEM_WEIGHT: f64 = 10.0;
const SLOPE_WEIGHT: f64 = 25.0;
const SHORT_STEM_WEIGHT: f64 = 100_000.0;
const COLLISION_WEIGHT: f64 = 1_000_000.0;

#[derive(Clone, Copy, Debug)]
struct Stem {
    id: EventId,
    x: f64,
    base: f64,
    ideal: f64,
    up: bool,
    flags: u8,
    scale: f64,
}

impl Draw<'_, '_, '_, '_> {
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
                });
            }
            if stems.len() < 2 {
                continue;
            }
            let middle = (4.0 + f64::from(self.staff.0) * 9.0) * space;
            let thickness = self.fonts.default_value("beamThickness", 0.5) * space;
            let (left, right) = choose(&stems, group.up, middle, space, thickness, self.work)?;
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
            self.beam_segments(&stems, group.up, (left, right), thickness, space)?;
        }
        Ok(())
    }

    fn beam_segments(
        &mut self,
        stems: &[Stem],
        up: bool,
        ends: (f64, f64),
        thickness: f64,
        space: f64,
    ) -> Result<(), Error> {
        let first = stems[0].x;
        let last = stems.last().map(|s| s.x).unwrap_or(first);
        let width = (last - first).max(0.01);
        let side = if up { 1.0 } else { -1.0 };
        let distance = thickness + self.fonts.default_value("beamSpacing", 0.25) * space;
        let at = |x: f64| ends.0 + (ends.1 - ends.0) * (x - first) / width;
        let levels = stems.iter().map(|s| s.flags).max().unwrap_or(0);
        for level in 0..levels {
            for (index, stem) in stems.iter().enumerate() {
                if stem.flags <= level {
                    continue;
                }
                let next = stems.get(index + 1).filter(|next| next.flags > level);
                let previous = index
                    .checked_sub(1)
                    .and_then(|i| stems.get(i))
                    .filter(|prev| prev.flags > level);
                let (a, b) = if let Some(next) = next {
                    (stem.x, next.x)
                } else if previous.is_some() {
                    continue;
                } else {
                    let right = stems.get(index + 1).map(|s| s.x - stem.x);
                    let left = index.checked_sub(1).map(|i| stem.x - stems[i].x);
                    if right.is_some() && (left.is_none() || index == 0) {
                        (stem.x, stem.x + right.unwrap_or(space).min(1.25 * space))
                    } else {
                        (stem.x - left.unwrap_or(space).min(1.25 * space), stem.x)
                    }
                };
                let offset = side * f64::from(level) * distance;
                let points = [
                    Point { x: a, y: at(a) + offset },
                    Point { x: b, y: at(b) + offset },
                    Point { x: b, y: at(b) + offset + side * thickness },
                    Point { x: a, y: at(a) + offset + side * thickness },
                ];
                let bounds = Rect {
                    left: a,
                    right: b,
                    top: points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
                    bottom: points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
                };
                self.add(Mark::Polygon { points, role: Role::Beam }, bounds)?;
            }
        }
        Ok(())
    }
}

fn choose(
    stems: &[Stem],
    up: bool,
    middle: f64,
    space: f64,
    thickness: f64,
    work: &mut usize,
) -> Result<(f64, f64), Error> {
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
    if concave {
        delta = 0.0;
    }
    let ideal_left = ((first.ideal + last.ideal - delta) / (0.5 * space)).round() * 0.25 * space;
    let ideal_right = ideal_left + delta;
    let mut best = None;
    for a in -8..=8 {
        for b in -8..=8 {
            charge(work, stems.len())?;
            let left = ideal_left + f64::from(a) * 0.25 * space;
            let right = ideal_right + f64::from(b) * 0.25 * space;
            if forbidden(left, up, middle, space, thickness)
                || forbidden(right, up, middle, space, thickness)
            {
                continue;
            }
            let slope = (right - left) / width;
            let mut cost = SLOPE_WEIGHT * (slope - delta / width).powi(2);
            for stem in stems {
                let y = left + slope * (stem.x - first.x);
                let length = if stem.up { stem.base - y } else { y - stem.base };
                let natural = (stem.base - stem.ideal).abs().max(space);
                cost += STEM_WEIGHT * ((length - natural) / natural).powi(2);
                if length < 2.5 * space * stem.scale {
                    cost += SHORT_STEM_WEIGHT;
                }
                if length <= 0.75 * space {
                    cost += COLLISION_WEIGHT;
                }
            }
            let tie = (slope.abs(), left + right);
            if best.is_none_or(|(old, old_tie, _, _)| cost < old || (cost == old && tie < old_tie))
            {
                best = Some((cost, tie, left, right));
            }
        }
    }
    best.map(|(_, _, a, b)| (a, b)).ok_or_else(|| failure("No valid beam quantization was found."))
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
