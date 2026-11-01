use gd_score::{AnnotationKind, Error, EventKind, TextStyle};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Mark, Point, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn decorations(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        for (staff_index, staff) in request.score.staves.iter().enumerate() {
            self.staff = gd_score::StaffId(staff_index as u32);
            self.measure = 0;
            let name = if request.first <= 1 { &staff.name } else { &staff.short_name };
            if !name.is_empty() {
                self.text(
                    name,
                    TextStyle::Regular,
                    Point { x: space, y: (4.0 + staff_index as f64 * 9.0) * space },
                    2.3 * space,
                    Role::Instrument,
                )?;
            }
        }
        for (index, column) in request.columns.iter().enumerate() {
            if index + 1 == request.columns.len() {
                break;
            }
            for id in &column.events {
                let event = &request.score.events[id.0 as usize];
                self.staff = request.score.voices[event.voice.0 as usize].staff;
                let middle = (4.0 + f64::from(self.staff.0) * 9.0) * space;
                self.measure = request.first;
                let x = request.positions[index];
                if let EventKind::ChordName(name, _) = &event.kind {
                    self.text(
                        name,
                        TextStyle::Regular,
                        Point { x, y: middle - 4.0 * space },
                        2.5 * space,
                        Role::ChordName,
                    )?;
                }
                let annotation_start = self.system.marks.len();
                for mark in &event.marks {
                    charge(self.work, 1)?;
                    let info = &request.plan.events[id.0 as usize];
                    let direction = request.score.voices[event.voice.0 as usize].direction;
                    let up = mark_above(mark, info.up, direction);
                    let at = Point { x, y: middle + if up { -5.0 * space } else { 5.0 * space } };
                    let first = self.system.marks.len();
                    self.annotation(mark, at, space, up)?;
                    self.separate_annotations(annotation_start, first, middle, up, space)?;
                }
            }
        }
        self.lyrics(request)
    }

    fn annotation(
        &mut self,
        mark: &gd_score::Annotation,
        at: Point,
        space: f64,
        up: bool,
    ) -> Result<(), Error> {
        match &mark.kind {
            AnnotationKind::Text(text, style) => {
                self.text(text, *style, at, 2.4 * space, Role::Script)?
            }
            AnnotationKind::Tempo(text) => {
                self.text(text, TextStyle::Bold, at, 2.5 * space, Role::Tempo)?;
            }
            AnnotationKind::Rehearsal(text) => {
                self.text(text, TextStyle::Bold, at, 2.5 * space, Role::Rehearsal)?;
            }
            AnnotationKind::Dynamic(text) => self.dynamic(text, at, space)?,
            AnnotationKind::Articulation(name) => self.articulation(name, up, at, space)?,
            AnnotationKind::Fingering(number) => {
                self.number(&number.to_string(), "timeSig", at, space * 0.55, Role::Fingering)?;
            }
        }
        Ok(())
    }

    fn separate_annotations(
        &mut self,
        start: usize,
        first: usize,
        middle: f64,
        up: bool,
        space: f64,
    ) -> Result<(), Error> {
        let Some(bounds) =
            self.system.marks[first..].iter().map(|m| m.bounds).reduce(crate::Rect::union)
        else {
            return Ok(());
        };
        let Some(role) = self.system.marks.get(first).map(|m| m.mark.role()) else {
            return Ok(());
        };
        let mut shift: f64 = 0.0;
        for previous in &self.system.marks[start..first] {
            charge(self.work, 1)?;
            if previous.mark.role() != role
                || ((previous.bounds.top + previous.bounds.bottom) / 2.0 < middle) != up
                || previous.bounds.right <= bounds.left
                || previous.bounds.left >= bounds.right
            {
                continue;
            }
            if up {
                shift = shift.min(previous.bounds.top - bounds.bottom - 0.25 * space);
            } else {
                shift = shift.max(previous.bounds.bottom - bounds.top + 0.25 * space);
            }
        }
        for mark in &mut self.system.marks[first..] {
            crate::vertical::translate(mark, 0.0, shift);
        }
        Ok(())
    }

    pub(crate) fn text(
        &mut self,
        text: &str,
        style: TextStyle,
        at: Point,
        size: f64,
        role: Role,
    ) -> Result<(), Error> {
        let run = crate::text::shape(
            self.fonts,
            text,
            size,
            style,
            &mut self.warnings,
            self.work,
            &mut self.options.reserve,
        )?;
        let bounds = run.bounds.translated(at.x, at.y);
        self.add(Mark::Text { run, x: at.x, y: at.y, role }, bounds)
    }

    fn dynamic(&mut self, text: &str, at: Point, size: f64) -> Result<(), Error> {
        let mut x = at.x;
        for c in text.chars() {
            let name = dynamic_glyph(c).ok_or_else(|| {
                crate::limits::font_error("A dynamic contains an unsupported music character.")
            })?;
            self.glyph(name, x, at.y, size, Role::Dynamic)?;
            let width = self
                .fonts
                .glyph(name)
                .ok()
                .and_then(|g| self.fonts.glyph_data(g))
                .map(|g| g.advance)
                .unwrap_or(1.0);
            x += (width + 0.05) * size;
        }
        Ok(())
    }

    fn articulation(&mut self, name: &str, up: bool, at: Point, size: f64) -> Result<(), Error> {
        let part = match name {
            "staccato" => "articStaccato",
            "accent" => "articAccent",
            "tenuto" => "articTenuto",
            "marcato" => "articMarcato",
            "fermata" => "fermata",
            "staccatissimo" => "articStaccatissimo",
            "portato" => "articTenuto",
            "trill" => "ornamentTrill",
            "turn" => "ornamentTurn",
            "mordent" => "ornamentMordent",
            "prall" => "ornamentShortTrill",
            "stopped" => "brassMuteClosed",
            "open" => "brassMuteOpen",
            "flageolet" => "stringsHarmonic",
            "thumb" => "pluckedSnapPizzicatoAbove",
            _ => return Err(crate::limits::font_error(format!("Unknown articulation {name}."))),
        };
        let glyph = if part.starts_with("artic") || part == "fermata" {
            format!("{part}{}", if up { "Above" } else { "Below" })
        } else {
            part.into()
        };
        self.glyph(&glyph, at.x, at.y, size, Role::Articulation)?;
        if name == "portato" {
            self.glyph(
                if up { "articStaccatoAbove" } else { "articStaccatoBelow" },
                at.x,
                at.y + if up { -0.6 * size } else { 0.6 * size },
                size,
                Role::Articulation,
            )?;
        }
        Ok(())
    }
}

pub(crate) fn dynamic_glyph(character: char) -> Option<&'static str> {
    Some(match character {
        'p' => "dynamicPiano",
        'm' => "dynamicMezzo",
        'f' => "dynamicForte",
        'r' => "dynamicRinforzando",
        's' => "dynamicSforzando",
        'z' => "dynamicZ",
        _ => return None,
    })
}

fn mark_above(mark: &gd_score::Annotation, stem_up: bool, voice: gd_score::Direction) -> bool {
    use gd_score::Direction;
    match mark.direction {
        Direction::Up => true,
        Direction::Down => false,
        Direction::Auto => match &mark.kind {
            AnnotationKind::Dynamic(_) => false,
            AnnotationKind::Articulation(name)
                if matches!(
                    name.as_str(),
                    "staccato" | "accent" | "tenuto" | "marcato" | "staccatissimo" | "portato"
                ) =>
            {
                match voice {
                    Direction::Up => true,
                    Direction::Down => false,
                    Direction::Auto => !stem_up,
                }
            }
            AnnotationKind::Fingering(_) => voice != Direction::Down,
            _ => true,
        },
    }
}
