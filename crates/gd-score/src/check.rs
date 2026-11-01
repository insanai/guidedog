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
    check_voices(score)?;
    for event in &score.events {
        check_event(event).map_err(|error| error.at(event.span))?;
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
            check_anchor(score, syllable.anchor)?;
            if let Some(end) = syllable.extender {
                check_anchor(score, end)?;
            }
        }
    }
    Ok(())
}

fn check_voices(score: &Score) -> Result<(), Error> {
    let mut seen = Vec::new();
    seen.try_reserve(score.events.len())
        .map_err(|_| invalid("Cannot allocate score validation marks."))?;
    seen.resize(score.events.len(), false);
    let mut counts = [0u8; 24];
    for (index, voice) in score.voices.iter().enumerate() {
        let Some(count) = counts.get_mut(voice.staff.0 as usize) else {
            return Err(invalid("A voice names a missing staff."));
        };
        if voice.staff.0 as usize >= score.staves.len() || *count >= 2 {
            return Err(invalid("A staff must exist and may contain at most two voices."));
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
    if let Some(duration) = event.kind.duration() {
        duration.time()?;
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
    for event in &score.events {
        if let EventKind::Time(meter) = &event.kind {
            let index = score.measures.partition_point(|m| m.start < event.at.main);
            let expected = score.measures.get(index).filter(|m| m.start == event.at.main);
            if expected.is_some_and(|m| &m.meter != meter) {
                return Err(invalid("All staves must use the same meter."));
            }
        }
    }
    Ok(())
}

fn check_spanners(score: &Score) -> Result<(), Error> {
    for spanner in &score.spanners {
        let start = endpoint(score, spanner.start)?;
        let end = endpoint(score, spanner.end)?;
        if start.at > end.at || start.voice != end.voice {
            return Err(invalid("A spanner must run forward within one voice.").at(spanner.span));
        }
        if matches!(spanner.kind, SpannerKind::Tie) {
            let (EventKind::Chord(a), EventKind::Chord(b)) = (&start.kind, &end.kind) else {
                return Err(invalid("A tie needs pitched notes at both ends."));
            };
            let adjacent = if start.at.grace < Ratio::ZERO {
                start.at.main == end.at.main
                    && start.at.grace.checked_add(a.duration.time()?)? == end.at.grace
            } else {
                start.at.main.checked_add(a.duration.time()?)? == end.at.main
            };
            if a.notes[spanner.start.note as usize].pitch
                != b.notes[spanner.end.note as usize].pitch
                || !adjacent
            {
                return Err(invalid("A tie needs equal pitches at adjacent rhythmic moments."));
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
            if moment.main < Ratio::ZERO && moment.grace > Ratio::ZERO {
                return Err(invalid("A lyric moment has an invalid grace offset."));
            }
        }
    }
    Ok(())
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::Score, message).hint("Check the reader's score construction invariants.")
}
