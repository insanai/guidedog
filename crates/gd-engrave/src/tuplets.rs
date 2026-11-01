//! Tuplet brackets disappear only when the complete interval is one beamed group.

use gd_score::{Direction, Error, Spanner};

use crate::drawing::{Draw, Request};
use crate::{Mark, Point, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn tuplet(
        &mut self,
        request: &Request<'_, '_>,
        spanner: &Spanner,
        number: u64,
        endpoints: (Point, Point),
        middle: f64,
    ) -> Result<(), Error> {
        let (first, last) = endpoints;
        let space = self.options.staff_size / 4.0;
        let start = &request.plan.events[spanner.start.event.0 as usize];
        let end = &request.plan.events[spanner.end.event.0 as usize];
        let up = match spanner.direction {
            Direction::Up => true,
            Direction::Down => false,
            Direction::Auto => start.heads.is_empty() || start.up,
        };
        let beamed = start
            .beam
            .filter(|beam| Some(*beam) == end.beam)
            .and_then(|index| request.plan.beams.get(index))
            .is_some_and(|beam| {
                beam.events.first() == Some(&spanner.start.event)
                    && beam.events.last() == Some(&spanner.end.event)
            });
        let side = if up { -1.0 } else { 1.0 };
        let stem_tip = |reference: gd_score::NoteRef, fallback: f64| {
            self.stem_marks
                .get(&reference.event.0)
                .and_then(|index| match self.system.marks[*index].mark {
                    Mark::Line { to, .. } => Some(to.y),
                    _ => None,
                })
                .unwrap_or(fallback)
        };
        let a = stem_tip(spanner.start, middle + start.stem_tip * space);
        let b = stem_tip(spanner.end, middle + end.stem_tip * space);
        let edge = if up {
            a.min(b).min(middle - 3.5 * space)
        } else {
            a.max(b).max(middle + 3.5 * space)
        };
        let y = edge + side * space;
        let text = number.to_string();
        let center = (first.x + last.x + 1.0 * space) / 2.0;
        self.number(&text, "tuplet", Point { x: center, y }, space, Role::Tuplet)?;
        if !beamed {
            let gap = text.len() as f64 * 0.45 * space + 0.3 * space;
            let line = y - 0.35 * space;
            for (left, right) in [(first.x, center - gap), (center + gap, last.x + space)] {
                if right > left {
                    self.line(
                        Point { x: left, y: line },
                        Point { x: right, y: line },
                        0.13 * space,
                        Role::Tuplet,
                    )?;
                }
            }
            for x in [first.x, last.x + space] {
                self.line(
                    Point { x, y: line },
                    Point { x, y: line - side * 0.6 * space },
                    0.13 * space,
                    Role::Tuplet,
                )?;
            }
        }
        Ok(())
    }
}
