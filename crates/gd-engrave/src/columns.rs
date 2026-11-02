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
    for verse in &score.lyrics {
        for syllable in &verse.syllables {
            charge(work, 1)?;
            for anchor in [Some(syllable.anchor), syllable.extender].into_iter().flatten() {
                if let gd_score::Anchor::Moment(at) = anchor {
                    moments.entry(at).or_default();
                }
            }
        }
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
    plan: &crate::prepare::Plan,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    out.clear();
    let shortest = common_shortest(score, work)?.to_f64();
    for pair in columns.windows(2) {
        charge(work, pair[0].events.len().max(1))?;
        if let Some((_, _, count)) = crate::multirests::span(score, pair[0].at.main, work)? {
            let width = 8.0 / count as f64;
            push(out, Spring { ideal: width, minimum: width }, &mut options.reserve)?;
            continue;
        }
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
        let direction = |column: &Column| {
            column.events.iter().find_map(|id| {
                let info = &plan.events[id.0 as usize];
                (!info.heads.is_empty()).then_some(info.up)
            })
        };
        let optical = match (direction(&pair[0]), direction(&pair[1])) {
            (Some(true), Some(false)) => options.optical_adjustment,
            (Some(false), Some(true)) => -options.optical_adjustment,
            _ => 0.0,
        };
        let ideal = duration_space(duration, shortest);
        push(
            out,
            Spring { ideal: (ideal * delta / duration + optical).max(0.8), minimum: 1.8 },
            &mut options.reserve,
        )?;
    }
    Ok(())
}

