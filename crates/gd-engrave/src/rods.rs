use std::collections::BTreeMap;

use gd_score::Error;

use crate::columns::Column;
use crate::limits::{charge, failure, push};
use crate::prepare::{EventInfo, Plan};
use crate::{Fonts, Options, Rect, Rod};

pub(crate) type Bands = BTreeMap<i32, (f64, f64)>;

pub(crate) fn collect(
    out: &mut Vec<Rod>,
    score: &gd_score::Score,
    columns: &[Column],
    plan: &Plan,
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    out.clear();
    let staves = plan.events.iter().map(|e| e.staff + 1).max().unwrap_or(0);
    for staff in 0..staves {
        let mut previous = BTreeMap::<i32, (usize, f64)>::new();
        for (index, column) in columns.iter().enumerate() {
            charge(work, 1)?;
            let mut current = Bands::new();
            for id in &column.events {
                let info = &plan.events[id.0 as usize];
                if info.staff != staff {
                    continue;
                }
                event_bands(&mut current, info, fonts, work)?;
                crate::footprints::event(
                    &mut current, info, &score.events[id.0 as usize], score, fonts, work,
                )?;
                crate::labels::event(
                    &mut current,
                    &score.events[id.0 as usize],
                    fonts,
                    work,
                    options,
                )?;
            }
            crate::labels::lyrics(&mut current, score, column.at, staff, fonts, work, options)?;
            if index + 1 == columns.len() {
                add_rect(
                    &mut current,
                    Rect { left: 0.0, right: 0.1, top: -32.0, bottom: 64.0 },
                    work,
                )?;
            }
            if current.is_empty() {
                continue;
            }
            let mut minima = BTreeMap::<usize, f64>::new();
            for (band, (left, right)) in current {
                charge(work, 1)?;
                if let Some((first, edge)) = previous.insert(band, (index, right)) {
                    let minimum = edge - left + 0.2;
                    if minimum > 0.0 {
                        minima.entry(first).and_modify(|m| *m = m.max(minimum)).or_insert(minimum);
                    }
                }
            }
            for (first, minimum) in minima {
                push(out, Rod { first, last: index, minimum }, &mut options.reserve)?;
            }
        }
    }
    hairpins(out, score, columns, fonts, options, work)?;

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

pub(crate) fn add_rect(bands: &mut Bands, rect: Rect, work: &mut usize) -> Result<(), Error> {
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

pub(crate) fn add_glyph(
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

fn hairpins(
    out: &mut Vec<Rod>,
    score: &gd_score::Score,
    columns: &[Column],
    fonts: &Fonts<'_>,
    options: &mut Options<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    for spanner in &score.spanners {
        charge(work, 1)?;
        if !matches!(
            spanner.kind,
            gd_score::SpannerKind::Crescendo | gd_score::SpannerKind::Decrescendo
        ) {
            continue;
        }
        let a = &score.events[spanner.start.event.0 as usize];
        let b = &score.events[spanner.end.event.0 as usize];
        let first = columns.partition_point(|c| c.at < a.at);
        let last = columns.partition_point(|c| c.at < b.at);
        if first >= last {
            continue;
        }
        let mut width: f64 = 0.0;
        for annotation in &a.marks {
            if let gd_score::AnnotationKind::Dynamic(text) = &annotation.kind {
                width = width.max(crate::labels::dynamic_width(fonts, text)?);
            }
        }
        push(out, Rod { first, last, minimum: width + 2.5 }, &mut options.reserve)?;
    }
    Ok(())
}
