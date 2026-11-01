//! Non-head ink participates in the same per-band rods as pitched columns.

use gd_score::{Direction, Error, Event, EventKind, Score};

use crate::prepare::EventInfo;
use crate::rods::{Bands, add_glyph, add_rect};
use crate::{Fonts, Rect};

pub(crate) fn event(
    bands: &mut Bands,
    info: &EventInfo,
    event: &Event,
    score: &Score,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    if let EventKind::Rest(duration) = event.kind {
        let direction = score.voices[event.voice.0 as usize].direction;
        let y = match direction {
            Direction::Up => -2.0,
            Direction::Down => 2.0,
            Direction::Auto => 0.0,
        } - if duration.base == gd_score::Base::Whole { 1.0 } else { 0.0 };
        add_glyph(
            bands,
            fonts,
            crate::drawing::rest_glyph(duration.base),
            1.0,
            info.prefix,
            y,
            work,
        )?;
        for dot in 0..duration.dots {
            add_glyph(
                bands,
                fonts,
                "augmentationDot",
                1.0,
                info.prefix + 1.7 + f64::from(dot) * 0.6,
                y - 0.5,
                work,
            )?;
        }
    } else if matches!(event.kind, EventKind::MeasureRest(_))
        && crate::multirests::span(score, event.at.main, work)?.is_none()
    {
        add_rect(
            bands,
            Rect { left: info.prefix, right: info.prefix + 2.4, top: -1.5, bottom: 0.5 },
            work,
        )?;
    }
    for head in &info.heads {
        let position = (-head.at.y * 2.0).round() as i16;
        for ledger in (6..=position.abs()).step_by(2) {
            let y = -f64::from(ledger * position.signum()) / 2.0;
            add_rect(
                bands,
                Rect {
                    left: info.prefix + head.at.x - 0.4 * info.head_scale,
                    right: info.prefix + head.at.x + 1.6 * info.head_scale,
                    top: y - 0.08,
                    bottom: y + 0.08,
                },
                work,
            )?;
        }
    }
    Ok(())
}
