use std::collections::BTreeMap;

use gd_score::Error;

use crate::columns::Column;
use crate::limits::{charge, failure, push};
use crate::prepare::{EventInfo, Plan};
use crate::{Fonts, Options, Rect, Rod};

type Bands = BTreeMap<i32, (f64, f64)>;

pub(crate) fn collect(
    out: &mut Vec<Rod>,
    columns: &[Column],
    plan: &Plan,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    out.clear();
    let staves = plan.events.iter().map(|e| e.staff + 1).max().unwrap_or(0);
    for staff in 0..staves {
        let mut previous: Option<(usize, Bands)> = None;
        for (index, column) in columns.iter().enumerate() {
            charge(work, 1)?;
            let mut current = Bands::new();
            for id in &column.events {
                let info = &plan.events[id.0 as usize];
                if info.staff != staff {
                    continue;
                }
                event_bands(&mut current, info, fonts, work)?;
            }
            if current.is_empty() {
                continue;
            }
            if let Some((first, previous)) = &previous {
                let mut minimum: f64 = 0.0;
                for (band, (left, _)) in &current {
                    charge(work, 1)?;
                    if let Some((_, right)) = previous.get(band) {
                        minimum = minimum.max(right - left + 0.2);
                    }
                }
                if minimum > 0.0 {
                    push(out, Rod { first: *first, last: index, minimum }, &mut options.reserve)?;
                }
            }
            previous = Some((index, current));
        }
    }
    Ok(())
}

fn event_bands(
    current: &mut Bands,
    info: &EventInfo,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    for head in &info.heads {
        add_glyph(
            current,
            fonts,
            info.head_glyph,
            info.head_scale,
            info.prefix + head.at.x,
            head.at.y,
            work,
        )?;
        if let Some(name) = head.accidental {
            add_glyph(
                current,
                fonts,
                name,
                info.head_scale,
                info.prefix + head.accidental_x,
                head.at.y,
                work,
            )?;
        }
        for dot in &head.dots {
            add_glyph(
                current,
                fonts,
                "augmentationDot",
                info.head_scale,
                info.prefix + info.voice_shift + dot.x,
                dot.y,
                work,
            )?;
        }
    }
    if !info.heads.is_empty() {
        add_rect(
            current,
            Rect {
                left: info.prefix + info.stem_x - 0.08,
                right: info.prefix + info.stem_x + 0.08,
                top: info.stem_base.min(info.stem_tip),
                bottom: info.stem_base.max(info.stem_tip),
            },
            work,
        )?;
        if info.flags > 0 && info.beam.is_none() {
            add_glyph(
                current,
                fonts,
                crate::heads::flag(info.flags, info.up),
                info.head_scale,
                info.prefix + info.stem_x,
                info.stem_tip,
                work,
            )?;
        }
    }
    if info.prefix > 0.0 {
        add_rect(current, Rect { left: 0.0, right: info.prefix, top: -4.0, bottom: 4.0 }, work)?;
    }
    Ok(())
}

fn add_rect(bands: &mut Bands, rect: Rect, work: &mut usize) -> Result<(), Error> {
    for band in (rect.top * 4.0).floor() as i32..=(rect.bottom * 4.0).ceil() as i32 {
        charge(work, 1)?;
        bands
            .entry(band)
            .and_modify(|value| {
                value.0 = value.0.min(rect.left);
                value.1 = value.1.max(rect.right);
            })
            .or_insert((rect.left, rect.right));
    }
    Ok(())
}

fn add_glyph(
    bands: &mut Bands,
    fonts: &Fonts<'_>,
    name: &str,
    scale: f64,
    x: f64,
    y: f64,
    work: &mut usize,
) -> Result<(), Error> {
    let glyph = fonts.glyph(name)?;
    let data =
        fonts.glyph_data(glyph).ok_or_else(|| failure("Missing glyph collision geometry."))?;
    let bounds = Rect {
        left: data.bounds.left * scale + x,
        right: data.bounds.right * scale + x,
        top: data.bounds.top * scale + y,
        bottom: data.bounds.bottom * scale + y,
    };
    for band in (bounds.top * 4.0).floor() as i32..=(bounds.bottom * 4.0).ceil() as i32 {
        charge(work, 1)?;
        let (top, bottom) = (f64::from(band) / 4.0, f64::from(band + 1) / 4.0);
        let (mut left, mut right) = (bounds.left, bounds.right);
        for (name, point) in &data.anchors {
            let cut_y = y - point.y * scale;
            let cut_x = x + point.x * scale;
            match name.as_str() {
                "cutOutNE" if bottom <= cut_y => right = right.min(cut_x),
                "cutOutSE" if top >= cut_y => right = right.min(cut_x),
                "cutOutNW" if bottom <= cut_y => left = left.max(cut_x),
                "cutOutSW" if top >= cut_y => left = left.max(cut_x),
                _ => {}
            }
        }
        if right >= left {
            bands
                .entry(band)
                .and_modify(|value| {
                    value.0 = value.0.min(left);
                    value.1 = value.1.max(right);
                })
                .or_insert((left, right));
        }
    }
    Ok(())
}
