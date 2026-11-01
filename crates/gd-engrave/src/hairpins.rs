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
            let width = a
                .marks
                .iter()
                .filter_map(|m| match &m.kind {
                    AnnotationKind::Dynamic(text) => Some(text.as_str()),
                    _ => None,
                })
                .map(|text| crate::labels::dynamic_width(self.fonts, text))
                .try_fold(0.0f64, |width, next| next.map(|next| width.max(next)))?;
            first.x += (width + 0.5) * space;
        }
        if b.at < end {
            last.x -= 0.5 * space;
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
        let y = middle + 5.0 * space;
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

fn progress(at: Moment, first: Moment, last: Moment) -> Result<f64, Error> {
    let (offset, length) = if first.main != last.main {
        (at.main.checked_sub(first.main)?, last.main.checked_sub(first.main)?)
    } else {
        (at.grace.checked_sub(first.grace)?, last.grace.checked_sub(first.grace)?)
    };
    Ok(offset.checked_div(length)?.to_f64().clamp(0.0, 1.0))
}
