use gd_score::{Alter, Clef, Error, Key, Meter, Step};

use crate::drawing::{Draw, Request};
use crate::prepare::accidental;
use crate::{Point, Role};

impl Draw<'_, '_, '_, '_, '_> {
    pub(crate) fn signatures(
        &mut self,
        request: &Request<'_, '_>,
        middle: f64,
    ) -> Result<(), Error> {
        let score = request.score;
        let measure = score.measures.iter().position(|m| m.number == request.first).unwrap_or(0);
        let Some(state) =
            request.plan.measures.get(measure).and_then(|m| m.get(self.staff.0 as usize))
        else {
            return Ok(());
        };
        let space = self.options.staff_size / 4.0;
        let x = request.indent + 3.5 * space;
        let width = self.key_signature(state.key, state.clef, Point { x, y: middle }, space)?;
        let meter_changed = measure == 0 || score.measures[measure - 1].meter != state.meter;
        if meter_changed {
            self.time_signature(
                &state.meter,
                Point { x: x + width + 0.5 * space, y: middle },
                space,
            )?;
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
                let y = signature_y(*step, alter.semitones() > 0, clef);
                self.glyph(
                    accidental(alter),
                    at.x + offset,
                    at.y + y * size,
                    size,
                    Role::Signature,
                )?;
                offset += 0.9 * size;
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
                offset += 0.8 * size;
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
        let numerator = meter.numerator.to_string();
        let denominator = meter.denominator.to_string();
        let width = numerator.len().max(denominator.len()) as f64 * 0.7 * size;
        self.number(
            &numerator,
            "timeSig",
            Point { x: at.x + width / 2.0, y: at.y - size },
            size,
            Role::Signature,
        )?;
        self.number(
            &denominator,
            "timeSig",
            Point { x: at.x + width / 2.0, y: at.y + size },
            size,
            Role::Signature,
        )
    }
}

fn signature_y(step: Step, sharp: bool, clef: Clef) -> f64 {
    let index = match (step, sharp) {
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
        Clef::Bass => -14,
        Clef::Treble => 0,
        Clef::TrebleEight => -7,
        Clef::BassEight => -21,
        Clef::Alto | Clef::Tenor => -7,
        Clef::Soprano => 0,
    };
    -f64::from(index + shift - clef.middle()) / 2.0
}
