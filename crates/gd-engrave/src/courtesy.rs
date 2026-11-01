//! Courtesy signatures reserve a suffix and repeat the coming context at a system end.

use gd_score::{Error, EventKind, Ratio, Score};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Fonts, Point};

pub(crate) fn width(
    score: &Score,
    plan: &crate::prepare::Plan,
    fonts: &Fonts<'_>,
    at: Ratio,
    work: &mut usize,
) -> Result<f64, Error> {
    let mut widths = [0.0f64; 24];
    let mut seen = [[false; 3]; 24];
    for (index, event) in score.events.iter().enumerate() {
        charge(work, 1)?;
        if event.at.main != at {
            continue;
        }
        let staff = score.voices[event.voice.0 as usize].staff.0 as usize;
        let (kind, width) = match &event.kind {
            EventKind::Clef(_) => (0, 2.5),
            EventKind::Key(key) => {
                let old = plan.events[index].previous_key.unwrap_or_default();
                let canceled = crate::signatures::cancel_width(fonts, old, *key)?;
                let printed = crate::signatures::key_width(fonts, *key)?;
                (1, canceled + printed + 0.5)
            }
            EventKind::Time(meter) => (2, crate::signatures::meter_width(fonts, meter)? + 0.6),
            _ => continue,
        };
        if !seen[staff][kind] {
            if widths[staff] == 0.0 {
                widths[staff] = 0.75;
            }
            widths[staff] += width * 0.75 + 0.5;
        }
        seen[staff][kind] = true;
    }
    Ok(widths.into_iter().fold(0.0, f64::max))
}

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn courtesy(
        &mut self,
        request: &Request<'_, '_>,
        middle: f64,
    ) -> Result<(), Error> {
        if request.suffix == 0.0 {
            return Ok(());
        }
        let Some(last) = request.columns.last() else { return Ok(()) };
        let space = self.options.staff_size / 4.0;
        let size = space * 0.75;
        let bar_right = crate::bars::closing_bar_right(request, self.work)?;
        let mut x = request.positions.last().copied().unwrap_or(0.0) + (bar_right + 0.5) * space;
        let mut seen = [false; 3];
        self.measure = 0;
        for id in &last.events {
            charge(self.work, 1)?;
            let event = &request.score.events[id.0 as usize];
            if request.score.voices[event.voice.0 as usize].staff != self.staff {
                continue;
            }
            let info = &request.plan.events[id.0 as usize];
            let (kind, advance) = match &event.kind {
                EventKind::Clef(clef) if !seen[0] => {
                    self.clef(*clef, x, middle, size)?;
                    (0, 2.5 * size)
                }
                EventKind::Key(key) if !seen[1] => {
                    let canceled = self.cancel_key(
                        info.previous_key.unwrap_or_default(),
                        *key,
                        info.state.clef,
                        Point { x, y: middle },
                        size,
                    )?;
                    let width = self.key_signature(
                        *key,
                        info.state.clef,
                        Point { x: x + canceled, y: middle },
                        size,
                    )?;
                    (1, width + canceled)
                }
                EventKind::Time(meter) if !seen[2] => {
                    self.time_signature(meter, Point { x, y: middle }, size)?;
                    (2, crate::signatures::meter_width(self.fonts, meter)? * size)
                }
                _ => continue,
            };
            x += advance + 0.5 * space;
            seen[kind] = true;
        }
        Ok(())
    }
}
