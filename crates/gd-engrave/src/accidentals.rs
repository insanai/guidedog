//! Greedy outside-in accidental columns use actual glyph ink and cautionary padding.

use gd_score::{Error, EventId};

use crate::Fonts;
use crate::prepare::{EventInfo, Plan};
use crate::rods::{Bands, add_glyph};

pub(crate) const CAUTIONARY_PAD: f64 = 0.72;

pub(crate) fn arrange(
    info: &mut EventInfo,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let mut order: Vec<_> = info
        .heads
        .iter()
        .enumerate()
        .filter_map(|(i, head)| (!head.shared && head.accidental.is_some()).then_some(i))
        .collect();
    order.sort_by(|&a, &b| info.heads[a].at.y.total_cmp(&info.heads[b].at.y));
    let outside = outside_in(order);
    let mut occupied: Vec<Bands> = Vec::new();
    for index in outside {
        let base_left = info.bounds.left;
        let head = &mut info.heads[index];
        head.accidental_x =
            place_head(head, info.head_scale, base_left, &mut occupied, fonts, work)?;
    }
    Ok(())
}

pub(crate) fn arrange_staff_column(
    plan: &mut Plan,
    ids: &[EventId],
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let mut base_left = f64::INFINITY;
    let mut items = Vec::new();
    for &id in ids {
        let info = &plan.events[id.0 as usize];
        base_left = base_left.min(info.bounds.left);
        for (idx, head) in info.heads.iter().enumerate() {
            if head.at.y.abs() >= 3.0 {
                base_left = base_left.min(head.at.x - 0.4 * info.head_scale);
            }
            if !head.shared && head.accidental.is_some() {
                items.push((id, idx, head.at.y));
            }
        }
    }
    if !base_left.is_finite() {
        return Ok(());
    }
    items.sort_by(|a, b| a.2.total_cmp(&b.2));
    // Voices that sound one pitch with one accidental print it once, as LilyPond's
    // accidental placement merges them; the column's accidentals stand left of every head.
    let mut kept: Vec<(EventId, usize, f64)> = Vec::new();
    for (id, idx, y) in items {
        crate::limits::charge(work, kept.len())?;
        let name = plan.events[id.0 as usize].heads[idx].accidental;
        let duplicate = kept.iter().any(|(other, other_idx, other_y)| {
            *other_y == y && plan.events[other.0 as usize].heads[*other_idx].accidental == name
        });
        if duplicate {
            plan.events[id.0 as usize].heads[idx].accidental = None;
        } else {
            kept.push((id, idx, y));
        }
    }
    let outside = outside_in(kept);
    let mut occupied: Vec<Bands> = Vec::new();
    for (id, idx, _) in outside {
        let info = &mut plan.events[id.0 as usize];
        let scale = info.head_scale;
        let head = &mut info.heads[idx];
        head.accidental_x = place_head(head, scale, base_left, &mut occupied, fonts, work)?;
    }
    Ok(())
}

fn outside_in<T>(mut sorted: Vec<T>) -> Vec<T> {
    let mut out = Vec::with_capacity(sorted.len());
    while !sorted.is_empty() {
        out.push(sorted.remove(0));
        if let Some(last) = sorted.pop() {
            out.push(last);
        }
    }
    out
}

fn place_head(
    head: &crate::prepare::Head,
    size: f64,
    base_left: f64,
    occupied: &mut Vec<Bands>,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<f64, Error> {
    let Some(name) = head.accidental else { return Ok(-1.2) };
    let glyph = fonts.glyph(name)?;
    let Some(data) = fonts.glyph_data(glyph) else { return Ok(-1.2) };
    let padding = if head.cautionary { CAUTIONARY_PAD * size } else { 0.0 };
    let ledger_left = if head.at.y.abs() >= 3.0 { head.at.x - 0.4 * size } else { base_left };
    let mut x = base_left.min(ledger_left) - 0.3 - data.bounds.right * size - padding;
    let mut ink = Bands::new();
    add_glyph(&mut ink, fonts, name, size, 0.0, head.at.y, work)?;
    if head.cautionary {
        for span in ink.values_mut() {
            span.0 = data.bounds.left * size - padding;
            span.1 = data.bounds.right * size + padding;
        }
    }
    for previous in &*occupied {
        for (band, (left, right)) in &ink {
            crate::limits::charge(work, 3)?;
            for neighbor in band - 1..=band + 1 {
                if let Some((edge, far)) = previous.get(&neighbor)
                    && right + x + 0.2 > *edge
                    && left + x < *far + 0.2
                {
                    x = x.min(edge - right - 0.2);
                }
            }
        }
    }
    for span in ink.values_mut() {
        span.0 += x;
        span.1 += x;
    }
    occupied.push(ink);
    Ok(x)
}
