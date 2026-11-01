use gd_score::Error;

use crate::limits::charge;
use crate::skyline::{Segment, Skyline};
use crate::{Bezier, Mark, Point, PositionedMark, Role, System};

#[derive(Clone, Debug, Default)]
struct Facing {
    upper: Skyline,
    lower: Skyline,
}

pub(crate) fn place(
    system: &mut System,
    staves: usize,
    space: f64,
    work: &mut usize,
) -> Result<(), Error> {
    outside(system, staves, space, work)?;
    let mut envelopes = vec![Facing::default(); staves];
    for placed in &system.marks {
        charge(work, 1)?;
        let old_middle = (4.0 + f64::from(placed.staff.0) * 9.0) * space;
        add(&mut envelopes[placed.staff.0 as usize], placed, -old_middle, work)?;
    }
    let mut middles = vec![0.0; staves];
    let top = envelopes[0].upper.segments.iter().map(|s| s.first.min(s.last)).fold(0.0, f64::min);
    middles[0] = -top + space;
    for index in 1..staves {
        let minimum = envelopes[index - 1].lower.distance(&envelopes[index].upper, work)? + space;
        middles[index] = middles[index - 1] + minimum.max(9.0 * space);
    }
    let mut bottom: f64 = 0.0;
    for placed in &mut system.marks {
        charge(work, 1)?;
        let staff = placed.staff.0 as usize;
        let old_middle = (4.0 + staff as f64 * 9.0) * space;
        translate(placed, 0.0, middles[staff] - old_middle);
        bottom = bottom.max(placed.bounds.bottom);
    }
    system.height = bottom + space;
    Ok(())
}

fn outside(system: &mut System, staves: usize, space: f64, work: &mut usize) -> Result<(), Error> {
    let roles = [
        Role::Articulation,
        Role::Fingering,
        Role::Tuplet,
        Role::Dynamic,
        Role::Script,
        Role::ChordName,
        Role::Tempo,
        Role::Rehearsal,
        Role::Volta,
        Role::Lyric,
        Role::MeasureNumber,
    ];
    for staff in 0..staves {
        let mut occupied = Facing::default();
        for mark in &system.marks {
            charge(work, 1)?;
            if mark.staff.0 as usize == staff && !roles.contains(&mark.mark.role()) {
                add(&mut occupied, mark, 0.0, work)?;
            }
        }
        let middle = (4.0 + staff as f64 * 9.0) * space;
        for role in roles {
            let local = matches!(
                role,
                Role::Articulation
                    | Role::Fingering
                    | Role::Tuplet
                    | Role::Script
                    | Role::ChordName
            );
            for up in [true, false] {
                place_role(system, &mut occupied, (staff, role, up, local), middle, space, work)?;
            }
        }
    }
    Ok(())
}

fn place_role(
    system: &mut System,
    occupied: &mut Facing,
    spec: (usize, Role, bool, bool),
    middle: f64,
    space: f64,
    work: &mut usize,
) -> Result<(), Error> {
    let (staff, role, up, local) = spec;
    let mut indices = Vec::new();
    for (index, mark) in system.marks.iter().enumerate() {
        charge(work, 1)?;
        let upper = (mark.bounds.top + mark.bounds.bottom) / 2.0 < middle;
        if mark.staff.0 as usize == staff && mark.mark.role() == role && upper == up {
            indices.push(index);
        }
    }
    if indices.is_empty() {
        return Ok(());
    }
    let clusters = if local {
        indices
            .sort_by(|&a, &b| system.marks[a].bounds.left.total_cmp(&system.marks[b].bounds.left));
        let mut groups: Vec<(Vec<usize>, f64)> = Vec::new();
        for idx in indices {
            let b = system.marks[idx].bounds;
            if let Some((last_group, right)) = groups.last_mut()
                && b.left <= *right + 0.4 * space
            {
                last_group.push(idx);
                *right = right.max(b.right);
            } else {
                groups.push((vec![idx], b.right));
            }
        }
        groups.into_iter().map(|(g, _)| g).collect()
    } else {
        vec![indices]
    };
    for cluster in clusters {
        let mut group = Facing::default();
        for &index in &cluster {
            add(&mut group, &system.marks[index], 0.0, work)?;
        }
        let delta = if up {
            -group.lower.distance(&occupied.upper, work)? - 0.25 * space
        } else {
            occupied.lower.distance(&group.upper, work)? + 0.25 * space
        };
        let delta = if delta.is_finite() { delta } else { 0.0 };
        for index in cluster {
            let mark = &mut system.marks[index];
            translate(mark, 0.0, delta);
            add(occupied, mark, 0.0, work)?;
        }
    }
    Ok(())
}

