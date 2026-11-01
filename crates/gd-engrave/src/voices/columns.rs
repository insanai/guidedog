//! Bounded placement against every preceding voice, rather than pairwise overwrites.

use gd_score::{Error, EventId, Score, VoiceStyle};

use crate::limits::{charge, failure};
use crate::prepare::{EventInfo, Plan, arrange_heads, stems};
use crate::{Fonts, Rect};

pub(super) fn arrange(
    plan: &mut Plan,
    ids: &[EventId],
    score: &Score,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let mut ordered = ids.to_vec();
    ordered.sort_by_key(|id| (shift_level(score, *id), rank(score, *id)));
    for (index, id) in ordered.iter().enumerate() {
        let info = &plan.events[id.0 as usize];
        let glyph = fonts.glyph(info.head_glyph)?;
        let bounds = fonts.glyph_data(glyph).ok_or_else(|| failure("Missing voice head."))?.bounds;
        let width = (bounds.right - bounds.left) * info.head_scale + 0.3;
        // As in LilyPond, shifted columns move away from the stem: right for upward
        // stems, left for downward ones.
        let shifted_down = shift_level(score, *id) > 0 && !info.up;
        let side = if shifted_down || rank(score, *id) == 4 { -1.0 } else { 1.0 };
        let base = inner_offset(plan, &ordered[..index], *id, score) * width;
        let mut placed = false;
        for step in 0..=16 {
            charge(work, 1)?;
            let shift = (base + f64::from(step) * width) * side;
            move_column(&mut plan.events[id.0 as usize], shift, fonts, work)?;
            let mut collision = false;
            for previous in &ordered[..index] {
                collision |= obstructs(
                    &plan.events[id.0 as usize],
                    &plan.events[previous.0 as usize],
                    (fonts, score),
                    work,
                )?;
            }
            if !collision {
                placed = true;
                break;
            }
        }
        if !placed {
            return Err(failure("Cannot separate the simultaneous voice columns."));
        }
    }
    share_heads(plan, &ordered, score, work)
}

/// A shifted column beside earlier columns with the same stem direction starts half a
/// head width aside for each lower shift level among them, as LilyPond's note
/// collisions shift it, so two stems in one direction never read as a single chord.
/// Columns with equal levels start together.
fn inner_offset(plan: &Plan, earlier: &[EventId], id: EventId, score: &Score) -> f64 {
    let info = &plan.events[id.0 as usize];
    let level = shift_level(score, id);
    let stemmed = |info: &EventInfo| stem(info).bottom > stem(info).top;
    if level == 0 || !stemmed(info) {
        return 0.0;
    }
    let mut lower = [false; 4];
    for previous in earlier {
        let other = &plan.events[previous.0 as usize];
        let other_level = shift_level(score, *previous);
        if other.up == info.up && stemmed(other) && other_level < level {
            lower[usize::from(other_level.min(3))] = true;
        }
    }
    0.5 * lower.iter().filter(|seen| **seen).count() as f64
}

/// The column's shift level: the explicit one from `\shiftOn` and its relatives, else
/// its voice policy's, where an unstyled third or fourth voice counts as an inner voice.
fn shift_level(score: &Score, id: EventId) -> u8 {
    let event = &score.events[id.0 as usize];
    event.column_shift.unwrap_or_else(|| match event.voice_style {
        Some(style) if style != VoiceStyle::Single => style.column_shift(),
        _ => u8::from(rank(score, id) >= 3),
    })
}

pub(super) fn rank(score: &Score, id: EventId) -> usize {
    let event = &score.events[id.0 as usize];
    match event.voice_style {
        Some(VoiceStyle::One) => 1,
        Some(VoiceStyle::Two) => 2,
        Some(VoiceStyle::Three) => 3,
        Some(VoiceStyle::Four) => 4,
        Some(VoiceStyle::Single) | None => {
            let staff = score.voices[event.voice.0 as usize].staff;
            score.voices[..=event.voice.0 as usize].iter().filter(|v| v.staff == staff).count()
        }
    }
}

fn move_column(
    info: &mut EventInfo,
    shift: f64,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    info.voice_shift = shift;
    arrange_heads(info, fonts, work)?;
    stems(info, fonts)?;
    crate::accidentals::arrange(info, fonts, work)
}

fn head_bounds(info: &EventInfo, fonts: &Fonts<'_>) -> Result<Rect, Error> {
    let glyph = fonts.glyph(info.head_glyph)?;
    let rect = fonts.glyph_data(glyph).ok_or_else(|| failure("Missing voice head."))?.bounds;
    let scale = info.head_scale;
    Ok(Rect {
        left: rect.left * scale,
        right: rect.right * scale,
        top: rect.top * scale,
        bottom: rect.bottom * scale,
    })
}

