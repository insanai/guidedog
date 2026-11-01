use gd_score::{Direction, Error, EventKind, NoteRef, Score, Spanner, SpannerKind, TextStyle};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Bezier, Mark, Point, Rect, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn spanners(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let score = request.score;
        let start = request.columns[0].at;
        let end = request.columns.last().map(|c| c.at).unwrap_or(start);
        let space = self.options.staff_size / 4.0;
        let mut order: Vec<usize> = (0..score.spanners.len()).collect();
        order.sort_by(|&ia, &ib| spanner_order(score, &score.spanners[ia], &score.spanners[ib]));
        for spanner_idx in order {
            charge(self.work, 1)?;
            let spanner = &score.spanners[spanner_idx];
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
                    self.phrase(request, spanner, first, last, middle)?;
                }
                SpannerKind::Crescendo | SpannerKind::Decrescendo => {
                    self.hairpin(request, spanner, (first, last), middle)?;
                }
                SpannerKind::Beam => {}
                SpannerKind::Tuplet(number, _) => {
                    self.tuplet(request, spanner, *number, (first, last), middle)?;
                }
                SpannerKind::Volta(label) => {
                    self.volta_span(request, spanner, label, first, last, middle)?;
                }
            }
        }
        Ok(())
    }

    fn volta_span(
        &mut self,
        request: &Request<'_, '_>,
        spanner: &Spanner,
        label: &str,
        first: Point,
        last: Point,
        middle: f64,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let start = request.columns[0].at;
        let end = request.columns.last().map(|c| c.at).unwrap_or(start);
        let a = &request.score.events[spanner.start.event.0 as usize];
        let b = &request.score.events[spanner.end.event.0 as usize];
        let finish = b.at.main.checked_add(
            b.kind.duration().map(|d| d.time()).transpose()?.unwrap_or(gd_score::Ratio::ZERO),
        )?;
        let open_left = a.at >= start;
        let close_right = finish <= end.main;
        let left_x = if open_left {
            let a_idx = request.columns.partition_point(|c| c.at.main < a.at.main);
            if a_idx == 0 {
                request.preface_end + 0.15 * space
            } else {
                request.positions.get(a_idx).copied().unwrap_or(first.x) + 0.15 * space
            }
        } else {
            request.preface_end + 0.2 * space
        };
        let right_x = if close_right {
            let index = request.columns.partition_point(|column| column.at.main < finish);
            let bar_x = request
                .positions
                .get(index)
                .copied()
                .unwrap_or_else(|| request.positions.last().copied().unwrap_or(last.x));
            (bar_x - 0.15 * space).max(left_x + 1.2 * space)
        } else {
            request.staff_right.max(left_x + 1.2 * space)
        };
        self.volta(label, (left_x, right_x), (open_left, close_right), middle, space)
    }

    fn volta(
        &mut self,
        label: &str,
        span_x: (f64, f64),
        hooks: (bool, bool),
        middle: f64,
        space: f64,
    ) -> Result<(), Error> {
        let (left_x, right_x) = span_x;
        let (open_left, close_right) = hooks;
        let y = middle - 7.0 * space;
        if open_left {
            self.line(
                Point { x: left_x, y: y + 1.5 * space },
                Point { x: left_x, y },
                0.16 * space,
                Role::Volta,
            )?;
        }
        self.line(Point { x: left_x, y }, Point { x: right_x, y }, 0.16 * space, Role::Volta)?;
        if close_right {
            self.line(
                Point { x: right_x, y },
                Point { x: right_x, y: y + 1.5 * space },
                0.16 * space,
                Role::Volta,
            )?;
        }
        if open_left && !label.is_empty() {
            let has_text_face = !self.fonts.regular.is_empty() || !self.fonts.bold.is_empty();
            if has_text_face || !label.chars().all(|c| c.is_ascii_digit()) {
                self.text(
                    label,
                    TextStyle::Bold,
                    Point { x: left_x + 0.45 * space, y: y + 1.75 * space },
                    1.9 * space,
                    Role::Volta,
                )?;
            } else {
                let digit_w = self.fonts.digits_width(label, "timeSig")? * space * 0.55;
                self.number(
                    label,
                    "timeSig",
                    Point { x: left_x + 0.45 * space + digit_w * 0.5, y: y + 1.65 * space },
                    space * 0.55,
                    Role::Volta,
                )?;
            }
        }
        Ok(())
    }

    fn phrase(
        &mut self,
        context: &Request<'_, '_>,
        spanner: &Spanner,
        mut first: Point,
        mut last: Point,
        middle: f64,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let sys_start = context.columns[0].at;
        let sys_end = context.columns.last().map(|c| c.at).unwrap_or(sys_start);
        let before = context.score.events[spanner.start.event.0 as usize].at < sys_start;
        let after = context.score.events[spanner.end.event.0 as usize].at >= sys_end;
        let tie = spanner.kind == SpannerKind::Tie;
        let info = &context.plan.events[spanner.start.event.0 as usize];
        let up = phrase_up(spanner, info);
        if !tie {
            if !before {
                first = self.slur_attachment(context, spanner.start, first, up);
            }
            if !after {
                last = self.slur_attachment(context, spanner.end, last, up);
            }
        }
        let offset = if up { -0.65 * space } else { 0.65 * space };
        let mut from =
            Point { x: if before { first.x } else { first.x + 1.1 * space }, y: first.y + offset };
        let mut to = Point { x: last.x - 0.1 * space, y: last.y + offset };
        if tie && !before && up && info.up {
            from.x = from.x.max(stem_edge(context, spanner.start, first.x, space) + 0.2 * space);
        }
        if tie && !after && !up && !context.plan.events[spanner.end.event.0 as usize].up {
            to.x = to.x.min(stem_edge(context, spanner.end, last.x, space) - 0.2 * space);
        }
        if before && tie {
            to.x = continued_tie_end(context, spanner, last.x, from.x, space).min(to.x);
        }
        if after && to.x < from.x + 0.8 * space {
            to.x = (from.x + 0.8 * space).min(context.staff_right - 0.08 * space);
        }
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
            // In polyphony other voices' notes crowd a curve's ends.
            polyphonic: context.score.events[spanner.start.event.0 as usize]
                .voice_style
                .is_some_and(|style| style != gd_score::VoiceStyle::Single),
        };
        let (outer, inner) = crate::bows::choose(&request, self.work)?;
        let bounds = curve_bounds(outer).union(curve_bounds(inner));
        self.add(
            Mark::Curve { outer, inner, role: if tie { Role::Tie } else { Role::Slur } },
            bounds,
        )
    }

    fn slur_attachment(
        &self,
        context: &Request<'_, '_>,
        note: NoteRef,
        point: Point,
        up: bool,
    ) -> Point {
        let info = &context.plan.events[note.event.0 as usize];
        if info.up == up
            && let Some(index) = self.stem_marks.get(&note.event.0)
            && let Mark::Line { to, .. } = self.system.marks[*index].mark
        {
            return to;
        }
        point
    }

    pub(crate) fn number(
        &mut self,
        text: &str,
        prefix: &str,
        at: Point,
        size: f64,
        role: Role,
    ) -> Result<(), Error> {
        let (items, width) = self.fonts.number_layout(text, prefix)?;
        let left = at.x - width * size * 0.5;
        let start = self.system.marks.len();
        for (glyph, gx) in items {
            let data = self
                .fonts
                .glyph_data(glyph)
                .ok_or_else(|| crate::limits::failure("Missing digit glyph geometry."))?;
            let x = left + gx * size;
            let bounds = Rect {
                left: x + data.bounds.left * size,
                right: x + data.bounds.right * size,
                top: at.y + data.bounds.top * size,
                bottom: at.y + data.bounds.bottom * size,
            };
            self.add(Mark::Glyph { glyph, x, y: at.y, size, role }, bounds)?;
        }
        self.end_sign(start)
    }
}

