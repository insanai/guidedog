use crate::{
    Anchor, Error, ErrorKind, Event, EventKind, Meter, NoteRef, Ratio, Score, SpannerKind,
};

/// Borrows a score and checks reader-independent invariants.
/// Visits events and references once, with logarithmic measure lookups for meter changes.
/// Returns the first invalid reference, duration, ordering, pitch, meter or interval.
pub fn check_score(score: &Score) -> Result<(), Error> {
    if score.staves.is_empty() || score.voices.is_empty() || score.events.is_empty() {
        return Err(invalid("A score needs a staff, a voice and a rhythmic event."));
    }
    if !score.events.iter().any(|event| event.kind.duration().is_some()) {
        return Err(invalid("A score needs at least one rhythmic event."));
    }
    if score.staves.len() > 24 {
        return Err(invalid("A score may contain at most 24 staves."));
    }
    for staff in &score.staves {
        if let Some(markup) = &staff.name_markup {
            markup.check(0)?;
        }
        if let Some(markup) = &staff.short_name_markup {
            markup.check(0)?;
        }
    }
    check_voices(score)?;
    check_simultaneous_voices(score)?;
    for event in &score.events {
        check_event(event).map_err(|error| error.or_at(event.span))?;
    }
    for group in &score.groups {
        if group.first.0 > group.last.0 || group.last.0 as usize >= score.staves.len() {
            return Err(invalid("A staff group names an invalid staff range."));
        }
    }
    check_measures(score)?;
    check_spanners(score)?;
    for verse in &score.lyrics {
        if verse.voice.0 as usize >= score.voices.len() {
            return Err(invalid("A lyric verse names a missing voice."));
        }
        for syllable in &verse.syllables {
            check_anchor(score, syllable.anchor).map_err(|e| e.or_at(syllable.span))?;
            if let Some(end) = syllable.extender {
                check_anchor(score, end).map_err(|e| e.or_at(syllable.span))?;
            }
        }
    }
    Ok(())
}

/// At most four voices of a staff may sound at once. A voice sounds from its first event
/// to the end of its last; a sweep over those spans finds the most that overlap.
fn check_simultaneous_voices(score: &Score) -> Result<(), Error> {
    let mut spans = Vec::new();
    spans
        .try_reserve(score.voices.len().saturating_mul(2))
        .map_err(|_| invalid("Cannot allocate score validation spans."))?;
    for voice in &score.voices {
        let (Some(first), Some(last)) = (voice.events.first(), voice.events.last()) else {
            continue;
        };
        let (Some(first), Some(last)) =
            (score.events.get(first.0 as usize), score.events.get(last.0 as usize))
        else {
            return Err(invalid("A voice names a missing event."));
        };
        let time = last.kind.duration().map(|d| d.time()).transpose()?.unwrap_or(Ratio::ZERO);
        let end = last.at.main.checked_add(time)?;
        if end > first.at.main {
            // Ends sort before starts at the same moment, so a voice that ends where
            // another begins does not overlap it.
            spans.push((voice.staff, end, 0i32));
            spans.push((voice.staff, first.at.main, 1i32));
        }
    }
    spans.sort();
    let mut sounding = [0i32; 24];
    for (staff, _, delta) in spans {
        let Some(count) = sounding.get_mut(staff.0 as usize) else {
            return Err(invalid("A voice names a missing staff."));
        };
        *count += if delta == 1 { 1 } else { -1 };
        if *count > 4 {
            return Err(invalid("At most four voices of a staff may sound at once."));
        }
    }
    Ok(())
}

fn check_voices(score: &Score) -> Result<(), Error> {
    let mut seen = Vec::new();
    seen.try_reserve(score.events.len())
        .map_err(|_| invalid("Cannot allocate score validation marks."))?;
    seen.resize(score.events.len(), false);
    let mut counts = [0usize; 24];
    for (index, voice) in score.voices.iter().enumerate() {
        let Some(count) = counts.get_mut(voice.staff.0 as usize) else {
            return Err(invalid("A voice names a missing staff."));
        };
        if voice.staff.0 as usize >= score.staves.len() || *count >= crate::MAX_STAFF_VOICES {
            return Err(invalid("A staff must exist and hold a bounded number of voices."));
        }
        *count += 1;
        let mut previous = None;
        for id in &voice.events {
            let Some(event) = score.events.get(id.0 as usize) else {
                return Err(invalid("A voice names a missing event."));
            };
            if event.voice.0 as usize != index || seen[id.0 as usize] {
                return Err(invalid("An event must belong to exactly one owning voice."));
            }
            seen[id.0 as usize] = true;
            if previous.is_some_and(|at| event.at < at) {
                return Err(invalid("Voice events must be ordered by exact moment."));
            }
            previous = Some(event.at);
        }
    }
    if seen.contains(&false) {
        return Err(invalid("An event is absent from its voice's event list."));
    }
    Ok(())
}