fn stem(info: &EventInfo) -> Rect {
    if matches!(info.head_glyph, "noteheadWhole" | "noteheadDoubleWhole") {
        return Rect::default();
    }
    Rect {
        left: info.stem_x - 0.12,
        right: info.stem_x + 0.12,
        top: info.stem_base.min(info.stem_tip),
        bottom: info.stem_base.max(info.stem_tip),
    }
}

fn compatible(a: &EventInfo, b: &EventInfo) -> bool {
    a.up != b.up && a.head_glyph == b.head_glyph && a.head_scale == b.head_scale
}

fn obstructs(
    a: &EventInfo,
    b: &EventInfo,
    context: (&Fonts<'_>, &Score),
    work: &mut usize,
) -> Result<bool, Error> {
    let (fonts, score) = context;
    let (ab, bb) = (head_bounds(a, fonts)?, head_bounds(b, fonts)?);
    for ah in &a.heads {
        for bh in &b.heads {
            charge(work, 1)?;
            let ar = ab.translated(ah.at.x, ah.at.y);
            let br = bb.translated(bh.at.x, bh.at.y);
            let shared = compatible(a, b)
                && ah.at == bh.at
                && ah.dots.len() == bh.dots.len()
                && same_pitch(score, a, b, ah, bh);
            if !shared
                && (ar.overlaps(br, 0.0) || ar.overlaps(stem(b), 0.0) || br.overlaps(stem(a), 0.0))
            {
                return Ok(true);
            }
        }
    }
    let (af, bf) = (flag(a, fonts)?, flag(b, fonts)?);
    for (info, heads, other) in [(a, ab, bf), (b, bb, af)] {
        for head in &info.heads {
            charge(work, 1)?;
            if heads.translated(head.at.x, head.at.y).overlaps(other, 0.0) {
                return Ok(true);
            }
        }
    }
    Ok(stem(a).overlaps(stem(b), 0.0)
        || af.overlaps(bf, 0.0)
        || af.overlaps(stem(b), 0.0)
        || bf.overlaps(stem(a), 0.0))
}

fn share_heads(
    plan: &mut Plan,
    ids: &[EventId],
    score: &Score,
    work: &mut usize,
) -> Result<(), Error> {
    // A shared head has at most one upward and one downward stem.
    for (index, id) in ids.iter().enumerate() {
        for previous in &ids[..index] {
            let (a, b) = (&plan.events[previous.0 as usize], &plan.events[id.0 as usize]);
            if !compatible(a, b) {
                continue;
            }
            charge(work, a.heads.len().saturating_mul(b.heads.len()))?;
            let shared: Vec<_> = b
                .heads
                .iter()
                .map(|bh| {
                    a.heads.iter().any(|ah| {
                        !ah.shared
                            && ah.at == bh.at
                            && ah.dots.len() == bh.dots.len()
                            && same_pitch(score, a, b, ah, bh)
                    })
                })
                .collect();
            for (head, shared) in plan.events[id.0 as usize].heads.iter_mut().zip(shared) {
                head.shared |= shared;
            }
        }
    }
    Ok(())
}

fn same_pitch(
    score: &Score,
    a: &EventInfo,
    b: &EventInfo,
    ah: &crate::prepare::Head,
    bh: &crate::prepare::Head,
) -> bool {
    match (&score.events[a.id.0 as usize].kind, &score.events[b.id.0 as usize].kind) {
        (gd_score::EventKind::Chord(a), gd_score::EventKind::Chord(b)) => {
            a.notes[ah.note as usize].pitch == b.notes[bh.note as usize].pitch
        }
        _ => false,
    }
}

fn flag(info: &EventInfo, fonts: &Fonts<'_>) -> Result<Rect, Error> {
    if info.flags == 0 || info.beam.is_some() {
        return Ok(Rect::default());
    }
    let glyph = fonts.glyph(crate::heads::flag(info.flags, info.up))?;
    let rect = fonts.glyph_data(glyph).ok_or_else(|| failure("Missing voice flag."))?.bounds;
    let scale = info.head_scale;
    Ok(Rect {
        left: rect.left * scale,
        right: rect.right * scale,
        top: rect.top * scale,
        bottom: rect.bottom * scale,
    }
    .translated(info.stem_x, info.stem_tip))
}
