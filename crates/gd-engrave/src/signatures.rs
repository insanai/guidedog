use gd_score::{Alter, Clef, Error, Key, Meter, MeterStyle, Step};

use crate::drawing::{Draw, Request};
use crate::prepare::accidental;
use crate::{Fonts, Point, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn signatures(
        &mut self,
        request: &Request<'_, '_>,
        middle: f64,
    ) -> Result<(), Error> {
        let score = request.score;
        let measure = request.measure_index;
        let Some(state) =
            request.plan.measures.get(measure).and_then(|m| m.get(self.staff.0 as usize))
        else {
            return Ok(());
        };
        let space = self.options.staff_size / 4.0;
        let x = request.indent + 4.2 * space;
        let width = self.key_signature(state.key, state.clef, Point { x, y: middle }, space)?;
        let meter_changed = measure == 0 || score.measures[measure - 1].meter != state.meter;
        if meter_changed {
            let gap = if width > 0.0 { 0.5 * space } else { 0.0 };
            self.time_signature(&state.meter, Point { x: x + width + gap, y: middle }, space)?;
        }
        Ok(())
    }

    pub(crate) fn key_signature(
        &mut self,
        key: Key,
        clef: Clef,
        at: Point,
        size: f64,
    ) -> Result<f64, Error> {
        let alterations = key.alterations()?;
        let sharp = [Step::F, Step::C, Step::G, Step::D, Step::A, Step::E, Step::B];
        let flat = [Step::B, Step::E, Step::A, Step::D, Step::G, Step::C, Step::F];
        let mut offset = 0.0;
        for (steps, positive) in [(&sharp, true), (&flat, false)] {
            for step in steps {
                let alter = alterations[step.index() as usize];
                if alter == Alter::Natural || ((alter.semitones() > 0) != positive) {
                    continue;
                }
                let name = accidental(alter);
                let step_w = glyph_step(self.fonts, name)?;
                let y = signature_y(*step, alter.semitones() > 0, clef);
                self.glyph(name, at.x + offset, at.y + y * size, size, Role::Signature)?;
                offset += step_w * size;
            }
        }
        Ok(offset)
    }

    pub(crate) fn cancel_key(
        &mut self,
        old: Key,
        new: Key,
        clef: Clef,
        at: Point,
        size: f64,
    ) -> Result<f64, Error> {
        let previous = old.alterations()?;
        let next = new.alterations()?;
        let step_w = glyph_step(self.fonts, "accidentalNatural")?;
        let mut offset = 0.0;
        for step in [Step::F, Step::C, Step::G, Step::D, Step::A, Step::E, Step::B] {
            let old = previous[step.index() as usize];
            if old != Alter::Natural && old != next[step.index() as usize] {
                let y = signature_y(step, old.semitones() > 0, clef);
                self.glyph(
                    "accidentalNatural",
                    at.x + offset,
                    at.y + y * size,
                    size,
                    Role::Signature,
                )?;
                offset += step_w * size;
            }
        }
        Ok(offset)
    }

    pub(crate) fn time_signature(
        &mut self,
        meter: &Meter,
        at: Point,
        size: f64,
    ) -> Result<(), Error> {
        if let Some(name) = meter_symbol(meter) {
            return self.glyph(name, at.x, at.y, size, Role::Signature);
        }
        let numerator = meter.numerator.to_string();
        let denominator = meter.denominator.to_string();
        let width = meter_width(self.fonts, meter)? * size;
        self.number(
            &numerator,
            "timeSig",
            Point { x: at.x + width / 2.0, y: at.y - 1.05 * size },
            size,
            Role::Signature,
        )?;
        self.number(
            &denominator,
            "timeSig",
            Point { x: at.x + width / 2.0, y: at.y + 1.05 * size },
            size,
            Role::Signature,
        )
    }
}

