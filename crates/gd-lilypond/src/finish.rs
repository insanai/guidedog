use std::collections::BTreeMap;

use gd_score::{EventKind, Measure, Meter, Ratio};

use crate::interpret::Reader;
use crate::limits::{limit, push};
use crate::{Error, ErrorKind, Span};

impl Reader<'_> {
    pub(crate) fn finish(&mut self) -> Result<(), Error> {
        let mut end = Ratio::ZERO;
        let mut changes = BTreeMap::new();
        let mut rhythmic = false;
        for event in &self.score.events {
            self.work.take(1, event.span)?;
            if let Some(duration) = event.kind.duration() {
                rhythmic = true;
                if event.at.grace == Ratio::ZERO {
                    end = end.max(event.at.main.checked_add(duration.time()?)?);
                }
            }
            if let EventKind::Time(meter) = &event.kind
                && changes.insert(event.at.main, meter.clone()).is_some_and(|old| old != *meter)
            {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    "Different staves use different meters.",
                )
                .at(event.span)
                .hint("Use one meter for every staff at each measure boundary."));
            }
        }
        if !rhythmic || end <= Ratio::ZERO {
            return Err(Error::new(
                ErrorKind::Empty,
                "The score contains no main rhythmic music.",
            )
            .hint("Add a note, rest or spacer outside grace groups."));
        }
        self.measures(end, changes)?;
        self.rests_and_bars()?;
        self.finish_lyrics()?;
        for voice in &mut self.score.voices {
            voice.events.sort_by_key(|id| self.score.events[id.0 as usize].at);
        }
        gd_score::check_score(&self.score)
    }

    fn measures(&mut self, end: Ratio, changes: BTreeMap<Ratio, Meter>) -> Result<(), Error> {
        let mut meter = changes
            .iter()
            .next()
            .filter(|(at, _)| **at <= Ratio::ZERO)
            .map(|(_, meter)| meter.clone())
            .unwrap_or_default();
        let mut at = self.partial.map(|d| d.checked_neg()).transpose()?.unwrap_or(Ratio::ZERO);
        let mut number = if at < Ratio::ZERO { 0u32 } else { 1 };
        while at < end {
            self.work.take(1, Span::default())?;
            if self.score.measures.len() >= self.limits.events {
                return Err(limit("measure count", Span::default()));
            }
            if let Some(change) = changes.get(&at) {
                meter.clone_from(change);
            }
            let length = Ratio::new(i64::from(meter.numerator), i64::from(meter.denominator))?;
            let next = if at < Ratio::ZERO { Ratio::ZERO } else { at.checked_add(length)? };
            if changes
                .range((std::ops::Bound::Excluded(at), std::ops::Bound::Excluded(next)))
                .next()
                .is_some()
            {
                return Err(Error::new(
                    ErrorKind::Syntax,
                    "A time signature changes inside a measure.",
                )
                .hint("Move the time change to a measure boundary."));
            }
            self.memory.take(
                std::mem::size_of::<Measure>() * 2 + meter.beats.len() * 2,
                Span::default(),
            )?;
            push(
                &mut self.score.measures,
                Measure { number, start: at, end: next.min(end), meter: meter.clone() },
                Span::default(),
            )?;
            at = next;
            number =
                number.checked_add(1).ok_or_else(|| limit("measure numbers", Span::default()))?;
        }
        Ok(())
    }

    fn rests_and_bars(&mut self) -> Result<(), Error> {
        for event in &self.score.events {
            if let EventKind::MeasureRest(duration) = event.kind {
                let index = self.score.measures.partition_point(|m| m.start < event.at.main);
                let measure = self
                    .score
                    .measures
                    .get(index)
                    .filter(|m| m.start == event.at.main)
                    .ok_or_else(|| {
                        Error::new(ErrorKind::Duration, "A measure rest must start at a bar line.")
                            .at(event.span)
                    })?;
                let length = Ratio::new(
                    i64::from(measure.meter.numerator),
                    i64::from(measure.meter.denominator),
                )?;
                if !duration.time()?.checked_div(length)?.is_integer() {
                    return Err(Error::new(
                        ErrorKind::Duration,
                        "A measure rest needs a whole number of measures.",
                    )
                    .at(event.span)
                    .hint("Use R with a duration matching the meter, such as R2. in 3/4."));
                }
            }
        }
        for (at, span) in &self.bar_checks {
            let index = self.score.measures.partition_point(|m| m.start <= *at);
            let boundary = self
                .score
                .measures
                .get(index.saturating_sub(1))
                .is_some_and(|m| m.start == *at || m.end == *at);
            if !boundary {
                let measure = self.score.measures.get(index.saturating_sub(1));
                let message = if let Some(measure) = measure {
                    format!(
                        "The bar check is {} whole notes into measure {}.",
                        at.checked_sub(measure.start)?,
                        measure.number
                    )
                } else {
                    "The bar check is outside the score's measure boundaries.".into()
                };
                self.warnings.push(
                    Error::new(ErrorKind::BarCheck, message)
                        .at(*span)
                        .warning()
                        .hint("Check the durations in this measure; engraving continues."),
                );
            }
        }
        Ok(())
    }
}
