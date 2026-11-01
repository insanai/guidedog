//! Greedy outside-in accidental columns use actual glyph ink and cautionary padding.

use gd_score::Error;

use crate::prepare::EventInfo;
use crate::{Fonts, Rect};

pub(crate) fn arrange(info: &mut EventInfo, fonts: &Fonts<'_>) -> Result<(), Error> {
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
    let mut occupied: Vec<Rect> = Vec::new();
    for index in outside {
        let head = &mut info.heads[index];
        let Some(name) = head.accidental else { continue };
        let glyph = fonts.glyph(name)?;
        let Some(data) = fonts.glyph_data(glyph) else { continue };
        let padding = if head.cautionary { 0.6 } else { 0.0 };
        let size = info.head_scale;
        let mut x = info.bounds.left.min(0.0) - 0.3 - data.bounds.right * size - padding;
        let bounds = |x| Rect {
            left: x + data.bounds.left * size - padding,
            right: x + data.bounds.right * size + padding,
            top: head.at.y + data.bounds.top * size - 0.2,
            bottom: head.at.y + data.bounds.bottom * size + 0.2,
        };
        for previous in &occupied {
            let ink = bounds(x);
            if ink.overlaps(*previous, 0.0) {
                x -= ink.right - previous.left + 0.2;
            }
        }
        occupied.push(bounds(x));
        head.accidental_x = x;
    }
    Ok(())
}
