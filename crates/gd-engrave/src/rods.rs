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
    let staves =
        score.staves.len().max(plan.events.iter().map(|e| e.staff + 1).max().unwrap_or(0));
    for staff in 0..staves {
        let mut previous = BTreeMap::<i32, (usize, f64)>::new();
        for (index, column) in columns.iter().enumerate() {
            charge(work, 1)?;
            let mut current = Bands::new();
            let terminal = index + 1 == columns.len();
            add_column_bar_band(&mut current, score, columns, index, staff, terminal, work)?;
            let mut col_prefix = 0.0f64;
            if !terminal {
                for id in &column.events {
                    let info = &plan.events[id.0 as usize];
                    if info.staff != staff {
                        continue;
                    }
                    col_prefix = col_prefix.max(info.prefix);
                    event_bands(&mut current, info, fonts, work)?;
                    crate::footprints::event(
                        &mut current,
                        info,
                        &score.events[id.0 as usize],
                        score,
                        fonts,
                        work,
                    )?;
                    crate::labels::event(
                        &mut current,
                        info,
                        &score.events[id.0 as usize],
                        fonts,
                        work,
                        options,
                    )?;
                }
                voice_span_band(&mut current, column, plan, staff, fonts, work)?;
                crate::labels::lyrics(
                    &mut current,
                    score,
                    (column.at, col_prefix),
                    staff,
                    fonts,
                    work,
                    options,
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

fn add_column_bar_band(
    current: &mut Bands,
    score: &gd_score::Score,
    columns: &[Column],
    index: usize,
    staff: usize,
    terminal: bool,
    work: &mut usize,
) -> Result<(), Error> {
    if index > 0
        && let Some(style) = crate::bars::column_bar_style(score, columns, index, staff, work)?
    {
        let (bar_left, bar_right) = crate::bars::style_extent(style);
        let tab =
            score.staves.get(staff).is_some_and(|s| s.kind == gd_score::StaffKind::Tablature);
        let half = if tab { crate::tablature::HALF_HEIGHT + 0.2 } else { 2.2 };
        let (top, bottom) = if terminal { (-32.0, 64.0) } else { (-half, half) };
        // A clef change at this bar line is drawn before it, so it needs room there.
        let clef_before = !terminal
            && !tab
            && columns[index].events.iter().any(|id| {
                let event = &score.events[id.0 as usize];
                matches!(event.kind, gd_score::EventKind::Clef(_))
                    && score.voices[event.voice.0 as usize].staff.0 as usize == staff
            });
        add_rect(
            current,
            Rect { left: bar_left - 0.55, right: bar_right + 0.2, top, bottom },
            work,
        )?;
        if clef_before {
            let left = bar_left - crate::prepare::CLEF_BEFORE_BAR - 0.45;
            let rect = Rect { left, right: bar_left - 0.2, top: -3.6, bottom: 3.6 };
            add_rect(current, rect, work)?;
        }
    } else if terminal {
        add_rect(current, Rect { left: -0.5, right: 0.1, top: -32.0, bottom: 64.0 }, work)?;
    }
    Ok(())
}

/// Adds the bands of each unshared head of `info`: the head, its accidental and its dots.
fn head_bands(
    current: &mut Bands,
    info: &EventInfo,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    for head in &info.heads {
        if !head.shared {
            add_glyph(
                current,
                fonts,
                info.head_glyph,
                info.head_scale,
                info.prefix + head.at.x,
                head.at.y,
                work,
            )?;
        }
        if !head.shared
            && let Some(name) = head.accidental
        {
            add_glyph(
                current,
                fonts,
                name,
                info.head_scale,
                info.prefix + head.accidental_x,
                head.at.y,
                work,
            )?;
            cautionary_bands(current, info, head, name, fonts, work)?;
        }
        for dot in head.dots.iter().filter(|_| !head.shared) {
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
    Ok(())
}

fn event_bands(
    current: &mut Bands,
    info: &EventInfo,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    head_bands(current, info, fonts, work)?;
    for rect in crate::tablature::boxes(info) {
        add_rect(current, rect, work)?;
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
    if info.base_prefix > 0.0 {
        add_rect(
            current,
            Rect { left: 0.0, right: info.base_prefix, top: -4.0, bottom: 4.0 },
            work,
        )?;
    }
    Ok(())
}

/// Where voices share a staff column and some are shifted sideways, the column's heads
/// form one unit, as LilyPond's note columns do: a band across the whole staff keeps every
/// head of the next column right of every head of this one, whatever their heights.
fn voice_span_band(
    current: &mut Bands,
    column: &Column,
    plan: &Plan,
    staff: usize,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    let (mut left, mut right, mut shifted) = (f64::INFINITY, f64::NEG_INFINITY, false);
    for id in &column.events {
        charge(work, 1)?;
        let info = &plan.events[id.0 as usize];
        if info.staff != staff || info.heads.is_empty() {
            continue;
        }
        let glyph = fonts.glyph(info.head_glyph)?;
        let bounds = fonts
            .glyph_data(glyph)
            .ok_or_else(|| failure("Missing glyph collision geometry."))?
            .bounds;
        for head in info.heads.iter().filter(|head| !head.shared) {
            shifted |= head.at.x != 0.0 || info.voice_shift != 0.0;
            left = left.min(info.prefix + head.at.x + bounds.left * info.head_scale);
            right = right.max(info.prefix + head.at.x + bounds.right * info.head_scale);
        }
    }
    if shifted && left < right {
        add_rect(current, Rect { left, right, top: -6.0, bottom: 6.0 }, work)?;
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
        let mut end_pad: f64 = 0.0;
        for annotation in &b.marks {
            if let gd_score::AnnotationKind::Dynamic(_) = &annotation.kind {
                end_pad = 0.8;
            }
        }
        push(out, Rod { first, last, minimum: width + end_pad + 2.8 }, &mut options.reserve)?;
    }
    Ok(())
}

fn cautionary_bands(
    current: &mut Bands,
    info: &EventInfo,
    head: &crate::prepare::Head,
    name: &str,
    fonts: &Fonts<'_>,
    work: &mut usize,
) -> Result<(), Error> {
    if head.cautionary {
        let data = fonts
            .glyph_data(fonts.glyph(name)?)
            .ok_or_else(|| failure("Missing cautionary accidental bounds."))?;
        let pad = crate::accidentals::CAUTIONARY_PAD * info.head_scale;
        add_rect(
            current,
            Rect {
                left: info.prefix + head.accidental_x + data.bounds.left * info.head_scale - pad,
                right: info.prefix + head.accidental_x + data.bounds.right * info.head_scale + pad,
                top: head.at.y + data.bounds.top * info.head_scale,
                bottom: head.at.y + data.bounds.bottom * info.head_scale,
            },
            work,
        )?;
    }
    Ok(())
}
