//! Bounded endpoint search against existing ink, independent of any input syntax.

use gd_score::Error;

use crate::curves::bow;
use crate::limits::charge;
use crate::{Bezier, Point, PositionedMark, Rect, Role};

pub(crate) struct Request<'a> {
    pub(crate) from: Point,
    pub(crate) to: Point,
    pub(crate) middle: f64,
    pub(crate) space: f64,
    pub(crate) up: bool,
    pub(crate) tie: bool,
    pub(crate) thickness: (f64, f64),
    pub(crate) obstacles: &'a [PositionedMark],
    pub(crate) staff: gd_score::StaffId,
}

pub(crate) fn choose(request: &Request<'_>, work: &mut usize) -> Result<(Bezier, Bezier), Error> {
    let mut obstacles = Vec::new();
    for mark in request.obstacles {
        charge(work, 1)?;
        if mark.staff == request.staff
            && !matches!(
                mark.mark.role(),
                Role::Staff
                    | Role::Bar
                    | Role::Dynamic
                    | Role::Lyric
                    | Role::Script
                    | Role::Tempo
                    | Role::Rehearsal
                    | Role::Volta
            )
            && mark.bounds.left < request.to.x
            && mark.bounds.right > request.from.x
        {
            obstacles.try_reserve(1).map_err(|_| crate::limits::failure("Curve obstacles."))?;
            obstacles.push(mark.bounds);
        }
    }
    let mut best = None;
    for left in 0..7 {
        for right in 0..7 {
            charge(work, 1)?;
            let pair = candidate(request, left, right);
            let mut score = f64::from(left * left + right * right) * 0.05;
            for sample in 1..=16 {
                let point = pair.0.point(f64::from(sample) / 17.0);
                for obstacle in &obstacles {
                    charge(work, 1)?;
                    score += collision(point, *obstacle, request.space, request.up);
                }
            }
            if request.tie {
                let apex = pair.0.point(0.5);
                let position = (apex.y - request.middle) / request.space;
                if position.abs() <= 2.1 && (position - position.round()).abs() < 0.15 {
                    score += 8.0;
                }
            }
            if best.as_ref().is_none_or(|(previous, _)| score < *previous) {
                best = Some((score, pair));
            }
        }
    }
    best.map(|(_, pair)| pair).ok_or_else(|| crate::limits::failure("No curve candidate."))
}

fn candidate(request: &Request<'_>, left: i32, right: i32) -> (Bezier, Bezier) {
    let side = if request.up { -1.0 } else { 1.0 };
    let step = if request.tie { 0.15 } else { 0.5 } * request.space;
    let from = Point { y: request.from.y + side * f64::from(left) * step, ..request.from };
    let to = Point { y: request.to.y + side * f64::from(right) * step, ..request.to };
    bow(
        from,
        to,
        if request.tie { 1.0 } else { 2.0 } * request.space,
        if request.tie { 0.333 } else { 0.25 },
        side,
        request.thickness,
    )
}

fn collision(point: Point, bounds: Rect, space: f64, up: bool) -> f64 {
    let padding = 0.2 * space;
    if point.x < bounds.left - padding || point.x > bounds.right + padding {
        return 0.0;
    }
    let depth =
        if up { point.y - bounds.top + padding } else { bounds.bottom - point.y + padding };
    if depth > 0.0 { 1000.0 * (1.0 + depth / space).powi(2) } else { 0.0 }
}
