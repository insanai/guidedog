use gd_score::{BarStyle, Error, EventKind, Moment, Ratio};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Point, Role};

impl Draw<'_, '_, '_, '_> {
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
        for (index, column) in request.columns.iter().enumerate().skip(1) {
            charge(self.work, 1)?;
            if column.at.grace != Ratio::ZERO {
                continue;
            }
            let boundary = request.score.measures.partition_point(|m| m.end < column.at.main);
            if request.score.measures.get(boundary).is_none_or(|m| m.end != column.at.main) {
                continue;
            }
            let explicit = column.events.iter().any(|id| {
                let event = &request.score.events[id.0 as usize];
                request.score.voices[event.voice.0 as usize].staff == self.staff
                    && matches!(event.kind, EventKind::Bar(_))
            });
            if !explicit {
                self.bar(BarStyle::Single, request.positions[index], middle)?;
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
            let finish = event.at.main.checked_add(duration.time()?)?;
            if finish <= start || event.at.main >= end {
                continue;
            }
            let first = event.at.main.max(start);
            let last = finish.min(end);
            let at = |time| {
                let index = request
                    .columns
                    .partition_point(|c| c.at < Moment { main: time, grace: Ratio::ZERO });
                request.positions.get(index).copied().unwrap_or(0.0)
            };
            let (left, right) = (at(first), at(last));
            let count = request
                .score
                .measures
                .iter()
                .filter(|m| m.start >= first && m.end <= last)
                .count();
            let x = (left + right) / 2.0;
            if count <= 1 {
                self.glyph("restWhole", x - 0.6 * space, middle - space, space, Role::Rest)?;
            } else {
                let half = ((right - left) * 0.25).clamp(space, 4.0 * space);
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
            }
        }
        Ok(())
    }
}
