//! Coarse scratch reservations cover allocations that belong to bounded layout stages.

use std::mem::size_of;

use gd_score::{AnnotationKind, Error, EventKind, Score};

use crate::limits::{charge, failure};
use crate::{Engraver, Options};

pub(crate) fn reserve(
    workspace: &mut Engraver,
    score: &Score,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let (heads, text) = counts(score, work)?;
    let syllables = score.lyrics.iter().map(|verse| verse.syllables.len()).sum::<usize>();
    let columns = score
        .events
        .len()
        .saturating_add(score.measures.len().saturating_mul(2))
        .saturating_add(syllables.saturating_mul(2));
    let snapshots = score.measures.len().saturating_mul(score.staves.len());
    let current_retained = retained(workspace);
    let additional_retained = current_retained.saturating_sub(workspace.charged_retained);
    let bytes = columns
        .saturating_mul(256)
        .saturating_add(score.events.len().saturating_mul(32))
        .saturating_add(score.spanners.len().saturating_mul(96))
        .saturating_add(heads.saturating_mul(size_of::<crate::prepare::Head>() * 2 + 128))
        .saturating_add(snapshots.saturating_mul(size_of::<crate::prepare::StaffState>() * 2))
        .saturating_add(score.staves.len().saturating_mul(70 * 128))
        .saturating_add(text.saturating_mul(64 * 80).max(8 * 1024 * 1024))
        .saturating_add(additional_retained);
    if options.reserve.as_mut().is_some_and(|reserve| !reserve(bytes)) {
        return Err(failure("The memory ledger refused engraving scratch space."));
    }
    workspace.charged_retained = workspace.charged_retained.max(current_retained);
    Ok(())
}

fn counts(score: &Score, work: &mut usize) -> Result<(usize, usize), Error> {
    let (mut heads, mut text) = (0usize, 0usize);
    for event in &score.events {
        charge(work, 1 + event.marks.len())?;
        if let EventKind::Chord(chord) = &event.kind {
            heads += chord.notes.len();
        }
        if let EventKind::ChordName(name, _) = &event.kind {
            text = text.max(name.len());
        }
        for annotation in &event.marks {
            match &annotation.kind {
                AnnotationKind::Text(value, _)
                | AnnotationKind::Tempo(value)
                | AnnotationKind::Rehearsal(value) => text = text.max(value.len()),
                AnnotationKind::Markup(markup, _) => text = text.max(markup.approx_bytes()),
                _ => {}
            }
        }
    }
    for staff in &score.staves {
        text = text.max(staff.name.len()).max(staff.short_name.len());
        if let Some(m) = &staff.name_markup {
            text = text.max(m.approx_bytes());
        }
        if let Some(m) = &staff.short_name_markup {
            text = text.max(m.approx_bytes());
        }
    }
    for group in &score.groups {
        text = text.max(group.name.len()).max(group.short_name.len());
        for m in [&group.name_markup, &group.short_name_markup].into_iter().flatten() {
            text = text.max(m.approx_bytes());
        }
    }
    for verse in &score.lyrics {
        charge(work, verse.syllables.len())?;
        for syllable in &verse.syllables {
            text = text.max(syllable.text.len());
        }
    }
    if text > 65_536 {
        return Err(failure("Text exceeds the 65536-byte shaping bound."));
    }
    Ok((heads, text))
}

fn retained(workspace: &Engraver) -> usize {
    let mut bytes = workspace.columns.capacity() * size_of::<crate::columns::Column>()
        + workspace.springs.capacity() * size_of::<crate::Spring>()
        + workspace.breaks.capacity() * size_of::<crate::BreakPoint>()
        + workspace.positions.capacity() * size_of::<f64>()
        + (workspace.rods.capacity() + workspace.local_rods.capacity()) * size_of::<crate::Rod>()
        + workspace.plan.events.capacity() * size_of::<crate::prepare::EventInfo>()
        + workspace.plan.measures.capacity() * size_of::<Vec<crate::prepare::StaffState>>()
        + workspace.plan.beams.capacity() * size_of::<crate::beaming::BeamGroup>();
    for column in &workspace.columns {
        bytes += column.events.capacity() * size_of::<gd_score::EventId>();
    }
    for event in &workspace.plan.events {
        bytes += event.heads.capacity() * size_of::<crate::prepare::Head>();
        for head in &event.heads {
            bytes += head.dots.capacity() * size_of::<crate::Point>();
        }
    }
    for measure in &workspace.plan.measures {
        bytes += measure.capacity() * size_of::<crate::prepare::StaffState>();
    }
    for beam in &workspace.plan.beams {
        bytes += beam.events.capacity() * size_of::<gd_score::EventId>();
    }
    bytes
}