pub(crate) fn duration_space(duration: f64, shortest: f64) -> f64 {
    if duration >= shortest {
        1.2 * (2.0 + crate::math::log2(duration / shortest))
    } else {
        1.2 * (1.0 + duration / shortest)
    }
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
        let (prefix, indent) = preface(score, columns, plan, fonts, index, options, work)?;
        push(
            out,
            BreakPoint {
                gap,
                measure: measure.number,
                prefix,
                indent,
                suffix: if index == 0 {
                    0.0
                } else {
                    crate::courtesy::width(score, plan, fonts, measure.start, work)?
                },
                ..BreakPoint::default()
            },
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
    break_commands(out, columns, score, work)?;
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

pub(crate) fn preface_widths(
    score: &Score,
    columns: &[Column],
    plan: &crate::prepare::Plan,
    fonts: &crate::Fonts<'_>,
    measure: usize,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(f64, f64, f64), Error> {
    let mut sig_width: f64 = 0.0;
    let indent = name_indent(score, fonts, measure == 0, options, work)?;
    for staff_index in 0..score.staves.len() {
        charge(work, 1)?;
        if score.staves[staff_index].kind == gd_score::StaffKind::Tablature {
            continue;
        }
        if let Some(state) = plan.measures.get(measure).and_then(|m| m.get(staff_index)) {
            let key_w = crate::signatures::key_width(fonts, state.key)?;
            let meter_changed = measure == 0 || score.measures[measure - 1].meter != state.meter;
            let meter_w = if meter_changed {
                let gap = if key_w > 0.0 { 0.5 } else { 0.0 };
                gap + crate::signatures::meter_width(fonts, &state.meter)?
            } else {
                0.0
            };
            sig_width = sig_width.max(4.2 + key_w + meter_w);
        }
    }
    let gap = columns.partition_point(|c| c.at.main < score.measures[measure].start);
    let open_bar = columns.get(gap).and_then(|col| {
        (0..score.staves.len())
            .filter_map(|staff| crate::bars::opening_bar(score, col, staff, measure == 0))
            .map(crate::bars::style_extent)
            .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
    });
    let sig_end = indent + sig_width.max(3.8);
    let preface_end = match open_bar {
        Some((left, right)) => sig_end + 0.6 - left + right,
        None => sig_end,
    };
    let leading = leading_ink(score, plan, fonts, measure, work)?;
    Ok((preface_end + 1.0 + leading, indent, preface_end))
}

/// The indent, in staff spaces, that instrument names and group names need. A group's
/// name stands left of its staves' names, so their widths add up.
fn name_indent(
    score: &Score,
    fonts: &crate::Fonts<'_>,
    first_line: bool,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<f64, Error> {
    use crate::labels::{label_advance, line_label};
    let space = options.staff_size / 4.0;
    let brace = if score.groups.is_empty() { 0.0 } else { 2.1 };
    let mut advances = Vec::new();
    for staff in &score.staves {
        charge(work, 1)?;
        let label = line_label(
            first_line,
            (&staff.name, staff.name_markup.as_ref()),
            (&staff.short_name, staff.short_name_markup.as_ref()),
        );
        let advance = label_advance(fonts, label, space, work, &mut options.reserve)?;
        push(&mut advances, advance, &mut options.reserve)?;
    }
    let mut widest = advances.iter().copied().fold(0.0, f64::max);
    for group in &score.groups {
        let label = line_label(
            first_line,
            (&group.name, group.name_markup.as_ref()),
            (&group.short_name, group.short_name_markup.as_ref()),
        );
        let own = label_advance(fonts, label, space, work, &mut options.reserve)?;
        if own > 0.0 {
            let members = (group.first.0..=group.last.0)
                .filter_map(|s| advances.get(s as usize).copied())
                .fold(0.0, f64::max);
            widest = widest.max(own + if members > 0.0 { members + space } else { 0.0 });
        }
    }
    Ok(if widest > 0.0 { widest / space + 1.5 + brace } else { brace })
}

fn preface(
    score: &Score,
    columns: &[Column],
    plan: &crate::prepare::Plan,
    fonts: &crate::Fonts<'_>,
    measure: usize,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(f64, f64), Error> {
    let (prefix, indent, _) = preface_widths(score, columns, plan, fonts, measure, options, work)?;
    Ok((prefix, indent))
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

fn leading_ink(
    score: &Score,
    plan: &crate::prepare::Plan,
    fonts: &crate::Fonts<'_>,
    measure: usize,
    work: &mut usize,
) -> Result<f64, Error> {
    let start = score.measures[measure].start;
    let mut left: f64 = 0.0;
    for event in &plan.events {
        charge(work, 1)?;
        if score.events[event.id.0 as usize].at.main != start {
            continue;
        }
        for head in &event.heads {
            left = left.min(head.at.x);
            if (-head.at.y * 2.0).round().abs() >= 6.0 {
                left = left.min(head.at.x - 0.4 * event.head_scale);
            }
            if let Some(name) = head.accidental {
                let glyph = fonts.glyph(name)?;
                if let Some(data) = fonts.glyph_data(glyph) {
                    left = left.min(
                        head.accidental_x + data.bounds.left * event.head_scale
                            - if head.cautionary { 0.72 * event.head_scale } else { 0.0 },
                    );
                }
            }
        }
        for fret in &event.frets {
            if !fret.continuation || fret.at_bar {
                left = left.min(crate::tablature::CENTER - fret.width / 2.0);
            }
        }
    }
    Ok(-left)
}

fn break_commands(
    out: &mut [BreakPoint],
    columns: &[Column],
    score: &Score,
    work: &mut usize,
) -> Result<(), Error> {
    for event in &score.events {
        charge(work, 1)?;
        if matches!(
            event.kind,
            EventKind::Key(_)
                | EventKind::Time(_)
                | EventKind::Bar(
                    gd_score::BarStyle::Double
                        | gd_score::BarStyle::Final
                        | gd_score::BarStyle::RepeatStart
                        | gd_score::BarStyle::RepeatEnd
                        | gd_score::BarStyle::RepeatBoth
                )
        ) {
            let index = out.partition_point(|b| columns[b.gap].at.main < event.at.main);
            if let Some(boundary) = out.get_mut(index) {
                boundary.penalty = -20;
            }
        }
        if let EventKind::Break(rule) = event.kind {
            let index = out.partition_point(|b| columns[b.gap].at.main < event.at.main);
            if let Some(boundary) = out.get_mut(index) {
                boundary.forced |= rule == BreakRule::Force;
                boundary.forbidden |= rule == BreakRule::Forbid;
            }
        }
    }
    Ok(())
}
