use gd_score::{BarStyle, Error, EventKind, Moment, Ratio, Score};

use crate::columns::Column;
use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Point, Role};

pub(crate) fn style_extent(style: BarStyle) -> (f64, f64) {
    match style {
        BarStyle::Single => (-0.08, 0.08),
        BarStyle::Double => (-0.63, 0.08),
        BarStyle::Final => (-0.63, 0.25),
        BarStyle::Initial => (-0.25, 0.63),
        BarStyle::RepeatStart => (-0.25, 1.70),
        BarStyle::RepeatEnd => (-2.05, 0.25),
        BarStyle::RepeatBoth => (-2.05, 1.70),
    }
}

pub(crate) fn opening_bar(
    score: &Score,
    column: &Column,
    staff: usize,
    score_start: bool,
) -> Option<BarStyle> {
    let mut chosen = None;
    for id in &column.events {
        let event = &score.events[id.0 as usize];
        if score.voices[event.voice.0 as usize].staff.0 as usize != staff {
            continue;
        }
        if let EventKind::Bar(style) = event.kind {
            let mapped = match style {
                BarStyle::RepeatStart | BarStyle::Initial => Some(style),
                BarStyle::RepeatBoth => Some(BarStyle::RepeatStart),
                BarStyle::Double if score_start => Some(BarStyle::Double),
                _ => None,
            };
            if mapped.is_some() {
                chosen = mapped;
            }
        }
    }
    chosen
}

pub(crate) fn closing_style(style: BarStyle, score_end: bool) -> BarStyle {
    match style {
        BarStyle::RepeatBoth => BarStyle::RepeatEnd,
        BarStyle::RepeatStart | BarStyle::Initial if !score_end => BarStyle::Single,
        other => other,
    }
}

pub(crate) fn column_bar_style(
    score: &Score,
    columns: &[Column],
    index: usize,
    staff: usize,
    work: &mut usize,
) -> Result<Option<BarStyle>, Error> {
    if index == 0 {
        return Ok(None);
    }
    let column = &columns[index];
    if column.at.grace != Ratio::ZERO {
        return Ok(None);
    }
    let terminal = index + 1 == columns.len();
    let score_end = score.measures.last().is_some_and(|m| m.end == column.at.main);
    for id in &column.events {
        let event = &score.events[id.0 as usize];
        if score.voices[event.voice.0 as usize].staff.0 as usize == staff
            && let EventKind::Bar(style) = event.kind
        {
            return Ok(Some(if terminal { closing_style(style, score_end) } else { style }));
        }
    }
    let boundary = score.measures.partition_point(|m| m.end < column.at.main);
    if score.measures.get(boundary).is_none_or(|m| m.end != column.at.main) {
        return Ok(None);
    }
    if !terminal
        && let Some((first, last, _)) = crate::multirests::span(score, column.at.main, work)?
        && first < column.at.main
        && column.at.main < last
    {
        return Ok(None);
    }
    Ok(Some(BarStyle::Single))
}

