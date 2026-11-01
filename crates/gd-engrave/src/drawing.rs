use gd_score::{Base, Clef, Error, EventKind, Score, StaffId};

use crate::columns::Column;
use crate::limits::{charge, failure, push};
use crate::{Fonts, Mark, Options, Point, PositionedMark, Rect, Role, System};

pub(crate) struct Request<'a, 'f> {
    pub(crate) score: &'a Score,
    pub(crate) fonts: &'a Fonts<'f>,
    pub(crate) columns: &'a [Column],
    pub(crate) positions: &'a [f64],
    pub(crate) first: u32,
    pub(crate) plan: &'a crate::prepare::Plan,
    pub(crate) indent: f64,
}

pub(crate) fn draw_system(
    request: &Request<'_, '_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(System, Vec<Error>), Error> {
    let Request { score, fonts, positions, first, .. } = *request;
    let space = options.staff_size / 4.0;
    let width = positions.last().copied().unwrap_or(space) + space;
    let mut draw = Draw {
        fonts,
        options,
        work,
        system: System { first_measure: first, last_measure: first, ..System::default() },
        staff: StaffId(0),
        measure: 0,
        warnings: Vec::new(),
        stem_marks: std::collections::BTreeMap::new(),
    };
    for staff_index in 0..score.staves.len() {
        draw.staff_system(request, staff_index, width)?;
    }
    draw.beams(request)?;
    draw.spanners(request)?;
    draw.decorations(request)?;
    crate::vertical::place(&mut draw.system, score.staves.len(), space, draw.work)?;
    Ok((draw.system, draw.warnings))
}

pub(crate) struct Draw<'f, 'o, 'callback, 'work> {
    pub(crate) fonts: &'f Fonts<'f>,
    pub(crate) options: &'o mut Options<'callback>,
    pub(crate) work: &'work mut usize,
    pub(crate) system: System,
    pub(crate) staff: StaffId,
    pub(crate) measure: u32,
    pub(crate) warnings: Vec<Error>,
    pub(crate) stem_marks: std::collections::BTreeMap<u32, usize>,
}

impl Draw<'_, '_, '_, '_> {
    fn staff_system(
        &mut self,
        request: &Request<'_, '_>,
        staff_index: usize,
        width: f64,
    ) -> Result<(), Error> {
        let Request { score, fonts, columns, .. } = *request;
        let space = self.options.staff_size / 4.0;
        let staff = StaffId(staff_index as u32);
        self.staff = staff;
        self.measure = 0;
        let middle = (4.0 + staff_index as f64 * 9.0) * space;
        for offset in -2..=2 {
            let y = middle + f64::from(offset) * space;
            self.line(
                Point { x: request.indent, y },
                Point { x: width, y },
                fonts.default_value("staffLineThickness", 0.13) * space,
                Role::Staff,
            )?;
        }
        let mut clef = Clef::Treble;
        for event in &score.events {
            charge(self.work, 1)?;
            if event.at <= columns[0].at
                && score.voices[event.voice.0 as usize].staff == staff
                && let EventKind::Clef(value) = event.kind
            {
                clef = value;
            }
        }
        self.clef(clef, request.indent + space, middle, space)?;
        self.signatures(request, middle)?;
        self.staff_music(request, middle)?;
        self.line(
            Point { x: width, y: middle - 2.0 * space },
            Point { x: width, y: middle + 2.0 * space },
            0.16 * space,
            Role::Bar,
        )?;
        Ok(())
    }

    fn staff_music(&mut self, request: &Request<'_, '_>, middle: f64) -> Result<(), Error> {
        let Request { score, columns, positions, first, .. } = *request;
        let space = self.options.staff_size / 4.0;
        let staff = self.staff;
        for (index, column) in columns.iter().enumerate() {
            let x = positions[index];
            let measure =
                score.measures.partition_point(|m| m.start <= column.at.main).saturating_sub(1);
            let number = score.measures.get(measure).map(|m| m.number).unwrap_or(first);
            self.system.last_measure = self.system.last_measure.max(number);
            self.measure = number;
            for id in &column.events {
                charge(self.work, 1)?;
                let event = &score.events[id.0 as usize];
                if score.voices[event.voice.0 as usize].staff != staff {
                    continue;
                }
                if index + 1 == columns.len() && event.kind.duration().is_some() {
                    continue;
                }
                match &event.kind {
                    EventKind::Clef(value) => {
                        if index != 0 {
                            self.clef(*value, x, middle, space * 0.75)?;
                        }
                    }
                    EventKind::Chord(chord) => self.heads(
                        chord,
                        &request.plan.events[id.0 as usize],
                        x,
                        middle,
                        column.at.main == request.columns[0].at.main,
                    )?,
                    EventKind::Rest(duration) | EventKind::MeasureRest(duration) => {
                        self.glyph(rest(duration.base), x, middle, space, Role::Rest)?;
                    }
                    EventKind::Bar(_) => self.line(
                        Point { x, y: middle - 2.0 * space },
                        Point { x, y: middle + 2.0 * space },
                        0.16 * space,
                        Role::Bar,
                    )?,
                    EventKind::Key(_) | EventKind::Time(_) => self.signature_change(
                        event,
                        &request.plan.events[id.0 as usize],
                        x,
                        middle,
                        column.at.main != columns[0].at.main,
                    )?,
                    EventKind::Skip(_) | EventKind::Break(_) | EventKind::ChordName(_, _) => {}
                }
            }
        }
        Ok(())
    }

    fn signature_change(
        &mut self,
        event: &gd_score::Event,
        info: &crate::prepare::EventInfo,
        x: f64,
        middle: f64,
        changed: bool,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        match &event.kind {
            EventKind::Key(key) => {
                if changed {
                    let at = Point { x: x + info.notation_x * space, y: middle };
                    let canceled = self.cancel_key(
                        info.previous_key.unwrap_or_default(),
                        *key,
                        info.state.clef,
                        at,
                        space,
                    )?;
                    self.key_signature(
                        *key,
                        info.state.clef,
                        Point { x: at.x + canceled, y: middle },
                        space,
                    )?;
                }
            }
            EventKind::Time(meter) if changed => {
                self.time_signature(
                    meter,
                    Point { x: x + info.notation_x * space, y: middle },
                    space,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn add(&mut self, mark: Mark, bounds: Rect) -> Result<(), Error> {
        charge(self.work, 1)?;
        if self.system.marks.len() >= self.options.limits.marks {
            return Err(failure("The drawing exceeds the positioned-mark limit."));
        }
        push(
            &mut self.system.marks,
            PositionedMark { mark, bounds, staff: self.staff, measure: self.measure },
            &mut self.options.reserve,
        )
    }

    pub(crate) fn glyph(
        &mut self,
        name: &str,
        x: f64,
        y: f64,
        size: f64,
        role: Role,
    ) -> Result<(), Error> {
        let glyph = self.fonts.glyph(name)?;
        let data =
            self.fonts.glyph_data(glyph).ok_or_else(|| failure("Missing glyph geometry."))?;
        let bounds = Rect {
            left: x + data.bounds.left * size,
            right: x + data.bounds.right * size,
            top: y + data.bounds.top * size,
            bottom: y + data.bounds.bottom * size,
        };
        self.add(Mark::Glyph { glyph, x, y, size, role }, bounds)
    }

    pub(crate) fn line(
        &mut self,
        from: Point,
        to: Point,
        thickness: f64,
        role: Role,
    ) -> Result<(), Error> {
        let half = thickness / 2.0;
        let bounds = Rect {
            left: from.x.min(to.x) - half,
            right: from.x.max(to.x) + half,
            top: from.y.min(to.y) - half,
            bottom: from.y.max(to.y) + half,
        };
        self.add(Mark::Line { from, to, thickness, role }, bounds)
    }

    fn clef(&mut self, clef: Clef, x: f64, middle: f64, size: f64) -> Result<(), Error> {
        let (name, offset) = match clef {
            Clef::Treble | Clef::TrebleEight => ("gClef", 1.0),
            Clef::Bass | Clef::BassEight => ("fClef", -1.0),
            Clef::Alto => ("cClef", 0.0),
            Clef::Tenor => ("cClef", -1.0),
            Clef::Soprano => ("cClef", 2.0),
        };
        self.glyph(name, x, middle + offset * self.options.staff_size / 4.0, size, Role::Clef)
    }
}

fn rest(base: Base) -> &'static str {
    match base {
        Base::Breve => "restDoubleWhole",
        Base::Whole => "restWhole",
        Base::Half => "restHalf",
        Base::Quarter => "restQuarter",
        Base::Eighth => "rest8th",
        Base::Sixteenth => "rest16th",
        Base::ThirtySecond => "rest32nd",
        Base::SixtyFourth => "rest64th",
        Base::HundredTwentyEighth => "rest128th",
    }
}
