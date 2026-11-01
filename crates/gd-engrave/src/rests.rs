use gd_score::{Base, Direction, Duration, Error, Event, EventKind};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Rect, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn rest(
        &mut self,
        event: &Event,
        duration: &Duration,
        request: &Request<'_, '_>,
        column: usize,
        middle: f64,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let name = crate::drawing::rest_glyph(duration.base);
        let glyph = self.fonts.glyph(name)?;
        let data = self
            .fonts
            .glyph_data(glyph)
            .ok_or_else(|| crate::limits::failure("Missing rest bounds."))?;
        let other_rest = request.columns[column].events.iter().find_map(|id| {
            let other = &request.score.events[id.0 as usize];
            (other.voice != event.voice
                && request.score.voices[other.voice.0 as usize].staff == self.staff
                && matches!(other.kind, EventKind::Rest(rest) if rest == *duration))
            .then_some(other.voice)
        });
        if other_rest.is_some_and(|voice| voice.0 < event.voice.0) {
            return Ok(());
        }
        let direction = if other_rest.is_some() {
            Direction::Auto
        } else {
            request.score.voices[event.voice.0 as usize].direction
        };
        let mut y = middle - if duration.base == Base::Whole { space } else { 0.0 };
        let side = if direction == Direction::Down { 1.0 } else { -1.0 };
        if direction != Direction::Auto {
            y += side * 2.0 * space;
        }
        let x = request.positions[column];
        for _ in 0..8 {
            let ink = Rect {
                left: x + data.bounds.left * space,
                right: x + data.bounds.right * space,
                top: y + data.bounds.top * space,
                bottom: y + data.bounds.bottom * space,
            };
            let mut collision = false;
            for id in &request.columns[column].events {
                charge(self.work, 1)?;
                let other = &request.score.events[id.0 as usize];
                if other.voice != event.voice && matches!(other.kind, EventKind::Chord(_)) {
                    let info = &request.plan.events[id.0 as usize];
                    if info.staff == self.staff.0 as usize {
                        let bounds = Rect {
                            left: x + info.bounds.left * space,
                            right: x + info.bounds.right * space,
                            top: middle + info.bounds.top * space - 0.5 * space,
                            bottom: middle + info.bounds.bottom * space + 0.5 * space,
                        };
                        collision |= ink.overlaps(bounds, 0.0);
                    }
                }
            }
            if !collision {
                break;
            }
            y += side * space;
        }
        self.glyph(name, x, y, space, Role::Rest)?;
        for dot in 0..duration.dots {
            self.glyph(
                "augmentationDot",
                x + (1.7 + f64::from(dot) * 0.6) * space,
                y - 0.5 * space,
                space,
                Role::Dot,
            )?;
        }
        Ok(())
    }
}
