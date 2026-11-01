use gd_score::{Direction, Error, NoteRef, Score, SpannerKind};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Bezier, Mark, Point, Rect, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn spanners(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let score = request.score;
        let start = request.columns[0].at;
        let end = request.columns.last().map(|c| c.at).unwrap_or(start);
        let space = self.options.staff_size / 4.0;
        for spanner in &score.spanners {
            charge(self.work, 1)?;
            let a = &score.events[spanner.start.event.0 as usize];
            let b = &score.events[spanner.end.event.0 as usize];
            if a.at >= end || b.at < start {
                continue;
            }
            self.staff = score.voices[a.voice.0 as usize].staff;
            let middle = (4.0 + f64::from(self.staff.0) * 9.0) * space;

            let first = endpoint(score, spanner.start, request, middle, space);
            let last = endpoint(score, spanner.end, request, middle, space);
            self.measure = request.first;
            match &spanner.kind {
                SpannerKind::Tie | SpannerKind::Slur | SpannerKind::PhrasingSlur => {
                    self.phrase(spanner, first, last, middle, space)?;
                }
                SpannerKind::Crescendo | SpannerKind::Decrescendo => {
                    let y = middle + 5.0 * space;
                    let opening = 0.65 * space;
                    let (left, right) = if spanner.kind == SpannerKind::Crescendo {
                        (0.0, opening)
                    } else {
                        (opening, 0.0)
                    };
                    for side in [-1.0, 1.0] {
                        self.line(
                            Point { x: first.x, y: y + side * left },
                            Point { x: last.x, y: y + side * right },
                            0.16 * space,
                            Role::Dynamic,
                        )?;
                    }
                }
                SpannerKind::Beam => {}
                SpannerKind::Tuplet(number, _) => {
                    self.number(
                        &number.to_string(),
                        "tuplet",
                        Point { x: (first.x + last.x) / 2.0, y: middle - 5.0 * space },
                        space,
                        Role::Tuplet,
                    )?;
                }
                SpannerKind::Volta(label) => self.volta(label, first, last, middle, space)?,
            }
        }
        Ok(())
    }

    fn volta(
        &mut self,
        label: &str,
        first: Point,
        last: Point,
        middle: f64,
        space: f64,
    ) -> Result<(), Error> {
        let y = middle - 7.0 * space;
        self.line(
            Point { x: first.x, y: y + space },
            Point { x: first.x, y },
            0.16 * space,
            Role::Volta,
        )?;
        self.line(
            Point { x: first.x, y },
            Point { x: last.x + 1.2 * space, y },
            0.16 * space,
            Role::Volta,
        )?;
        self.number(
            label,
            "timeSig",
            Point { x: first.x + space, y: y + space },
            space * 0.6,
            Role::Volta,
        )?;
        Ok(())
    }

    fn phrase(
        &mut self,
        spanner: &gd_score::Spanner,
        first: Point,
        last: Point,
        middle: f64,
        space: f64,
    ) -> Result<(), Error> {
        let tie = spanner.kind == SpannerKind::Tie;
        let up = match spanner.direction {
            Direction::Up => true,
            Direction::Down => false,
            Direction::Auto => first.y < middle,
        };
        let offset = if up { -0.65 * space } else { 0.65 * space };
        let from = Point { x: first.x + 1.1 * space, y: first.y + offset };
        let to = Point { x: last.x - 0.1 * space, y: last.y + offset };
        if to.x <= from.x {
            return Ok(());
        }

        let end_thickness = self.fonts.default_value(
            if tie { "tieEndpointThickness" } else { "slurEndpointThickness" },
            0.1,
        ) * space;
        let middle_thickness = self.fonts.default_value(
            if tie { "tieMidpointThickness" } else { "slurMidpointThickness" },
            0.2,
        ) * space;
        let request = crate::bows::Request {
            from,
            to,
            middle,
            space,
            up,
            tie,
            thickness: (end_thickness, middle_thickness),
            obstacles: &self.system.marks,
            staff: self.staff,
        };
        let (outer, inner) = crate::bows::choose(&request, self.work)?;
        let bounds = curve_bounds(outer).union(curve_bounds(inner));
        self.add(
            Mark::Curve { outer, inner, role: if tie { Role::Tie } else { Role::Slur } },
            bounds,
        )?;
        Ok(())
    }

    pub(crate) fn number(
        &mut self,
        text: &str,
        prefix: &str,
        at: Point,
        size: f64,
        role: Role,
    ) -> Result<(), Error> {
        let width = text.len() as f64 * size * 0.7;
        for (index, digit) in text.chars().enumerate() {
            self.glyph(
                &format!("{prefix}{digit}"),
                at.x - width / 2.0 + index as f64 * size * 0.7,
                at.y,
                size,
                role,
            )?;
        }
        Ok(())
    }
}

pub(crate) fn endpoint(
    score: &Score,
    reference: NoteRef,
    request: &Request<'_, '_>,
    middle: f64,
    space: f64,
) -> Point {
    let event = &score.events[reference.event.0 as usize];
    let index = request.columns.partition_point(|column| column.at < event.at);
    let x = request
        .positions
        .get(index)
        .copied()
        .unwrap_or_else(|| request.positions.last().copied().unwrap_or(0.0));
    let info = &request.plan.events[reference.event.0 as usize];
    let head = info.heads.get(reference.note as usize);
    let local = head.map(|head| head.at).unwrap_or_default();
    let y = middle + local.y * space;
    Point { x: x + (info.prefix + local.x) * space, y }
}

pub(crate) fn bow(
    from: Point,
    to: Point,
    limit: f64,
    ratio: f64,
    side: f64,
    thickness: (f64, f64),
) -> (Bezier, Bezier) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let width = (dx * dx + dy * dy).sqrt().max(0.001);
    let height = limit * 2.0 / std::f64::consts::PI
        * crate::math::atan(std::f64::consts::PI * ratio * width / (2.0 * limit));
    let f = 1.0 / 3.1;
    let g = 2.0 * limit / f;
    let inset = 2.0 * limit - f * g * g / (width + g);
    let control = |origin: Point, along: f64, across: f64| Point {
        x: origin.x + along * dx / width - across * dy / width,
        y: origin.y + along * dy / width + across * dx / width,
    };
    let outer = Bezier {
        from,
        to,
        first: control(from, inset, side * height),
        second: control(to, -inset, side * height),
    };
    let delta = (4.0 * thickness.1 - thickness.0) / 3.0;
    let inner = Bezier {
        from: control(from, 0.0, -side * thickness.0),
        to: control(to, 0.0, -side * thickness.0),
        first: control(from, inset, side * (height - delta)),
        second: control(to, -inset, side * (height - delta)),
    };
    (outer, inner)
}

fn curve_bounds(curve: Bezier) -> Rect {
    Rect {
        left: curve.from.x.min(curve.first.x).min(curve.second.x).min(curve.to.x),
        right: curve.from.x.max(curve.first.x).max(curve.second.x).max(curve.to.x),
        top: curve.from.y.min(curve.first.y).min(curve.second.y).min(curve.to.y),
        bottom: curve.from.y.max(curve.first.y).max(curve.second.y).max(curve.to.y),
    }
}