fn add(
    facing: &mut Facing,
    mark: &PositionedMark,
    shift: f64,
    work: &mut usize,
) -> Result<(), Error> {
    match &mark.mark {
        Mark::Line { from, to, thickness, .. } if from.x != to.x => {
            let (from, to) = if from.x < to.x { (*from, *to) } else { (*to, *from) };
            facing.upper.add(
                Segment {
                    left: from.x,
                    right: to.x,
                    first: from.y + shift - thickness / 2.0,
                    last: to.y + shift - thickness / 2.0,
                },
                true,
                work,
            )?;
            facing.lower.add(
                Segment {
                    left: from.x,
                    right: to.x,
                    first: from.y + shift + thickness / 2.0,
                    last: to.y + shift + thickness / 2.0,
                },
                false,
                work,
            )
        }
        Mark::Polygon { points, .. } => {
            facing.upper.add(
                Segment {
                    left: points[0].x,
                    right: points[1].x,
                    first: points[0].y.min(points[3].y) + shift,
                    last: points[1].y.min(points[2].y) + shift,
                },
                true,
                work,
            )?;
            facing.lower.add(
                Segment {
                    left: points[0].x,
                    right: points[1].x,
                    first: points[0].y.max(points[3].y) + shift,
                    last: points[1].y.max(points[2].y) + shift,
                },
                false,
                work,
            )
        }
        Mark::Curve { outer, inner, .. } => add_curve(facing, *outer, *inner, shift, work),
        Mark::Glyph { .. } | Mark::Text { .. } | Mark::Line { .. } => {
            add_rect(facing, mark.bounds, shift, work)
        }
    }
}

fn add_rect(
    facing: &mut Facing,
    bounds: crate::Rect,
    shift: f64,
    work: &mut usize,
) -> Result<(), Error> {
    facing.upper.add(
        Segment {
            left: bounds.left,
            right: bounds.right,
            first: bounds.top + shift,
            last: bounds.top + shift,
        },
        true,
        work,
    )?;
    facing.lower.add(
        Segment {
            left: bounds.left,
            right: bounds.right,
            first: bounds.bottom + shift,
            last: bounds.bottom + shift,
        },
        false,
        work,
    )
}

fn add_curve(
    facing: &mut Facing,
    outer: Bezier,
    inner: Bezier,
    shift: f64,
    work: &mut usize,
) -> Result<(), Error> {
    for index in 0..16 {
        let a = f64::from(index) / 16.0;
        let b = f64::from(index + 1) / 16.0;
        let (oa, ob, ia, ib) = (outer.point(a), outer.point(b), inner.point(a), inner.point(b));
        facing.upper.add(
            Segment {
                left: oa.x.min(ia.x),
                right: ob.x.max(ib.x),
                first: oa.y.min(ia.y) + shift - 0.1,
                last: ob.y.min(ib.y) + shift - 0.1,
            },
            true,
            work,
        )?;
        facing.lower.add(
            Segment {
                left: oa.x.min(ia.x),
                right: ob.x.max(ib.x),
                first: oa.y.max(ia.y) + shift + 0.1,
                last: ob.y.max(ib.y) + shift + 0.1,
            },
            false,
            work,
        )?;
    }
    Ok(())
}

pub(crate) fn translate(placed: &mut PositionedMark, x: f64, y: f64) {
    let point = |p: &mut Point| {
        p.x += x;
        p.y += y;
    };
    let curve = |b: &mut Bezier| {
        point(&mut b.from);
        point(&mut b.first);
        point(&mut b.second);
        point(&mut b.to);
    };
    match &mut placed.mark {
        Mark::Glyph { x: px, y: py, .. } | Mark::Text { x: px, y: py, .. } => {
            *px += x;
            *py += y;
        }
        Mark::Line { from, to, .. } => {
            point(from);
            point(to);
        }
        Mark::Polygon { points, .. } => {
            for p in points {
                point(p);
            }
        }
        Mark::Curve { outer, inner, .. } => {
            curve(outer);
            curve(inner);
        }
    }
    placed.bounds = placed.bounds.translated(x, y);
}

/// Includes furniture added after staff placement in the final drawing bounds.
pub(crate) fn fit(system: &mut System, space: f64, work: &mut usize) -> Result<(), Error> {
    let top = system.marks.iter().map(|m| m.bounds.top).fold(space, f64::min);
    let shift = (space - top).max(0.0);
    let left = system.marks.iter().map(|m| m.bounds.left).fold(0.0, f64::min);
    let horizontal = (-left).max(0.0);
    let mut right: f64 = 0.0;
    let mut bottom: f64 = 0.0;
    for mark in &mut system.marks {
        charge(work, 1)?;
        translate(mark, horizontal, shift);
        right = right.max(mark.bounds.right);
        bottom = bottom.max(mark.bounds.bottom);
    }
    system.height = bottom + space;
    system.width = right;
    Ok(())
}
