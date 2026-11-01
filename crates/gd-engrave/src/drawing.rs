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
    pub(crate) measure_index: usize,
    pub(crate) plan: &'a crate::prepare::Plan,
    pub(crate) indent: f64,
    pub(crate) preface_end: f64,
    pub(crate) staff_right: f64,
    pub(crate) suffix: f64,
    pub(crate) remaining_marks: usize,
    pub(crate) clefs: [(Clef, Clef); 24],
}

pub(crate) fn draw_system(
    request: &Request<'_, '_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(System, Vec<Error>), Error> {
    let Request { score, fonts, positions, first, .. } = *request;
    let space = options.staff_size / 4.0;
    let width = if request.suffix > 0.0 {
        positions.last().copied().unwrap_or(space) + request.suffix
    } else {
        request.staff_right
    };
    let mut draw = Draw {
        fonts,
        options,
        work,
        system: System { first_measure: first, last_measure: first, ..System::default() },
        staff: StaffId(0),
        measure: 0,
        warnings: Vec::new(),
        stem_marks: std::collections::BTreeMap::new(),
        mark_limit: request.remaining_marks,
    };
    for staff_index in 0..score.staves.len() {
        draw.staff_system(request, staff_index, width)?;
    }
    if request.measure_index != 0 {
        draw.staff = StaffId(0);
        draw.measure = 0;
        draw.number(
            &request.first.to_string(),
            "timeSig",
            Point { x: request.indent + space, y: -1.5 * space },
            space * 0.5,
            Role::MeasureNumber,
        )?;
    }
    draw.beams(request)?;
    draw.grace_slashes(request)?;
    draw.decorations(request)?;
    draw.spanners(request)?;
    crate::vertical::place(
        &mut draw.system,
        score.staves.len(),
        space,
        draw.work,
        draw.options.scratch,
    )?;
    draw.groups(request)?;
    crate::vertical::fit(&mut draw.system, space, draw.work)?;
    Ok((draw.system, draw.warnings))
}

pub(crate) struct Draw<'a, 'f, 'o, 'callback, 'work> {
    pub(crate) fonts: &'a Fonts<'f>,
    pub(crate) options: &'o mut Options<'callback>,
    pub(crate) work: &'work mut usize,
    pub(crate) system: System,
    pub(crate) staff: StaffId,
    pub(crate) measure: u32,
    pub(crate) warnings: Vec<Error>,
    pub(crate) stem_marks: std::collections::BTreeMap<u32, usize>,
    mark_limit: usize,
}