fn spanner_order(score: &Score, a: &Spanner, b: &Spanner) -> std::cmp::Ordering {
    let rank = |kind: &SpannerKind| match kind {
        SpannerKind::Tie => 0u8,
        SpannerKind::Slur => 1,
        SpannerKind::PhrasingSlur => 2,
        SpannerKind::Tuplet(_, _) => 3,
        SpannerKind::Crescendo | SpannerKind::Decrescendo => 4,
        SpannerKind::Volta(_) => 5,
        SpannerKind::Beam => 6,
    };
    let span_len = |s: &Spanner| {
        let start = score.events[s.start.event.0 as usize].at;
        let end = score.events[s.end.event.0 as usize].at;
        end.main.checked_sub(start.main).unwrap_or_default()
    };
    rank(&a.kind).cmp(&rank(&b.kind)).then_with(|| span_len(a).cmp(&span_len(b)))
}

/// Where a tie continued on a new system ends: before the accidental its note restates.
fn continued_tie_end(
    context: &Request<'_, '_>,
    spanner: &Spanner,
    head_x: f64,
    from_x: f64,
    space: f64,
) -> f64 {
    let end = &context.plan.events[spanner.end.event.0 as usize];
    match end.heads.get(spanner.end.note as usize) {
        Some(head) if head.accidental.is_some() => {
            let accidental_left = head_x + (head.accidental_x - head.at.x) * space;
            (accidental_left - 0.2 * space).max(from_x + 0.5 * space)
        }
        _ => f64::INFINITY,
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
    let mut x = request
        .positions
        .get(index)
        .copied()
        .unwrap_or_else(|| request.positions.last().copied().unwrap_or(0.0));
    let info = &request.plan.events[reference.event.0 as usize];
    let head = info.heads.get(reference.note as usize);
    let local = head.map(|head| head.at).unwrap_or_default();
    let first = request.columns[0].at;
    let last = request.columns.last().map(|c| c.at).unwrap_or(first);
    let before = event.at < first;
    let after = event.at >= last;
    let y = if before || after {
        let clef = if before { request.clefs[info.staff].0 } else { request.clefs[info.staff].1 };
        if let EventKind::Chord(chord) = &event.kind {
            middle
                - f64::from(chord.notes[reference.note as usize].pitch.diatonic() - clef.middle())
                    * space
                    / 2.0
        } else {
            middle
        }
    } else {
        middle + local.y * space
    };
    if before {
        x = request.preface_end + 0.35 * space;
    } else if after {
        x = request.positions.last().copied().unwrap_or(x) - 0.25 * space;
    } else {
        let prefix = if event.at.main == first.main { 0.0 } else { info.prefix };
        x += (prefix + local.x) * space;
    }
    Point { x, y }
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

/// Keeps a same-side tie endpoint beyond the note's actual stem edge.
fn stem_edge(context: &Request<'_, '_>, note: NoteRef, head_x: f64, space: f64) -> f64 {
    let info = &context.plan.events[note.event.0 as usize];
    let local_x = info.heads[note.note as usize].at.x;
    head_x + (info.stem_x - local_x) * space
}

fn phrase_up(spanner: &Spanner, info: &crate::prepare::EventInfo) -> bool {
    match spanner.direction {
        Direction::Up => true,
        Direction::Down => false,
        Direction::Auto if spanner.kind != SpannerKind::Tie => !info.up,
        Direction::Auto => {
            let mut heights: Vec<_> = info.heads.iter().map(|h| h.at.y).collect();
            heights.sort_by(f64::total_cmp);
            let y = info.heads[spanner.start.note as usize].at.y;
            let center = heights[heights.len() / 2];
            if y == center && heights.len() % 2 == 1 { !info.up } else { y < center }
        }
    }
}
