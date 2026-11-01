use gd_score::{Direction, Error, EventId, EventKind, Ratio, Score, SpannerKind};

use crate::limits::{charge, push};
use crate::prepare::{Plan, arrange_heads, stems};
use crate::{Fonts, Options};

#[derive(Clone, Debug, Default)]
pub(crate) struct BeamGroup {
    pub(crate) events: Vec<EventId>,
    pub(crate) up: bool,
    pub(crate) manual: bool,
}

pub(crate) fn groups(
    plan: &mut Plan,
    score: &Score,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    plan.beams.clear();
    tuplets(plan, score, work)?;
    for spanner in &score.spanners {
        if spanner.kind != SpannerKind::Beam {
            continue;
        }
        let start = &score.events[spanner.start.event.0 as usize];
        let end = &score.events[spanner.end.event.0 as usize];
        let mut events = Vec::new();
        for id in &score.voices[start.voice.0 as usize].events {
            charge(work, 1)?;
            let event = &score.events[id.0 as usize];
            if event.at >= start.at
                && event.at <= end.at
                && matches!(event.kind, EventKind::Chord(_))
            {
                push(&mut events, *id, &mut options.reserve)?;
            }
        }
        add(plan, score, events, true, fonts, options, work)?;
    }
    for voice in &score.voices {
        automatic(plan, score, voice, fonts, options, work)?;
    }
    Ok(())
}

fn automatic(
    plan: &mut Plan,
    score: &Score,
    voice: &gd_score::Voice,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let fastest = fastest(score, voice, work)?;
    let mut events = Vec::new();
    let mut previous = None;
    for id in &voice.events {
        charge(work, 1)?;
        let event = &score.events[id.0 as usize];
        let measure =
            score.measures.partition_point(|m| m.start <= event.at.main).saturating_sub(1);
        let beat = beat_key(score, *id, fastest[measure])?;
        let info = &mut plan.events[id.0 as usize];
        info.beat = beat;
        let eligible = matches!(&event.kind, EventKind::Chord(chord) if chord.auto_beam)
            && info.flags > 0
            && info.beam.is_none();
        let key = if eligible { Some((beat, info.tuplet)) } else { None };
        if key != previous || !eligible {
            add(plan, score, std::mem::take(&mut events), false, fonts, options, work)?;
        }
        if eligible {
            push(&mut events, *id, &mut options.reserve)?;
        }
        previous = key;
    }
    add(plan, score, events, false, fonts, options, work)?;
    Ok(())
}

fn add(
    plan: &mut Plan,
    score: &Score,
    events: Vec<EventId>,
    manual: bool,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    if events.len() < 2 {
        return Ok(());
    }
    let mut above = 0usize;
    let mut below = 0usize;
    let mut top: f64 = 0.0;
    let mut bottom: f64 = 0.0;
    for id in &events {
        for head in &plan.events[id.0 as usize].heads {
            top = top.min(head.at.y);
            bottom = bottom.max(head.at.y);
            above += usize::from(head.at.y < 0.0);
            below += usize::from(head.at.y > 0.0);
        }
    }
    let automatic = bottom > -top || (bottom == -top && below > above);
    let up = events
        .iter()
        .find_map(|id| match crate::voices::direction(score, &score.events[id.0 as usize]) {
            Direction::Up => Some(true),
            Direction::Down => Some(false),
            Direction::Auto => None,
        })
        .unwrap_or(automatic);
    let index = plan.beams.len();
    for id in &events {
        let info = &mut plan.events[id.0 as usize];
        info.beam = Some(index);
        if let EventKind::Chord(_) = &score.events[id.0 as usize].kind {
            info.up = match crate::voices::direction(score, &score.events[id.0 as usize]) {
                Direction::Up => true,
                Direction::Down => false,
                Direction::Auto => up,
            };
        }
        arrange_heads(info, fonts, work)?;
        stems(info, fonts)?;
    }
    push(&mut plan.beams, BeamGroup { events, up, manual }, &mut options.reserve)
}

fn beat_key(score: &Score, id: EventId, fastest: u16) -> Result<(usize, i64, i64), Error> {
    let event = &score.events[id.0 as usize];
    let measure = score.measures.partition_point(|m| m.start <= event.at.main).saturating_sub(1);
    if event.at.grace < Ratio::ZERO {
        return Ok((measure, event.at.main.numerator(), -event.at.main.denominator()));
    }
    let meter = &score.measures[measure].meter;
    let position = event.at.main.checked_sub(score.measures[measure].start)?;
    let EventKind::Chord(_) = &event.kind else {
        return Ok((measure, 0, 0));
    };
    let eighths = fastest <= 8;
    let beats = if !meter.beats.is_empty() {
        meter.beats.clone()
    } else if (meter.numerator, meter.denominator) == (3, 8)
        || ((meter.numerator, meter.denominator) == (3, 4) && eighths)
    {
        vec![meter.numerator]
    } else if matches!((meter.numerator, meter.denominator), (4, 4) | (2, 2)) && eighths {
        vec![meter.numerator / 2; 2]
    } else if meter.numerator >= 6 && meter.numerator.is_multiple_of(3) {
        vec![3; usize::from(meter.numerator / 3)]
    } else if (meter.numerator, meter.denominator) == (5, 8) {
        vec![3, 2]
    } else if (meter.numerator, meter.denominator) == (8, 8) {
        vec![3, 3, 2]
    } else {
        vec![1; usize::from(meter.numerator)]
    };
    let mut end = Ratio::ZERO;
    for (index, beat) in beats.iter().enumerate() {
        end = end.checked_add(Ratio::new(i64::from(*beat), i64::from(meter.denominator))?)?;
        if position < end {
            return Ok((measure, index as i64, i64::from(meter.denominator)));
        }
    }
    Ok((measure, beats.len() as i64, i64::from(meter.denominator)))
}

fn fastest(score: &Score, voice: &gd_score::Voice, work: &mut usize) -> Result<Vec<u16>, Error> {
    charge(work, score.measures.len())?;
    let mut values = vec![0; score.measures.len()];
    for id in &voice.events {
        charge(work, 1)?;
        let event = &score.events[id.0 as usize];
        if event.at.grace < Ratio::ZERO {
            continue;
        }
        if let EventKind::Chord(chord) = &event.kind {
            let measure =
                score.measures.partition_point(|m| m.start <= event.at.main).saturating_sub(1);
            values[measure] = values[measure].max(chord.duration.base.denominator());
        }
    }
    Ok(values)
}

fn tuplets(plan: &mut Plan, score: &Score, work: &mut usize) -> Result<(), Error> {
    for (index, spanner) in score.spanners.iter().enumerate() {
        if !matches!(spanner.kind, SpannerKind::Tuplet(_, _)) {
            continue;
        }
        let start = &score.events[spanner.start.event.0 as usize];
        let end = &score.events[spanner.end.event.0 as usize];
        let voice = &score.voices[start.voice.0 as usize];
        let first = voice.events.partition_point(|id| score.events[id.0 as usize].at < start.at);
        let last = voice.events.partition_point(|id| score.events[id.0 as usize].at <= end.at);
        for id in &voice.events[first..last] {
            charge(work, 1)?;
            let slot = &mut plan.events[id.0 as usize].tuplet;
            let replace = slot.is_none_or(|old| {
                let previous = &score.spanners[old];
                score.events[previous.start.event.0 as usize].at <= start.at
                    && score.events[previous.end.event.0 as usize].at >= end.at
            });
            if replace {
                *slot = Some(index);
            }
        }
    }
    Ok(())
}
