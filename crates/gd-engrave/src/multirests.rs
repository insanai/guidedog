//! A completely silent measure span has one fixed ideal width across every voice.

use gd_score::{Error, EventKind, Ratio, Score};

use crate::limits::charge;

pub(crate) fn span(
    score: &Score,
    at: Ratio,
    work: &mut usize,
) -> Result<Option<(Ratio, Ratio, usize)>, Error> {
    let mut start = None;
    let mut end = None;
    for voice in &score.voices {
        let last = voice.events.partition_point(|id| score.events[id.0 as usize].at.main <= at);
        let mut rest = None;
        for id in voice.events[..last].iter().rev() {
            charge(work, 1)?;
            let event = &score.events[id.0 as usize];
            if let EventKind::MeasureRest(duration) = event.kind {
                let finish = event.at.main.checked_add(duration.time()?)?;
                if finish > at {
                    rest = Some((event.at.main, finish));
                }
                break;
            } else if event.kind.duration().is_some() {
                break;
            }
        }
        let Some((first, last)) = rest else { return Ok(None) };
        start = Some(start.map_or(first, |old: Ratio| old.max(first)));
        end = Some(end.map_or(last, |old: Ratio| old.min(last)));
    }
    let (Some(start), Some(end)) = (start, end) else { return Ok(None) };
    let first = score.measures.partition_point(|m| m.start < start);
    let last = score.measures.partition_point(|m| m.end <= end);
    Ok((last > first + 1).then_some((start, end, last - first)))
}
