//! Bounded endpoint search against existing ink, independent of any input syntax.

use gd_score::Error;

use crate::curves::bow;
use crate::limits::charge;
use crate::{Bezier, Point, PositionedMark, Role};

const END_WEIGHT: f64 = 0.05;
const SLOPE_WEIGHT: f64 = 0.5;
const HEIGHT_WEIGHT: f64 = 4.0;
const VARIANCE_WEIGHT: f64 = 0.02;
const TIE_WEIGHT: f64 = 8.0;

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
            obstacles.push((mark.bounds, mark.mark.role()));
        }
    }
    let mut best = None;
    for left in 0..7 {
        for right in 0..7 {
            charge(work, 1)?;
            let mut pair = candidate(request, left, right);
            let (height, variance, ties) = clearance(request, pair.0, &obstacles, work)?;
            raise(&mut pair, height, request.up);
            if request.tie {
                avoid_line(&mut pair, request);
            }
            let score = END_WEIGHT * f64::from(left * left + right * right)
                + SLOPE_WEIGHT * f64::from((left - right).pow(2))
                + HEIGHT_WEIGHT * (height / request.space).powi(2)
                + VARIANCE_WEIGHT * variance
                + TIE_WEIGHT * ties;
            if best.as_ref().is_none_or(|(previous, _)| score < *previous) {
                best = Some((score, pair));
            }
        }
    }
    best.map(|(_, pair)| pair).ok_or_else(|| crate::limits::failure("No curve candidate."))
}

fn clearance(
    request: &Request<'_>,
    curve: Bezier,
    obstacles: &[(crate::Rect, Role)],
    work: &mut usize,
) -> Result<(f64, f64, f64), Error> {
    let (mut extra, mut sum, mut squares, mut count, mut ties) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    for sample in 1..=16 {
        let t = f64::from(sample) / 17.0;
        let point = curve.point(t);
        for (bounds, role) in obstacles {
            charge(work, 1)?;
            if point.x < bounds.left - 0.2 * request.space
                || point.x > bounds.right + 0.2 * request.space
            {
                continue;
            }
            let depth = if request.up { point.y - bounds.top } else { bounds.bottom - point.y };
            extra = extra.max((depth + 0.2 * request.space) / (3.0 * t * (1.0 - t)));
            if *role == Role::Notehead {
                let distance = depth / request.space;
                sum += distance;
                squares += distance * distance;
                count += 1.0;
            }
            if *role == Role::Tie && depth.abs() < request.space {
                ties += 1.0;
            }
        }
    }
    let variance = if count > 0.0 { (squares - sum * sum / count).max(0.0) / count } else { 0.0 };
    Ok((extra, variance, ties))
}

fn raise(pair: &mut (Bezier, Bezier), extra: f64, up: bool) {
    let delta = if up { -extra } else { extra };
    for curve in [&mut pair.0, &mut pair.1] {
        curve.first.y += delta;
        curve.second.y += delta;
    }
}

fn avoid_line(pair: &mut (Bezier, Bezier), request: &Request<'_>) {
    let apex = pair.0.point(0.5);
    let position = (apex.y - request.middle) / request.space;
    if position.abs() <= 2.1 && (position - position.round()).abs() < 0.15 {
        let target = position.round() + if request.up { -0.5 } else { 0.5 };
        raise(pair, (target - position).abs() * request.space / 0.75, request.up);
    }
}

fn candidate(request: &Request<'_>, left: i32, right: i32) -> (Bezier, Bezier) {
    let side = if request.up { -1.0 } else { 1.0 };
    let step = if request.tie { 0.15 } else { 0.25 } * request.space;
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