impl Draw<'_, '_, '_, '_, '_> {
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
        let measure = request.measure_index;
        let clef = request
            .plan
            .measures
            .get(measure)
            .and_then(|m| m.get(staff_index))
            .map(|state| state.clef)
            .unwrap_or(Clef::Treble);
        self.clef(clef, request.indent + space, middle, space)?;
        self.signatures(request, middle)?;
        if let Some(open_style) =
            crate::bars::opening_bar(score, &columns[0], staff_index, measure == 0)
        {
            let (_, right) = crate::bars::style_extent(open_style);
            self.bar(open_style, request.preface_end - right * space, middle)?;
        }
        self.staff_music(request, middle)?;
        self.measure_bars(request, middle)?;
        self.measure_rests(request, middle)?;
        self.courtesy(request, middle)?;
        Ok(())
    }

    fn staff_music(&mut self, request: &Request<'_, '_>, middle: f64) -> Result<(), Error> {
        let Request { score, columns, first, .. } = *request;
        let staff = self.staff;
        for (index, column) in columns.iter().enumerate() {
            let terminal = index + 1 == columns.len();
            let measure = score
                .measures
                .partition_point(|m| {
                    if terminal { m.start < column.at.main } else { m.start <= column.at.main }
                })
                .saturating_sub(1);
            let number = score.measures.get(measure).map(|m| m.number).unwrap_or(first);
            self.system.last_measure = self.system.last_measure.max(number);
            self.measure = number;
            for id in &column.events {
                charge(self.work, 1)?;
                let event = &score.events[id.0 as usize];
                if score.voices[event.voice.0 as usize].staff != staff {
                    continue;
                }
                if terminal && event.kind.duration().is_some() {
                    continue;
                }
                if terminal
                    && matches!(
                        event.kind,
                        EventKind::Clef(_) | EventKind::Key(_) | EventKind::Time(_)
                    )
                {
                    continue;
                }
                self.music_event(request, *id, index, middle)?;
            }
        }
        Ok(())
    }

    fn music_event(
        &mut self,
        request: &Request<'_, '_>,
        id: gd_score::EventId,
        index: usize,
        middle: f64,
    ) -> Result<(), Error> {
        let event = &request.score.events[id.0 as usize];
        let columns = request.columns;
        let column = &columns[index];
        let x = request.positions[index];
        let space = self.options.staff_size / 4.0;
        match &event.kind {
            EventKind::Clef(value) => {
                if index != 0 {
                    let info = &request.plan.events[id.0 as usize];
                    self.clef(*value, x + info.notation_x * space, middle, space * 0.75)?;
                }
            }
            EventKind::Chord(chord) => self.heads(
                chord,
                &request.plan.events[id.0 as usize],
                x,
                middle,
                column.at.main == request.columns[0].at.main,
            )?,
            EventKind::Rest(duration) => {
                self.rest(id, event, duration, request, index, middle)?;
            }
            EventKind::Bar(style) => {
                if index == 0 {
                    return Ok(());
                }
                let terminal = index + 1 == columns.len();
                let score_end =
                    request.score.measures.last().is_some_and(|m| m.end == column.at.main);
                let effective =
                    if terminal { crate::bars::closing_style(*style, score_end) } else { *style };
                // A repeat start that a system end reduces to a single line adds nothing
                // where another bar event of the staff already closes the system.
                let staff = request.score.voices[event.voice.0 as usize].staff;
                let stronger = terminal
                    && effective == gd_score::BarStyle::Single
                    && column.events.iter().any(|other| {
                        let other = &request.score.events[other.0 as usize];
                        request.score.voices[other.voice.0 as usize].staff == staff
                            && matches!(other.kind, EventKind::Bar(s)
                                if crate::bars::closing_style(s, score_end)
                                    != gd_score::BarStyle::Single)
                    });
                if !stronger {
                    self.bar(effective, x, middle)?;
                }
            }
            EventKind::MeasureRest(_) => {}
            EventKind::Key(_) | EventKind::Time(_) => self.signature_change(
                event,
                &request.plan.events[id.0 as usize],
                x,
                middle,
                column.at.main != columns[0].at.main,
            )?,
            EventKind::Skip(_) | EventKind::Break(_) | EventKind::ChordName(_, _) => {}
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
        if self.system.marks.len() >= self.mark_limit {
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
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let length = (dx * dx + dy * dy).sqrt().max(0.001);
        let horizontal = thickness * 0.5 * dy.abs() / length;
        let vertical = thickness * 0.5 * dx.abs() / length;
        let bounds = Rect {
            left: from.x.min(to.x) - horizontal,
            right: from.x.max(to.x) + horizontal,
            top: from.y.min(to.y) - vertical,
            bottom: from.y.max(to.y) + vertical,
        };
        self.add(Mark::Line { from, to, thickness, role }, bounds)
    }

    pub(crate) fn clef(
        &mut self,
        clef: Clef,
        x: f64,
        middle: f64,
        size: f64,
    ) -> Result<(), Error> {
        let (name, offset) = match clef {
            Clef::Treble => ("gClef", 1.0),
            Clef::TrebleEight => ("gClef8vb", 1.0),
            Clef::Bass => ("fClef", -1.0),
            Clef::BassEight => ("fClef8vb", -1.0),
            Clef::Alto => ("cClef", 0.0),
            Clef::Tenor => ("cClef", -1.0),
            Clef::Soprano => ("cClef", 2.0),
        };
        let name = if size < self.options.staff_size / 4.0 {
            match name {
                "gClef" => "gClefSmall",
                "fClef" => "fClefSmall",
                "cClef" => "cClefSmall",
                _ => name,
            }
        } else {
            name
        };
        self.glyph(name, x, middle + offset * self.options.staff_size / 4.0, size, Role::Clef)
    }
}

pub(crate) fn rest_glyph(base: Base) -> &'static str {
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
