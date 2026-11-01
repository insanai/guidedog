use gd_score::{Anchor, AnnotationKind, Error, EventKind, NoteRef, TextStyle};

use crate::curves::endpoint;
use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Mark, Point, Role};

impl Draw<'_, '_, '_, '_> {
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
                for mark in &event.marks {
                    charge(self.work, 1)?;
                    let up = mark.direction != gd_score::Direction::Down;
                    let at = Point { x, y: middle + if up { -5.0 * space } else { 5.0 * space } };
                    match &mark.kind {
                        AnnotationKind::Text(text, style) => {
                            self.text(text, *style, at, 2.4 * space, Role::Script)?
                        }
                        AnnotationKind::Tempo(text) | AnnotationKind::Rehearsal(text) => {
                            self.text(text, TextStyle::Bold, at, 2.5 * space, Role::Script)?;
                        }
                        AnnotationKind::Dynamic(text) => {
                            self.dynamic(text, Point { x, y: middle + 5.0 * space }, space)?
                        }
                        AnnotationKind::Articulation(name) => {
                            self.articulation(name, up, at, space)?
                        }
                        AnnotationKind::Fingering(number) => {
                            self.number(
                                &number.to_string(),
                                "timeSig",
                                at,
                                space * 0.55,
                                Role::Articulation,
                            )?;
                        }
                    }
                }
            }
        }
        self.lyrics(request)
    }

    pub(crate) fn text(
        &mut self,
        text: &str,
        style: TextStyle,
        at: Point,
        size: f64,
        role: Role,
    ) -> Result<(), Error> {
        let run =
            crate::text::shape(self.fonts, text, size, style, &mut self.warnings, self.work)?;
        let bounds = run.bounds.translated(at.x, at.y);
        self.add(Mark::Text { run, x: at.x, y: at.y, role }, bounds)
    }

    fn dynamic(&mut self, text: &str, at: Point, size: f64) -> Result<(), Error> {
        let mut x = at.x;
        for c in text.chars() {
            let name = match c {
                'p' => "dynamicPiano",
                'm' => "dynamicMezzo",
                'f' => "dynamicForte",
                'r' => "dynamicRinforzando",
                's' => "dynamicSforzando",
                'z' => "dynamicZ",
                _ => continue,
            };
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

    fn lyrics(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let first = request.columns[0].at;
        let last = request.columns.last().map(|c| c.at).unwrap_or(first);
        let mut verses = vec![0usize; request.score.staves.len()];
        for verse in &request.score.lyrics {
            self.staff = request.score.voices[verse.voice.0 as usize].staff;
            let middle = (4.0 + f64::from(self.staff.0) * 9.0) * space;
            let row = verses[self.staff.0 as usize];
            verses[self.staff.0 as usize] += 1;
            let baseline = middle + (7.0 + row as f64 * 3.0) * space;
            for syllable in &verse.syllables {
                charge(self.work, 1)?;
                let (moment, x) = match syllable.anchor {
                    Anchor::Note(reference) => {
                        let e = &request.score.events[reference.event.0 as usize];
                        (e.at, endpoint(request.score, reference, request, middle, space).x)
                    }
                    Anchor::Moment(moment) => {
                        let i = request.columns.partition_point(|c| c.at < moment);
                        (moment, request.positions.get(i).copied().unwrap_or(0.0))
                    }
                };
                if moment < first || moment >= last {
                    continue;
                }
                let run = crate::text::shape(
                    self.fonts,
                    &syllable.text,
                    2.4 * space,
                    TextStyle::Regular,
                    &mut self.warnings,
                    self.work,
                )?;
                let x = x - run.advance / 2.0;
                let bounds = run.bounds.translated(x, baseline);
                self.add(Mark::Text { run, x, y: baseline, role: Role::Lyric }, bounds)?;
                if syllable.hyphen {
                    self.line(
                        Point { x: x + 2.0 * space, y: baseline - 0.7 * space },
                        Point { x: x + 2.6 * space, y: baseline - 0.7 * space },
                        0.12 * space,
                        Role::Lyric,
                    )?;
                }
                if let Some(Anchor::Note(NoteRef { event, note })) = syllable.extender {
                    let to =
                        endpoint(request.score, NoteRef { event, note }, request, middle, space);
                    self.line(
                        Point { x: x + 2.0 * space, y: baseline + 0.2 * space },
                        Point { x: to.x, y: baseline + 0.2 * space },
                        0.12 * space,
                        Role::Lyric,
                    )?;
                }
            }
        }
        Ok(())
    }
}