/// How far the system's closing bar line reaches right of its column, in staff spaces.
pub(crate) fn closing_bar_right(
    request: &Request<'_, '_>,
    work: &mut usize,
) -> Result<f64, Error> {
    let last_idx = request.columns.len().saturating_sub(1);
    let mut right = 0.08f64;
    for staff in 0..request.score.staves.len() {
        if let Some(style) =
            column_bar_style(request.score, request.columns, last_idx, staff, work)?
        {
            right = right.max(style_extent(style).1);
        }
    }
    Ok(right)
}

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn bar(&mut self, style: BarStyle, x: f64, middle: f64) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let (thin, thick, gap) = (0.16 * space, 0.5 * space, 0.55 * space);
        let lines: &[(f64, f64)] = match style {
            BarStyle::Single => &[(0.0, thin)],
            BarStyle::Double => &[(-gap, thin), (0.0, thin)],
            BarStyle::Final | BarStyle::RepeatEnd => &[(-gap, thin), (0.0, thick)],
            BarStyle::Initial | BarStyle::RepeatStart => &[(0.0, thick), (gap, thin)],
            BarStyle::RepeatBoth => &[(-gap, thin), (0.0, thick), (gap, thin)],
        };
        for &(dx, thickness) in lines {
            self.line(
                Point { x: x + dx, y: middle - 2.0 * space },
                Point { x: x + dx, y: middle + 2.0 * space },
                thickness,
                Role::Bar,
            )?;
        }
        for (active, dx) in [
            (matches!(style, BarStyle::RepeatStart | BarStyle::RepeatBoth), 1.2),
            (matches!(style, BarStyle::RepeatEnd | BarStyle::RepeatBoth), -1.8),
        ] {
            if active {
                for dy in [-0.5, 0.5] {
                    self.glyph(
                        "repeatDot",
                        x + dx * space,
                        middle + dy * space,
                        space,
                        Role::Bar,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn measure_bars(
        &mut self,
        request: &Request<'_, '_>,
        middle: f64,
    ) -> Result<(), Error> {
        let staff = self.staff.0 as usize;
        for (index, column) in request.columns.iter().enumerate().skip(1) {
            charge(self.work, 1)?;
            let explicit = column.events.iter().any(|id| {
                let event = &request.score.events[id.0 as usize];
                request.score.voices[event.voice.0 as usize].staff == self.staff
                    && matches!(event.kind, EventKind::Bar(_))
            });
            if !explicit
                && let Some(style) =
                    column_bar_style(request.score, request.columns, index, staff, self.work)?
            {
                self.bar(style, request.positions[index], middle)?;
            }
        }
        Ok(())
    }

    pub(crate) fn measure_rests(
        &mut self,
        request: &Request<'_, '_>,
        middle: f64,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let start = request.columns[0].at.main;
        let end = request.columns.last().map(|c| c.at.main).unwrap_or(start);
        for event in &request.score.events {
            charge(self.work, 1)?;
            if request.score.voices[event.voice.0 as usize].staff != self.staff {
                continue;
            }
            let EventKind::MeasureRest(duration) = &event.kind else { continue };
            charge(self.work, request.score.events.len())?;
            let shared = request.score.events.iter().any(|other| {
                other.voice.0 < event.voice.0
                    && request.score.voices[other.voice.0 as usize].staff == self.staff
                    && other.at == event.at
                    && other.kind == event.kind
            });
            if shared {
                continue;
            }
            let finish = event.at.main.checked_add(duration.time()?)?;
            if finish <= start || event.at.main >= end {
                continue;
            }
            let first = event.at.main.max(start);
            let last = finish.min(end);
            // In polyphony a measure rest moves with its voice, as LilyPond's
            // MultiMeasureRest does, so it clears the other voices' notes.
            let direction = event
                .voice_style
                .map(|style| style.direction())
                .unwrap_or(request.score.voices[event.voice.0 as usize].direction);
            let middle = middle
                + match direction {
                    gd_score::Direction::Up => -2.0 * space,
                    gd_score::Direction::Down => 2.0 * space,
                    gd_score::Direction::Auto => 0.0,
                };
            let (left, right) =
                rest_span_bounds(request, self.staff.0 as usize, first, last, space, self.work)?;
            let count = request
                .score
                .measures
                .iter()
                .filter(|m| m.start >= first && m.end <= last)
                .count();
            if count <= 1 {
                let meter =
                    request.score.measures.iter().find(|m| m.start <= first && m.end >= last);
                let two = Ratio::ONE.checked_add(Ratio::ONE)?;
                let breve = meter
                    .is_some_and(|m| m.end.checked_sub(m.start).is_ok_and(|time| time >= two));
                let x = (left + right) / 2.0;
                self.glyph(
                    if breve { "restDoubleWhole" } else { "restWhole" },
                    x - 0.58 * space,
                    middle - space,
                    space,
                    Role::Rest,
                )?;
            } else {
                self.hbar(count, left, right, middle)?;
            }
        }
        Ok(())
    }

    fn hbar(&mut self, count: usize, left: f64, right: f64, middle: f64) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let x = (left + right) / 2.0;
        let half = ((right - left) * 0.25).clamp(0.8 * space, 4.0 * space);
        self.line(
            Point { x: x - half, y: middle },
            Point { x: x + half, y: middle },
            0.8 * space,
            Role::Rest,
        )?;
        for x in [x - half, x + half] {
            self.line(
                Point { x, y: middle - space },
                Point { x, y: middle + space },
                0.16 * space,
                Role::Rest,
            )?;
        }
        self.number(
            &count.to_string(),
            "timeSig",
            Point { x, y: middle - 3.0 * space },
            space * 0.75,
            Role::Rest,
        )?;
        Ok(())
    }
}

fn rest_span_bounds(
    request: &Request<'_, '_>,
    staff: usize,
    first: Ratio,
    last: Ratio,
    space: f64,
    work: &mut usize,
) -> Result<(f64, f64), Error> {
    let start = request.columns[0].at.main;
    let index_at = |time| {
        request.columns.partition_point(|c| c.at < Moment { main: time, grace: Ratio::ZERO })
    };
    let left = if first == start {
        request.preface_end
    } else {
        let idx = index_at(first);
        let base = request.positions.get(idx).copied().unwrap_or(0.0);
        let pad = column_bar_style(request.score, request.columns, idx, staff, work)?
            .map(|s| style_extent(s).1)
            .unwrap_or(0.0);
        base + pad * space
    };
    let end_idx = index_at(last).min(request.columns.len().saturating_sub(1));
    let right_base = request.positions.get(end_idx).copied().unwrap_or(left + 4.0 * space);
    let right_pad = column_bar_style(request.score, request.columns, end_idx, staff, work)?
        .map(|s| style_extent(s).0)
        .unwrap_or(0.0);
    Ok((left, (right_base + right_pad * space).max(left + space)))
}
