use std::collections::BTreeMap;

use gd_score::{Base, Chord, Error};

use crate::drawing::Draw;
use crate::prepare::EventInfo;
use crate::{Point, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn heads(
        &mut self,
        chord: &Chord,
        info: &EventInfo,
        column: f64,
        middle: f64,
        line_start: bool,
    ) -> Result<(), Error> {
        let space = self.options.staff_size / 4.0;
        let size = space * info.head_scale;
        let x = column + if line_start { 0.0 } else { info.prefix * space };
        let mut ledgers = BTreeMap::<i16, (f64, f64)>::new();
        for head in &info.heads {
            let at = Point { x: x + head.at.x * space, y: middle + head.at.y * space };
            if !head.shared {
                self.glyph(info.head_glyph, at.x, at.y, size, Role::Notehead)?;
            }
            if (!head.continuation || line_start || head.accidental_forced)
                && let Some(name) = head.accidental
            {
                let accidental_x = x + head.accidental_x * space;
                self.glyph(name, accidental_x, at.y, size, Role::Accidental)?;
                if head.cautionary {
                    self.cautionary(name, accidental_x, at.y, size)?;
                }
            }
            for dot in head.dots.iter().filter(|_| !head.shared) {
                self.glyph(
                    "augmentationDot",
                    x + (dot.x + info.voice_shift) * space,
                    middle + dot.y * space,
                    size,
                    Role::Dot,
                )?;
            }
            let position = (-head.at.y * 2.0).round() as i16;
            let sign = if position < 0 { -1i16 } else { 1i16 };
            let mut ledger = 6 * sign;
            while ledger.abs() <= position.abs() && position.abs() >= 6 {
                let left = head.at.x - 0.4 * info.head_scale;
                let right = head.at.x + 1.6 * info.head_scale;
                ledgers
                    .entry(ledger)
                    .and_modify(|span| {
                        span.0 = span.0.min(left);
                        span.1 = span.1.max(right);
                    })
                    .or_insert((left, right));
                ledger += 2 * sign;
            }
        }
        for (position, (left, right)) in ledgers {
            let y = middle - f64::from(position) * space / 2.0;
            self.line(
                Point { x: x + left * space, y },
                Point { x: x + right * space, y },
                self.fonts.default_value("legerLineThickness", 0.16) * space,
                Role::Ledger,
            )?;
        }
        self.stem(chord, info, x, middle, space)?;
        Ok(())
    }
    fn cautionary(
        &mut self,
        name: &str,
        accidental_x: f64,
        y: f64,
        size: f64,
    ) -> Result<(), Error> {
        let ink = self
            .fonts
            .glyph_data(self.fonts.glyph(name)?)
            .ok_or_else(|| crate::limits::font_error("Missing accidental bounds."))?
            .bounds;
        let left = self
            .fonts
            .glyph_data(self.fonts.glyph("accidentalParensLeft")?)
            .ok_or_else(|| crate::limits::font_error("Missing parenthesis bounds."))?
            .bounds;
        let right = self
            .fonts
            .glyph_data(self.fonts.glyph("accidentalParensRight")?)
            .ok_or_else(|| crate::limits::font_error("Missing parenthesis bounds."))?
            .bounds;
        self.glyph(
            "accidentalParensLeft",
            accidental_x + (ink.left - 0.1 - left.right) * size,
            y,
            size,
            Role::Accidental,
        )?;
        self.glyph(
            "accidentalParensRight",
            accidental_x + (ink.right + 0.1 - right.left) * size,
            y,
            size,
            Role::Accidental,
        )?;
        Ok(())
    }

    fn stem(
        &mut self,
        chord: &Chord,
        info: &EventInfo,
        x: f64,
        middle: f64,
        space: f64,
    ) -> Result<(), Error> {
        let size = space * info.head_scale;
        if !matches!(chord.duration.base, Base::Whole | Base::Breve) {
            let stem_x = x + info.stem_x * space;
            self.stem_marks.insert(info.id.0, self.system.marks.len());
            self.line(
                Point { x: stem_x, y: middle + info.stem_base * space },
                Point { x: stem_x, y: middle + info.stem_tip * space },
                self.fonts.default_value("stemThickness", 0.12) * space,
                Role::Stem,
            )?;
            if info.flags > 0 && info.beam.is_none() {
                self.glyph(
                    flag(info.flags, info.up),
                    stem_x,
                    middle + info.stem_tip * space,
                    size,
                    Role::Flag,
                )?;
            }
            if chord.grace_slash {
                let tip = middle + info.stem_tip * space;
                self.line(
                    Point { x: stem_x - 0.45 * space, y: tip + space },
                    Point { x: stem_x + 0.75 * space, y: tip },
                    0.16 * space,
                    Role::Flag,
                )?;
            }
        }
        Ok(())
    }
}

pub(crate) fn flag(level: u8, up: bool) -> &'static str {
    match (level, up) {
        (1, true) => "flag8thUp",
        (1, false) => "flag8thDown",
        (2, true) => "flag16thUp",
        (2, false) => "flag16thDown",
        (3, true) => "flag32ndUp",
        (3, false) => "flag32ndDown",
        (4, true) => "flag64thUp",
        (4, false) => "flag64thDown",
        (_, true) => "flag128thUp",
        (_, false) => "flag128thDown",
    }
}
