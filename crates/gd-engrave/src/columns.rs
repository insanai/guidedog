use std::collections::BTreeMap;

use gd_score::{BreakRule, Error, EventId, EventKind, Moment, Ratio, Score};

use crate::limits::{charge, failure, push};
use crate::{BreakPoint, Options, Spring};

#[derive(Clone, Debug, Default)]
pub(crate) struct Column {
    pub(crate) at: Moment,
    pub(crate) events: Vec<EventId>,
}

pub(crate) fn collect(
    columns: &mut Vec<Column>,
    score: &Score,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    columns.clear();
    let mut moments = BTreeMap::<Moment, Vec<EventId>>::new();
    for measure in &score.measures {
        charge(work, 1)?;
        moments.entry(Moment { main: measure.start, grace: Ratio::ZERO }).or_default();
        moments.entry(Moment { main: measure.end, grace: Ratio::ZERO }).or_default();
    }
    for (id, event) in score.events.iter().enumerate() {
        charge(work, 1)?;
        push(moments.entry(event.at).or_default(), EventId(id as u32), &mut options.reserve)?;
    }
    for (at, events) in moments {
        push(columns, Column { at, events }, &mut options.reserve)?;
    }
    if columns.len() < 2 {
        return Err(failure("A score needs a positive rhythmic span."));
    }
    Ok(())
}

pub(crate) fn springs(
    out: &mut Vec<Spring>,
    columns: &[Column],
    score: &Score,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    out.clear();
    let shortest = common_shortest(score, work)?.to_f64();
    for pair in columns.windows(2) {
        charge(work, pair[0].events.len().max(1))?;
        let duration = pair[0]
            .events
            .iter()
            .filter_map(|id| score.events[id.0 as usize].kind.duration())
            .map(|d| d.time())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min()
            .unwrap_or(Ratio::new(1, 4)?)
            .to_f64();
        let delta = if pair[0].at.main == pair[1].at.main {
            pair[1].at.grace.checked_sub(pair[0].at.grace)?.to_f64()
        } else {
            pair[1].at.main.checked_sub(pair[0].at.main)?.to_f64()
        };
        let ideal = if duration >= shortest {
            1.2 * (2.0 + crate::math::log2(duration / shortest))
        } else {
            1.2 * (1.0 + duration / shortest)
        };
        push(
            out,
            Spring { ideal: (ideal * delta / duration).max(0.8), minimum: 1.8 },
            &mut options.reserve,
        )?;
    }
    Ok(())
}

pub(crate) fn breaks(
    out: &mut Vec<BreakPoint>,
    columns: &[Column],
    score: &Score,
    plan: &crate::prepare::Plan,
    fonts: &crate::Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    out.clear();
    for (index, measure) in score.measures.iter().enumerate() {
        charge(work, 1)?;
        let gap = columns.partition_point(|c| c.at.main < measure.start);
        let (prefix, indent) = preface(score, plan, fonts, index, options, work)?;
        push(
            out,
            BreakPoint { gap, measure: measure.number, prefix, indent, ..BreakPoint::default() },
            &mut options.reserve,
        )?;
    }
    push(
        out,
        BreakPoint {
            gap: columns.len() - 1,
            measure: score.measures.last().map(|m| m.number + 1).unwrap_or(1),
            forced: true,
            ..BreakPoint::default()
        },
        &mut options.reserve,
    )?;
    if let Some(first) = out.first_mut() {
        first.forced = true;
    }
    for event in &score.events {
        if let EventKind::Break(rule) = event.kind {
            let index = out.partition_point(|b| columns[b.gap].at.main < event.at.main);
            if let Some(boundary) = out.get_mut(index) {
                boundary.forced |= rule == BreakRule::Force;
                boundary.forbidden |= rule == BreakRule::Forbid;
            }
        }
    }
    for group in &plan.beams {
        if !group.manual {
            continue;
        }
        let (Some(first), Some(last)) = (group.events.first(), group.events.last()) else {
            continue;
        };
        let first = score.events[first.0 as usize].at.main;
        let last = score.events[last.0 as usize].at.main;
        for boundary in out.iter_mut() {
            charge(work, 1)?;
            let at = columns[boundary.gap].at.main;
            if at > first && at <= last {
                if boundary.forced {
                    return Err(failure("A forced line break crosses a manual beam."));
                }
                boundary.forbidden = true;
            }
        }
    }
    Ok(())
}

fn preface(
    score: &Score,
    plan: &crate::prepare::Plan,
    fonts: &crate::Fonts<'_>,
    measure: usize,
    options: &Options<'_>,
    work: &mut usize,
) -> Result<(f64, f64), Error> {
    let space = options.staff_size / 4.0;
    let mut width: f64 = 0.0;
    let mut indent: f64 = if score.groups.is_empty() { 0.0 } else { 2.1 };
    let mut warnings = Vec::new();
    for (staff_index, staff) in score.staves.iter().enumerate() {
        charge(work, 1)?;
        let name = if measure == 0 { &staff.name } else { &staff.short_name };
        if !name.is_empty() {
            let shaped = crate::text::shape(
                fonts,
                name,
                space * 2.3,
                gd_score::TextStyle::Regular,
                &mut warnings,
                work,
            )?;
            indent = indent.max(
                shaped.advance / space + 1.5 + if score.groups.is_empty() { 0.0 } else { 2.1 },
            );
        }
        if let Some(state) = plan.measures.get(measure).and_then(|m| m.get(staff_index)) {
            let key = state
                .key
                .alterations()?
                .iter()
                .filter(|a| **a != gd_score::Alter::Natural)
                .count();
            let meter = measure == 0 || score.measures[measure - 1].meter != state.meter;
            width = width.max(4.0 + key as f64 * 0.9 + if meter { 3.0 } else { 0.0 });
        }
    }
    Ok((indent + width.max(4.0) + 1.0, indent))
}

fn common_shortest(score: &Score, work: &mut usize) -> Result<Ratio, Error> {
    let mut shortest = vec![None; score.measures.len()];
    for event in &score.events {
        charge(work, 1)?;
        if !matches!(event.kind, EventKind::Chord(_) | EventKind::Rest(_)) {
            continue;
        }
        let measure =
            score.measures.partition_point(|m| m.start <= event.at.main).saturating_sub(1);
        if let Some(duration) = event.kind.duration() {
            let time = duration.time()?;
            if let Some(slot) = shortest.get_mut(measure) {
                *slot = Some(slot.unwrap_or(time).min(time));
            }
        }
    }
    let mut counts = BTreeMap::<Ratio, (usize, usize)>::new();
    for (index, time) in shortest.into_iter().enumerate() {
        if let Some(time) = time {
            let value = counts.entry(time).or_default();
            value.0 += 1;
            value.1 = index;
        }
    }
    let common = counts.into_iter().max_by_key(|(_, count)| *count).map(|(time, _)| time);
    Ok(common.unwrap_or(Ratio::new(1, 8)?).min(Ratio::new(3, 16)?))
}
