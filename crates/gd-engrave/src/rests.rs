use gd_score::{Base, Direction, Duration, Error, Event, EventId, EventKind};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Rect, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn rest(
        &mut self,
        id: EventId,
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
        let Some(direction) = rest_direction(event, duration, request, column, self.staff) else {
            return Ok(());
        };
        let mut y = middle - if duration.base == Base::Whole { space } else { 0.0 };
        let side = if direction == Direction::Down { 1.0 } else { -1.0 };
        if direction != Direction::Auto {
            y += side * 2.0 * space;
        }
        let line_start = request.columns[column].at.main == request.columns[0].at.main;
        let prefix = if line_start { 0.0 } else { request.plan.events[id.0 as usize].prefix };
        let x = request.positions[column] + prefix * space;
        let nearby =
            self.nearby_rests(x + data.bounds.left * space, x + data.bounds.right * space)?;
        for attempt in 0..32 {
            let ink = Rect {
                left: x + data.bounds.left * space,
                right: x + data.bounds.right * space,
                top: y + data.bounds.top * space,
                bottom: y + data.bounds.bottom * space,
            };
            let mut collision = false;
            for other_id in &request.columns[column].events {
                charge(self.work, 1)?;
                let other = &request.score.events[other_id.0 as usize];
                if other.voice != event.voice && matches!(other.kind, EventKind::Chord(_)) {
                    let info = &request.plan.events[other_id.0 as usize];
                    if info.staff == self.staff.0 as usize {
                        // A little side room keeps a rest off a neighbouring stem.
                        let bounds = Rect {
                            left: x + info.bounds.left * space - 0.15 * space,
                            right: x + info.bounds.right * space + 0.15 * space,
                            top: middle + info.bounds.top * space - 0.5 * space,
                            bottom: middle + info.bounds.bottom * space + 0.5 * space,
                        };
                        collision |= ink.overlaps(bounds, 0.0);
                    }
                }
            }
            for previous in &nearby {
                charge(self.work, 1)?;
                collision |= ink.overlaps(*previous, 0.0);
            }
            if !collision {
                break;
            }
            if attempt == 31 {
                return Err(crate::limits::failure("Cannot separate simultaneous rests."));
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

impl Draw<'_, '_, '_, '_, '_> {
    /// Earlier rests of this staff within `left..right`, collected once per rest rather
    /// than by scanning the whole system on every placement attempt.
    fn nearby_rests(&mut self, left: f64, right: f64) -> Result<Vec<Rect>, Error> {
        let mut nearby = Vec::new();
        for previous in &self.system.marks {
            charge(self.work, 1)?;
            if previous.staff == self.staff
                && previous.mark.role() == Role::Rest
                && previous.bounds.right > left
                && previous.bounds.left < right
            {
                nearby.try_reserve(1).map_err(|_| crate::limits::failure("Rest neighbours."))?;
                nearby.push(previous.bounds);
            }
        }
        Ok(nearby)
    }
}

fn rest_direction(
    event: &Event,
    duration: &Duration,
    request: &Request<'_, '_>,
    column: usize,
    staff: gd_score::StaffId,
) -> Option<Direction> {
    let mut equal_rest = None;
    let mut any_other_rest = false;
    let mut any_note = false;
    for other_id in &request.columns[column].events {
        let other = &request.score.events[other_id.0 as usize];
        let same_staff = other.voice != event.voice
            && request.score.voices[other.voice.0 as usize].staff == staff;
        any_note |= same_staff && matches!(other.kind, EventKind::Chord(_));
        if same_staff && let EventKind::Rest(rest) = other.kind {
            any_other_rest = true;
            if rest == *duration {
                equal_rest = Some(equal_rest.map_or(other.voice, |old: gd_score::VoiceId| {
                    gd_score::VoiceId(old.0.min(other.voice.0))
                }));
            }
        }
    }
    // Equal rests merge into one only when no other voice sounds there, as in LilyPond;
    // beside a note each rest keeps its voice's side.
    if any_note {
        equal_rest = None;
    }
    if equal_rest.is_some_and(|voice| voice.0 < event.voice.0) {
        return None;
    }
    let voice_dir = event
        .voice_style
        .map(|style| style.direction())
        .unwrap_or(request.score.voices[event.voice.0 as usize].direction);
    let dir = if equal_rest.is_some() {
        Direction::Auto
    } else if voice_dir != Direction::Auto {
        voice_dir
    } else if any_other_rest {
        let earlier = request.columns[column].events.iter().any(|other_id| {
            let other = &request.score.events[other_id.0 as usize];
            other.voice.0 < event.voice.0
                && request.score.voices[other.voice.0 as usize].staff == staff
                && matches!(other.kind, EventKind::Rest(_))
        });
        if earlier { Direction::Down } else { Direction::Up }
    } else {
        Direction::Auto
    };
    Some(dir)
}
