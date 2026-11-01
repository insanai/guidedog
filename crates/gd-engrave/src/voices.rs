//! Simultaneous heads share compatible unisons and separate conflicting voice columns.

use gd_score::{Direction, Error, EventKind, Score};

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
    for (index, first) in column.events.iter().enumerate() {
        let a = &score.events[first.0 as usize];
        let EventKind::Chord(chord_a) = &a.kind else { continue };
        for second in &column.events[index + 1..] {
            charge(work, 1)?;
            let b = &score.events[second.0 as usize];
            if a.voice == b.voice
                || score.voices[a.voice.0 as usize].staff != score.voices[b.voice.0 as usize].staff
            {
                continue;
            }
            let EventKind::Chord(chord_b) = &b.kind else { continue };
            let mut collision = false;
            let compatible = chord_a.duration.base == chord_b.duration.base
                && chord_a.duration.dots == chord_b.duration.dots;
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
            if collision {
                for id in [*first, *second] {
                    for head in &mut plan.events[id.0 as usize].heads {
                        head.shared = false;
                    }
                }
                let id = if !plan.events[second.0 as usize].up { *second } else { *first };
                let info = &mut plan.events[id.0 as usize];
                info.voice_shift = 1.3;
                for head in &mut info.heads {
                    head.shared = false;
                }
                arrange_heads(info, fonts)?;
                stems(info, fonts)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn direction(score: &Score, event: &gd_score::Event) -> Direction {
    let EventKind::Chord(chord) = &event.kind else { return Direction::Auto };
    if chord.direction == Direction::Auto {
        score.voices[event.voice.0 as usize].direction
    } else {
        chord.direction
    }
}
