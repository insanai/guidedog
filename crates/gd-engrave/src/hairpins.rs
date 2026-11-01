use gd_score::{AnnotationKind, Error, Moment, Spanner, SpannerKind};

use crate::drawing::{Draw, Request};
use crate::{Point, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn hairpin(
        &mut self,
        request: &Request<'_, '_>,
        spanner: &Spanner,
        endpoints: (Point, Point),
        middle: f64,
    ) -> Result<(), Error> {
        let a = &request.score.events[spanner.start.event.0 as usize];
        let b = &request.score.events[spanner.end.event.0 as usize];
        let start = request.columns[0].at;
        let end = request.columns.last().map(|c| c.at).unwrap_or(start);
        if b.at <= start || a.at == b.at {
            return Ok(());
        }
        let space = self.options.staff_size / 4.0;
        let (mut first, mut last) = endpoints;
        if a.at >= start {
            let a_base = base_event_x(request, spanner.start.event, start, space);
            let width = a
                .marks
                .iter()
                .filter_map(|m| match &m.kind {
                    AnnotationKind::Dynamic(text) => Some(text.as_str()),
                    _ => None,
                })
                .map(|text| crate::labels::dynamic_width(self.fonts, text))
                .try_fold(0.0f64, |width, next| next.map(|next| width.max(next)))?;
            first.x = if width > 0.0 {
                first.x.max(a_base) + (width + 0.45) * space
            } else {
                first.x + 0.35 * space
            };
        }
        if b.at < end {
            let b_base = base_event_x(request, spanner.end.event, start, space);
            let has_end_dyn = b.marks.iter().any(|m| matches!(m.kind, AnnotationKind::Dynamic(_)));
            last.x =
                if has_end_dyn { last.x.min(b_base) - 0.55 * space } else { last.x - 0.4 * space };
        }
        if last.x <= first.x {
            return Ok(());
        }
        let mut left = progress(start.max(a.at), a.at, b.at)?;
        let mut right = progress(end.min(b.at), a.at, b.at)?;
        if spanner.kind == SpannerKind::Decrescendo {
            left = 1.0 - left;
            right = 1.0 - right;
        }
        let above = spanner.direction == gd_score::Direction::Up;
        let y = middle + if above { -5.0 * space } else { 5.0 * space };
        for side in [-1.0, 1.0] {
            self.line(
                Point { x: first.x, y: y + side * left * 0.65 * space },
                Point { x: last.x, y: y + side * right * 0.65 * space },
                0.16 * space,
                Role::Dynamic,
            )?;
        }
        Ok(())
    }
}

fn base_event_x(
    request: &Request<'_, '_>,
    event_id: gd_score::EventId,
    start: Moment,
    space: f64,
) -> f64 {
    let event = &request.score.events[event_id.0 as usize];
    let idx = request.columns.partition_point(|c| c.at < event.at);
    let col_x = request.positions.get(idx).copied().unwrap_or(0.0);
    let prefix = if event.at.main == start.main {
        0.0
    } else {
        request.plan.events[event_id.0 as usize].prefix * space
    };
    col_x + prefix
}

fn progress(at: Moment, first: Moment, last: Moment) -> Result<f64, Error> {
    let (offset, length) = if first.main != last.main {
        (at.main.checked_sub(first.main)?, last.main.checked_sub(first.main)?)
    } else {
        (at.grace.checked_sub(first.grace)?, last.grace.checked_sub(first.grace)?)
    };
    Ok(offset.checked_div(length)?.to_f64().clamp(0.0, 1.0))
}
