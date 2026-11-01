//! Greedy outside-in accidental columns use actual glyph ink and cautionary padding.

use gd_score::Error;

use crate::Fonts;
use crate::prepare::EventInfo;
use crate::rods::{Bands, add_glyph};

pub(crate) fn arrange(
    info: &mut EventInfo,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let mut order: Vec<_> =
        info.heads.iter().enumerate().filter_map(|(i, head)| head.accidental.map(|_| i)).collect();
    order.sort_by(|&a, &b| info.heads[a].at.y.total_cmp(&info.heads[b].at.y));
    let mut outside = Vec::new();
    while !order.is_empty() {
        outside.push(order.remove(0));
        if let Some(last) = order.pop() {
            outside.push(last);
        }
    }
    let mut occupied: Vec<Bands> = Vec::new();
    for index in outside {
        let head = &mut info.heads[index];
        let Some(name) = head.accidental else { continue };
        let glyph = fonts.glyph(name)?;
        let Some(data) = fonts.glyph_data(glyph) else { continue };
        let padding = if head.cautionary { 0.6 } else { 0.0 };
        let size = info.head_scale;
        let ledger_left = if head.at.y.abs() >= 3.0 { head.at.x - 0.4 * size } else { 0.0 };
        let mut x = info.bounds.left.min(ledger_left) - 0.3 - data.bounds.right * size - padding;
        let mut ink = Bands::new();
        add_glyph(&mut ink, fonts, name, size, 0.0, head.at.y, work)?;
        // Cautionary parentheses have no useful nesting cut-outs.
        if head.cautionary {
            for span in ink.values_mut() {
                span.0 = data.bounds.left * size - padding;
                span.1 = data.bounds.right * size + padding;
            }
        }
        for previous in &occupied {
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
        head.accidental_x = x;
    }
    Ok(())
}
