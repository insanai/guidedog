use gd_score::{AnnotationKind, Error, EventKind, Markup, MarkupRole, TextStyle};

use crate::drawing::{Draw, Request};
use crate::limits::charge;
use crate::{Mark, Point, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn decorations(&mut self, request: &Request<'_, '_>) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        self.instrument_names(request, space)?;
        for (index, column) in request.columns.iter().enumerate() {
            if index + 1 == request.columns.len() {
                break;
            }
            let line_start = column.at.main == request.columns[0].at.main;
            for id in &column.events {
                let event = &request.score.events[id.0 as usize];
                let info = &request.plan.events[id.0 as usize];
                self.staff = request.score.voices[event.voice.0 as usize].staff;
                let middle = (4.0 + f64::from(self.staff.0) * 9.0) * space;
                self.measure = request.first;
                let x =
                    request.positions[index] + if line_start { 0.0 } else { info.prefix * space };
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
                let tab = request.score.staves[self.staff.0 as usize].kind
                    == gd_score::StaffKind::Tablature;
                for mark in &event.marks {
                    charge(self.work, 1)?;
                    // Default tablature shows no scripts, fingerings or dynamics; tempo and
                    // rehearsal marks belong to the whole score and stay.
                    let local = !matches!(
                        mark.kind,
                        AnnotationKind::Tempo(_)
                            | AnnotationKind::Rehearsal(_)
                            | AnnotationKind::Markup(_, MarkupRole::Tempo | MarkupRole::Rehearsal)
                    );
                    if tab && local {
                        continue;
                    }
                    let direction = request.score.voices[event.voice.0 as usize].direction;
                    let up = mark_above(mark, info.up, direction);
                    // A shifted voice's marks follow its noteheads, not the column.
                    let y = middle + if up { -5.0 * space } else { 5.0 * space };
                    let at = Point { x: x + info.voice_shift * space, y };
                    let first = self.system.marks.len();
                    self.annotation(mark, at, space, up)?;
                    self.separate_annotations(annotation_start, first, middle, up, space)?;
                }
            }
        }
        self.lyrics(request)
    }

    fn instrument_names(&mut self, request: &Request<'_, '_>, space: f64) -> Result<(), Error> {
        let first_line = self.first_line(request);
        for (staff_index, staff) in request.score.staves.iter().enumerate() {
            self.staff = gd_score::StaffId(staff_index as u32);
            self.measure = 0;
            let middle = (4.0 + staff_index as f64 * 9.0) * space;
            let label = crate::labels::line_label(
                first_line,
                (&staff.name, staff.name_markup.as_ref()),
                (&staff.short_name, staff.short_name_markup.as_ref()),
            );
            // A named group's label stands at the left edge; its staves' names follow it.
            let mut x = space;
            for group in &request.score.groups {
                if (group.first.0..=group.last.0).contains(&(staff_index as u32)) {
                    let own = self.group_label(group, first_line);
                    let advance = crate::labels::label_advance(
                        self.fonts,
                        own,
                        space,
                        self.work,
                        &mut self.options.reserve,
                    )?;
                    if advance > 0.0 {
                        x += advance + space;
                    }
                }
            }
            self.instrument_label(label, x, middle, space)?;
        }
        Ok(())
    }

    /// Whether this system shows full names: it starts at the score's first measure.
    pub(crate) fn first_line(&self, request: &Request<'_, '_>) -> bool {
        request.measure_index == 0
    }

    /// The name a group shows on this line.
    pub(crate) fn group_label<'g>(
        &self,
        group: &'g gd_score::StaffGroup,
        first_line: bool,
    ) -> (&'g str, Option<&'g Markup>) {
        crate::labels::line_label(
            first_line,
            (&group.name, group.name_markup.as_ref()),
            (&group.short_name, group.short_name_markup.as_ref()),
        )
    }

    /// Draws an instrument name starting at `x`, centred on the staff line `middle`.
    pub(crate) fn instrument_label(
        &mut self,
        label: (&str, Option<&Markup>),
        x: f64,
        middle: f64,
        space: f64,
    ) -> Result<(), Error> {
        match label {
            (_, Some(markup)) => {
                let mut ctx = crate::markup::Context {
                    fonts: self.fonts,
                    space,
                    warnings: &mut self.warnings,
                    work: self.work,
                    reserve: &mut self.options.reserve,
                };
                let run = crate::markup::shape(&mut ctx, markup, 2.3 * space, TextStyle::Regular)?;
                let center_y = middle - (run.bounds.top + run.bounds.bottom) * 0.5;
                let bounds = run.bounds.translated(x, center_y);
                self.add(Mark::Text { run, x, y: center_y, role: Role::Instrument }, bounds)
            }
            ("", None) => Ok(()),
            (name, None) => self.text(
                name,
                TextStyle::Regular,
                Point { x, y: middle },
                2.3 * space,
                Role::Instrument,
            ),
        }
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
            AnnotationKind::Markup(markup, role) => {
                let (style, size, mark_role) = match role {
                    MarkupRole::Script => (TextStyle::Regular, 2.4 * space, Role::Script),
                    MarkupRole::Tempo => (TextStyle::Bold, 2.5 * space, Role::Tempo),
                    MarkupRole::Rehearsal => (TextStyle::Bold, 2.5 * space, Role::Rehearsal),
                };
                self.markup(markup, style, at, (size, space), mark_role)?;
            }
            AnnotationKind::Breath => {
                // The comma stands just above the top line after the note; `at` is five
                // spaces from the middle of the staff.
                let y = at.y + 2.5 * space;
                self.glyph(
                    "breathMarkComma",
                    at.x + BREATH_X * space,
                    y,
                    space,
                    Role::Articulation,
                )?;
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

    pub(crate) fn markup(
        &mut self,
        markup: &Markup,
        style: TextStyle,
        at: Point,
        metrics: (f64, f64),
        role: Role,
    ) -> Result<(), Error> {
        let (size, space) = metrics;
        let mut ctx = crate::markup::Context {
            fonts: self.fonts,
            space,
            warnings: &mut self.warnings,
            work: self.work,
            reserve: &mut self.options.reserve,
        };
        let run = crate::markup::shape(&mut ctx, markup, size, style)?;
        let bounds = run.bounds.translated(at.x, at.y);
        self.add(Mark::Text { run, x: at.x, y: at.y, role }, bounds)
    }

    fn dynamic(&mut self, text: &str, at: Point, size: f64) -> Result<(), Error> {
        let start = self.system.marks.len();
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
        self.end_sign(start)
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

/// Where a breathing sign stands after its note's column, in staff spaces: past a
/// notehead and a little air.
pub(crate) const BREATH_X: f64 = 1.7;

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