fn check_event(event: &Event) -> Result<(), Error> {
    if event.at.grace > Ratio::ZERO {
        return Err(invalid("An event cannot have a positive grace offset."));
    }
    if let Some(duration) = event.kind.duration() {
        duration.time()?;
    }
    for mark in &event.marks {
        if let crate::AnnotationKind::Markup(markup, _) = &mark.kind {
            markup.check(0)?;
        }
    }
    match &event.kind {
        EventKind::Chord(chord) => {
            if chord.notes.is_empty() || chord.notes.len() > 16 {
                return Err(invalid("A chord must have between one and sixteen notes."));
            }
            for note in &chord.notes {
                note.pitch.check_range()?;
            }
        }
        EventKind::Key(key) => {
            key.alterations()?;
        }
        EventKind::Time(meter) => check_meter(meter)?,
        EventKind::Rest(_)
        | EventKind::Skip(_)
        | EventKind::MeasureRest(_)
        | EventKind::Clef(_)
        | EventKind::Bar(_)
        | EventKind::Break(_)
        | EventKind::ChordName(_, _) => {}
    }
    Ok(())
}

fn check_meter(meter: &Meter) -> Result<(), Error> {
    if meter.numerator == 0
        || meter.numerator > 128
        || !meter.denominator.is_power_of_two()
        || meter.denominator > 128
        || meter.beats.len() > 128
        || (!meter.beats.is_empty()
            && (meter.beats.contains(&0)
                || meter.beats.iter().map(|n| u32::from(*n)).sum::<u32>()
                    != u32::from(meter.numerator)))
    {
        return Err(invalid("A meter needs a positive numerator and a power-of-two denominator."));
    }
    Ok(())
}

fn check_measures(score: &Score) -> Result<(), Error> {
    let mut previous = None;
    for measure in &score.measures {
        check_meter(&measure.meter)?;
        if measure.start >= measure.end || previous.is_some_and(|end| end != measure.start) {
            return Err(invalid(
                "Measures must be positive, contiguous and chronologically ordered.",
            ));
        }
        previous = Some(measure.end);
    }
    if score.measures.is_empty() {
        return Ok(());
    }
    for event in &score.events {
        if let EventKind::Time(meter) = &event.kind {
            let index = score.measures.partition_point(|m| m.start < event.at.main);
            let Some(expected) = score.measures.get(index).filter(|m| m.start == event.at.main)
            else {
                return Err(
                    invalid("A meter change must fall on a measure boundary.").or_at(event.span)
                );
            };
            if &expected.meter != meter {
                return Err(invalid("All staves must use the same meter.").or_at(event.span));
            }
        }
        if let EventKind::MeasureRest(duration) = &event.kind {
            if event.at.grace != Ratio::ZERO {
                return Err(invalid("A measure rest cannot be a grace event.").or_at(event.span));
            }
            let end = event.at.main.checked_add(duration.time()?)?;
            let index = score.measures.partition_point(|m| m.start < event.at.main);
            if score.measures.get(index).is_none_or(|m| m.start != event.at.main)
                || !score.measures[index..].iter().any(|m| m.end == end)
            {
                return Err(invalid("A measure rest must align with measure boundaries.")
                    .or_at(event.span));
            }
        }
    }
    Ok(())
}

fn check_spanners(score: &Score) -> Result<(), Error> {
    for spanner in &score.spanners {
        let start = endpoint(score, spanner.start).map_err(|e| e.or_at(spanner.span))?;
        let end = endpoint(score, spanner.end).map_err(|e| e.or_at(spanner.span))?;
        if start.at > end.at || start.voice != end.voice {
            return Err(
                invalid("A spanner must run forward within one voice.").or_at(spanner.span)
            );
        }
        if matches!(spanner.kind, SpannerKind::Tie) {
            let (EventKind::Chord(a), EventKind::Chord(b)) = (&start.kind, &end.kind) else {
                return Err(invalid("A tie needs pitched notes at both ends.").or_at(spanner.span));
            };
            let step = a.duration.time().map_err(|e| e.or_at(spanner.span))?;
            let adjacent = if start.at.grace < Ratio::ZERO {
                start.at.main == end.at.main && start.at.grace.checked_add(step)? == end.at.grace
            } else {
                end.at.grace == Ratio::ZERO && start.at.main.checked_add(step)? == end.at.main
            };
            if a.notes[spanner.start.note as usize].pitch
                != b.notes[spanner.end.note as usize].pitch
                || !adjacent
            {
                return Err(invalid("A tie needs equal pitches at adjacent rhythmic moments.")
                    .or_at(spanner.span));
            }
        }
    }
    Ok(())
}

fn endpoint(score: &Score, reference: NoteRef) -> Result<&Event, Error> {
    let event = score
        .events
        .get(reference.event.0 as usize)
        .ok_or_else(|| invalid("A spanner names a missing event."))?;
    let count = match &event.kind {
        EventKind::Chord(chord) => chord.notes.len(),
        _ => 1,
    };
    if reference.note as usize >= count {
        return Err(invalid("A spanner names a missing chord head."));
    }
    Ok(event)
}

fn check_anchor(score: &Score, anchor: Anchor) -> Result<(), Error> {
    match anchor {
        Anchor::Note(reference) => {
            if !matches!(endpoint(score, reference)?.kind, EventKind::Chord(_)) {
                return Err(invalid("A lyric note anchor must name a pitched event."));
            }
        }
        Anchor::Moment(moment) => {
            if moment.grace > Ratio::ZERO {
                return Err(invalid("A lyric moment has an invalid grace offset."));
            }
        }
    }
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::Score, message).hint("Check the reader's score construction invariants.")
}
