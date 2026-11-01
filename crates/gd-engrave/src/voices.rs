//! Simultaneous heads share compatible unisons and separate conflicting voice columns.

use std::collections::BTreeMap;

use gd_score::{Direction, Error, EventId, EventKind, Score};

use crate::Fonts;
use crate::columns::Column;
use crate::limits::charge;
use crate::prepare::{Plan, arrange_heads, stems};

pub(crate) fn arrange(
    plan: &mut Plan,
    column: &Column,
    score: &Score,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let mut by_staff: BTreeMap<usize, Vec<EventId>> = BTreeMap::new();
    for (index, first) in column.events.iter().enumerate() {
        let a = &score.events[first.0 as usize];
        let EventKind::Chord(chord_a) = &a.kind else { continue };
        let staff = score.voices[a.voice.0 as usize].staff.0 as usize;
        by_staff.entry(staff).or_default().push(*first);
        for second in &column.events[index + 1..] {
            charge(work, 1)?;
            let b = &score.events[second.0 as usize];
            if a.voice == b.voice || score.voices[b.voice.0 as usize].staff.0 as usize != staff {
                continue;
            }
            let EventKind::Chord(chord_b) = &b.kind else { continue };
            resolve_pair(plan, (*first, chord_a), (*second, chord_b), fonts, work)?;
        }
    }
    for ids in by_staff.values() {
        if ids.len() >= 2 {
            crate::dots::arrange_staff_column(plan, ids);
            crate::accidentals::arrange_staff_column(plan, ids, fonts, work)?;
        }
    }
    Ok(())
}

fn resolve_pair(
    plan: &mut Plan,
    a: (EventId, &gd_score::Chord),
    b: (EventId, &gd_score::Chord),
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let (first, chord_a) = a;
    let (second, chord_b) = b;
    let up_a = plan.events[first.0 as usize].up;
    let up_b = plan.events[second.0 as usize].up;
    let compatible = chord_a.duration.base == chord_b.duration.base
        && chord_a.duration.dots == chord_b.duration.dots
        && up_a != up_b;
    let mut collision = false;
    for (head_b, note_b) in chord_b.notes.iter().enumerate() {
        for note_a in &chord_a.notes {
            charge(work, 1)?;
            let distance = (note_a.pitch.diatonic() - note_b.pitch.diatonic()).abs();
            let shared = compatible && note_a.pitch == note_b.pitch;
            collision |= distance <= 1 && !shared;
            if shared {
                plan.events[second.0 as usize].heads[head_b].shared = true;
            }
        }
    }
    let crossed = if up_a != up_b {
        let (up_id, down_id) = if up_a { (first, second) } else { (second, first) };
        let up_low = plan.events[up_id.0 as usize]
            .heads
            .iter()
            .map(|h| h.at.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let down_high = plan.events[down_id.0 as usize]
            .heads
            .iter()
            .map(|h| h.at.y)
            .fold(f64::INFINITY, f64::min);
        let strictly_crossed = up_low > down_high + 0.01;
        collision |= strictly_crossed;
        strictly_crossed
    } else {
        let a_bounds = plan.events[first.0 as usize].bounds;
        let b_bounds = plan.events[second.0 as usize].bounds;
        collision |=
            a_bounds.bottom >= b_bounds.top - 0.1 && b_bounds.bottom >= a_bounds.top - 0.1;
        false
    };
    if !collision {
        return Ok(());
    }
    for id in [first, second] {
        for head in &mut plan.events[id.0 as usize].heads {
            head.shared = false;
        }
    }
    let shift_id = if up_a != up_b {
        let (up_id, down_id) = if up_a { (first, second) } else { (second, first) };
        if crossed { up_id } else { down_id }
    } else {
        second
    };
    let info = &mut plan.events[shift_id.0 as usize];
    let glyph = fonts.glyph(info.head_glyph)?;
    info.voice_shift = fonts
        .glyph_data(glyph)
        .map(|data| (data.bounds.right - data.bounds.left + 0.16) * info.head_scale)
        .unwrap_or(1.34);
    arrange_heads(info, fonts, work)?;
    stems(info, fonts)?;
    crate::accidentals::arrange(info, fonts, work)?;
    Ok(())
}

pub(crate) fn direction(score: &Score, event: &gd_score::Event) -> Direction {
    let EventKind::Chord(chord) = &event.kind else { return Direction::Auto };
    let direction = if chord.direction == Direction::Auto {
        score.voices[event.voice.0 as usize].direction
    } else {
        chord.direction
    };
    // Grace notes point up in a single voice, as LilyPond's grace settings make them.
    if direction == Direction::Auto && event.at.grace < gd_score::Ratio::ZERO {
        Direction::Up
    } else {
        direction
    }
}