pub(crate) fn key_width(fonts: &Fonts<'_>, key: Key) -> Result<f64, Error> {
    let alterations = key.alterations()?;
    let mut width = 0.0;
    for alter in alterations {
        if alter != Alter::Natural {
            width += glyph_step(fonts, accidental(alter))?;
        }
    }
    Ok(width)
}

pub(crate) fn cancel_width(fonts: &Fonts<'_>, old: Key, new: Key) -> Result<f64, Error> {
    let previous = old.alterations()?;
    let next = new.alterations()?;
    let step = glyph_step(fonts, "accidentalNatural")?;
    let count = previous
        .iter()
        .zip(next)
        .filter(|(old, new)| **old != Alter::Natural && **old != *new)
        .count();
    Ok(count as f64 * step)
}

/// The glyph that stands for `meter`, as LilyPond's default style draws 4/4 and 2/2, or
/// `None` when the meter is drawn as numbers.
fn meter_symbol(meter: &Meter) -> Option<&'static str> {
    match (meter.style, meter.numerator, meter.denominator) {
        (MeterStyle::Symbol, 4, 4) => Some("timeSigCommon"),
        (MeterStyle::Symbol, 2, 2) => Some("timeSigCutCommon"),
        _ => None,
    }
}

pub(crate) fn meter_width(fonts: &Fonts<'_>, meter: &Meter) -> Result<f64, Error> {
    if let Some(name) = meter_symbol(meter) {
        let data = fonts
            .glyph_data(fonts.glyph(name)?)
            .ok_or_else(|| crate::limits::font_error("Missing time signature geometry."))?;
        return Ok(data.advance.max(data.bounds.right));
    }
    let num = fonts.digits_width(&meter.numerator.to_string(), "timeSig")?;
    let den = fonts.digits_width(&meter.denominator.to_string(), "timeSig")?;
    Ok(num.max(den))
}

fn glyph_step(fonts: &Fonts<'_>, name: &str) -> Result<f64, Error> {
    let glyph = fonts.glyph(name)?;
    let width = fonts
        .glyph_data(glyph)
        .map(|d| (d.bounds.right - d.bounds.left.min(0.0)).max(d.advance))
        .unwrap_or(0.9);
    Ok(width + 0.14)
}

fn signature_y(step: Step, sharp: bool, clef: Clef) -> f64 {
    let diatonic = match clef {
        Clef::Tenor if sharp => match step {
            Step::F => 3,
            Step::C => 7,
            Step::G => 4,
            Step::D => 8,
            Step::A => 5,
            Step::E => 9,
            Step::B => 6,
        },
        Clef::Soprano if sharp => match step {
            Step::F => 10,
            Step::C => 14,
            Step::G => 11,
            Step::D => 15,
            Step::A => 12,
            Step::E => 9,
            Step::B => 13,
        },
        Clef::Soprano => match step {
            Step::B => 13,
            Step::E => 9,
            Step::A => 12,
            Step::D => 8,
            Step::G => 11,
            Step::C => 14,
            Step::F => 10,
        },
        _ => {
            let treble = match (step, sharp) {
                (Step::F, true) => 17,
                (Step::C, true) => 14,
                (Step::G, true) => 18,
                (Step::D, true) => 15,
                (Step::A, true) => 12,
                (Step::E, true) => 16,
                (Step::B, true) => 13,
                (Step::B, false) => 13,
                (Step::E, false) => 16,
                (Step::A, false) => 12,
                (Step::D, false) => 15,
                (Step::G, false) => 11,
                (Step::C, false) => 14,
                (Step::F, false) => 10,
            };
            let shift = match clef {
                Clef::Treble => 0,
                Clef::TrebleEight | Clef::Alto | Clef::Tenor => -7,
                Clef::Bass => -14,
                Clef::BassEight => -21,
                Clef::Soprano => 0,
            };
            treble + shift
        }
    };
    -f64::from(diatonic - clef.middle()) / 2.0
}
