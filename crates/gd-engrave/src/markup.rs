//! Shapes structured `gd_score::Markup` trees into self-contained `TextRun` outlines.

use std::fmt::Write;

use gd_score::{ColumnAlign, Enclosure, Error, Markup, TextStyle};

use crate::fonts::outline_reserved;
use crate::limits::{charge, failure, font_error, push};
use crate::svg::Number;
use crate::{Fonts, Rect, TextGlyph, TextRun};

pub(crate) struct Context<'a, 'f, 'w, 'r, 'cb> {
    pub(crate) fonts: &'a Fonts<'f>,
    pub(crate) space: f64,
    pub(crate) warnings: &'w mut Vec<Error>,
    pub(crate) work: &'w mut usize,
    pub(crate) reserve: &'r mut Option<&'cb mut crate::Reserve<'cb>>,
}

pub(crate) fn shape(
    ctx: &mut Context<'_, '_, '_, '_, '_>,
    markup: &Markup,
    size: f64,
    style: TextStyle,
) -> Result<TextRun, Error> {
    shape_node(ctx, markup, size, style, 0)
}

fn shape_node(
    ctx: &mut Context<'_, '_, '_, '_, '_>,
    markup: &Markup,
    size: f64,
    style: TextStyle,
    depth: usize,
) -> Result<TextRun, Error> {
    charge(ctx.work, 4)?;
    if depth > 64 || !size.is_finite() || size <= 0.0 {
        return Err(font_error("Markup exceeds depth or size bounds."));
    }
    match markup {
        Markup::Text(text) => {
            let mut run = crate::text::shape(
                ctx.fonts,
                text,
                size,
                style,
                ctx.warnings,
                ctx.work,
                ctx.reserve,
            )?;
            if run.word_space == 0.0 {
                run.word_space = size * 0.25;
            }
            if run.ascent == 0.0 {
                run.ascent = size * 0.7;
            }
            Ok(run)
        }
        Markup::Style(change, body) => shape_node(ctx, body, size, change.apply(style), depth + 1),
        Markup::Scaled { scale_percent, shift_percent, body } => {
            let scaled = size * (f64::from(*scale_percent) / 100.0).clamp(0.1, 10.0);
            let dy = -size * (f64::from(*shift_percent) / 100.0).clamp(-10.0, 10.0);
            let mut run = shape_node(ctx, body, scaled, style, depth + 1)?;
            translate_run(&mut run, 0.0, dy);
            Ok(run)
        }
        Markup::Space { horizontal, vertical } => {
            let dx = f64::from(*horizontal) / 100.0 * ctx.space;
            let dy = f64::from(*vertical) / 100.0 * ctx.space;
            Ok(TextRun {
                advance: dx.max(0.0),
                ascent: dy.max(0.0),
                descent: (-dy).max(0.0),
                word_space: size * 0.25,
                ..TextRun::default()
            })
        }
        Markup::MusicGlyph(name) => music_glyph(ctx, name, size),
        Markup::Dynamic(text) => dynamic_run(ctx, text, size),
        Markup::Line(items) => sequence(ctx, items, size, style, true, depth + 1),
        Markup::Concat(items) => sequence(ctx, items, size, style, false, depth + 1),
        Markup::Column(align, items) => column(ctx, *align, items, size, style, depth + 1),
        Markup::Framed(enclosure, body) => {
            let run = shape_node(ctx, body, size, style, depth + 1)?;
            frame(ctx, *enclosure, run, size)
        }
    }
}

fn sequence(
    ctx: &mut Context<'_, '_, '_, '_, '_>,
    items: &[Markup],
    size: f64,
    style: TextStyle,
    spaced: bool,
    depth: usize,
) -> Result<TextRun, Error> {
    let mut out = TextRun { word_space: size * 0.25, ..TextRun::default() };
    let mut prev_non_space = false;
    for item in items {
        if let Markup::Space { horizontal, .. } = item {
            out.advance = (out.advance + f64::from(*horizontal) / 100.0 * ctx.space).max(0.0);
            prev_non_space = false;
            continue;
        }
        let child = shape_node(ctx, item, size, style, depth)?;
        if child.glyphs.is_empty() && child.advance <= 0.0 {
            continue;
        }
        if spaced && prev_non_space {
            out.advance += out.word_space.max(child.word_space).max(size * 0.25);
        }
        let x = out.advance;
        merge_into(&mut out, child, x, 0.0, ctx.reserve)?;
        prev_non_space = true;
    }
    Ok(out)
}

fn merge_into(
    target: &mut TextRun,
    mut child: TextRun,
    dx: f64,
    dy: f64,
    reserve: &mut Option<&mut crate::Reserve<'_>>,
) -> Result<(), Error> {
    let had_glyphs = !target.glyphs.is_empty();
    translate_run(&mut child, dx, dy);
    if !child.glyphs.is_empty() {
        target.bounds = if had_glyphs { target.bounds.union(child.bounds) } else { child.bounds };
    }
    target.ascent = target.ascent.max(child.ascent - dy);
    target.descent = target.descent.max(child.descent + dy);
    target.word_space = target.word_space.max(child.word_space);
    target.advance = target.advance.max(dx + child.advance);
    for source in child.sources {
        if !target.sources.contains(&source) {
            target.sources.push(source);
        }
    }
    for glyph in child.glyphs {
        push(&mut target.glyphs, glyph, reserve)?;
    }
    Ok(())
}

fn column(
    ctx: &mut Context<'_, '_, '_, '_, '_>,
    align: ColumnAlign,
    items: &[Markup],
    size: f64,
    style: TextStyle,
    depth: usize,
) -> Result<TextRun, Error> {
    let mut rows: Vec<(TextRun, f64)> = Vec::new();
    let mut extra_gap = 0.0f64;
    for item in items {
        if let Markup::Space { vertical, .. } = item {
            extra_gap += (f64::from(*vertical) / 100.0 * ctx.space).max(0.0);
            continue;
        }
        let row = shape_node(ctx, item, size, style, depth)?;
        rows.push((row, std::mem::take(&mut extra_gap)));
    }
    if rows.is_empty() {
        return Ok(TextRun::default());
    }
    let max_w = rows
        .iter()
        .map(|(r, _)| r.advance.max(r.bounds.right - r.bounds.left.min(0.0)))
        .fold(0.0f64, f64::max);
    let mut offsets = vec![0.0f64; rows.len()];
    for idx in 1..rows.len() {
        let prev_desc = rows[idx - 1].0.descent.max(size * 0.18);
        let curr_asc = rows[idx].0.ascent.max(size * 0.72);
        offsets[idx] = offsets[idx - 1] + prev_desc + curr_asc + size * 0.2 + rows[idx].1;
    }
    let bottom_y = *offsets.last().unwrap_or(&0.0);
    let mut out = TextRun { advance: max_w, word_space: size * 0.25, ..TextRun::default() };
    for ((row, _), y) in rows.into_iter().zip(offsets) {
        let w = row.advance.max(row.bounds.right - row.bounds.left.min(0.0));
        let dx = match align {
            ColumnAlign::Left => 0.0,
            ColumnAlign::Center => (max_w - w) * 0.5,
            ColumnAlign::Right => max_w - w,
        };
        merge_into(&mut out, row, dx, y - bottom_y, ctx.reserve)?;
    }
    out.advance = max_w;
    Ok(out)
}

fn music_glyph(
    ctx: &mut Context<'_, '_, '_, '_, '_>,
    name: &str,
    size: f64,
) -> Result<TextRun, Error> {
    let glyph = ctx.fonts.glyph(name)?;
    let data = ctx.fonts.glyph_data(glyph).ok_or_else(|| font_error("Missing music glyph."))?;
    let music_space = size * 0.36;
    let scale = (music_space * 4.0) / f64::from(ctx.fonts.music.units_per_em());
    let (outline, bounds) = outline_reserved(
        &ctx.fonts.music,
        ttf_parser::GlyphId(glyph.0),
        scale,
        ctx.reserve.as_deref_mut(),
    )?;
    charge(ctx.work, outline.len().saturating_add(64))?;
    // SMuFL centers `metAugmentationDot` on the metronome notehead's origin with zero
    // sidebearing, so it shares the notehead's baseline lift and takes a small lead-in.
    let met_dot = name == "metAugmentationDot";
    let dy = if met_dot || bounds.bottom > 0.25 * music_space { -size * 0.32 } else { 0.0 };
    let lead = if met_dot { 0.2 * music_space } else { 0.0 };
    let dx = (-bounds.left).max(0.0) + lead;
    let placed = bounds.translated(dx, dy);
    let advance = (dx + bounds.right).max(lead + data.advance * music_space);
    let mut glyphs = Vec::new();
    push(&mut glyphs, TextGlyph { outline, x: dx, y: dy }, ctx.reserve)?;
    Ok(TextRun {
        glyphs,
        advance,
        bounds: placed,
        ascent: (-placed.top).max(size * 0.7),
        descent: placed.bottom.max(0.0),
        word_space: size * 0.25,
        sources: Vec::new(),
    })
}

fn dynamic_run(
    ctx: &mut Context<'_, '_, '_, '_, '_>,
    text: &str,
    size: f64,
) -> Result<TextRun, Error> {
    let mut out = TextRun { word_space: size * 0.25, ..TextRun::default() };
    let music_space = size * 0.38;
    let scale = (music_space * 4.0) / f64::from(ctx.fonts.music.units_per_em());
    for ch in text.chars() {
        let name = crate::decorations::dynamic_glyph(ch)
            .ok_or_else(|| font_error("Markup dynamic contains an unsupported character."))?;
        let glyph = ctx.fonts.glyph(name)?;
        let data = ctx.fonts.glyph_data(glyph).ok_or_else(|| font_error("Missing dynamic."))?;
        let (outline, bounds) = outline_reserved(
            &ctx.fonts.music,
            ttf_parser::GlyphId(glyph.0),
            scale,
            ctx.reserve.as_deref_mut(),
        )?;
        charge(ctx.work, outline.len().saturating_add(64))?;
        let x = out.advance - bounds.left.min(0.0);
        let placed = bounds.translated(x, 0.0);
        out.bounds = if out.glyphs.is_empty() { placed } else { out.bounds.union(placed) };
        push(&mut out.glyphs, TextGlyph { outline, x, y: 0.0 }, ctx.reserve)?;
        out.advance = x + (data.advance + 0.05) * music_space;
    }
    out.advance = out.advance.max(out.bounds.right);
    out.ascent = (-out.bounds.top).max(size * 0.7);
    out.descent = out.bounds.bottom.max(0.0);
    Ok(out)
}

fn frame(
    ctx: &mut Context<'_, '_, '_, '_, '_>,
    enclosure: Enclosure,
    mut run: TextRun,
    size: f64,
) -> Result<TextRun, Error> {
    if run.glyphs.is_empty() && run.advance <= 0.0 {
        return Ok(run);
    }
    // The floor is a tenth of a staff space, so spacing (in staff spaces) and drawing
    // (in points) agree on a frame's width.
    let thickness = (size * 0.075).max(ctx.space * 0.1);
    let pad = size * 0.22;
    let mut content = if run.glyphs.is_empty() {
        Rect { left: 0.0, right: run.advance, top: -size * 0.7, bottom: 0.0 }
    } else {
        run.bounds
    };
    let (outline, frame_bounds) = match enclosure {
        Enclosure::Underline => {
            let left = content.left.min(0.0);
            let right = content.right.max(run.advance);
            let top = content.bottom.max(0.0) + size * 0.1;
            let bottom = top + thickness;
            (solid_rect(left, top, right, bottom)?, Rect { left, right, top, bottom })
        }
        Enclosure::Box => {
            let shift = (pad + thickness - content.left).max(0.0);
            if shift > 0.0 {
                translate_run(&mut run, shift, 0.0);
                content = content.translated(shift, 0.0);
            }
            let outer = Rect {
                left: content.left - pad - thickness,
                right: content.right + pad + thickness,
                top: content.top - pad - thickness,
                bottom: content.bottom + pad + thickness,
            };
            let inner = Rect {
                left: content.left - pad,
                right: content.right + pad,
                top: content.top - pad,
                bottom: content.bottom + pad,
            };
            run.advance = outer.right;
            (hollow_box(outer, inner)?, outer)
        }
        Enclosure::Circle => circle_frame(&mut run, &mut content, pad, thickness, size)?,
    };
    run.bounds = content.union(frame_bounds);
    run.ascent = (-run.bounds.top).max(run.ascent);
    run.descent = run.bounds.bottom.max(run.descent);
    push(&mut run.glyphs, TextGlyph { outline, x: 0.0, y: 0.0 }, ctx.reserve)?;
    Ok(run)
}

fn circle_frame(
    run: &mut TextRun,
    content: &mut Rect,
    pad: f64,
    thickness: f64,
    size: f64,
) -> Result<(String, Rect), Error> {
    let mut rx = ((content.right - content.left) * 0.5 + pad).max(size * 0.42);
    let mut ry = ((content.bottom - content.top) * 0.5 + pad).max(size * 0.42);
    if (rx - ry).abs() <= rx.max(ry) * 0.3 {
        let r = rx.max(ry);
        rx = r;
        ry = r;
    }
    let (orx, ory) = (rx + thickness, ry + thickness);
    let shift = (orx - (content.left + content.right) * 0.5).max(0.0);
    if shift > 0.0 {
        translate_run(run, shift, 0.0);
        *content = content.translated(shift, 0.0);
    }
    let (cx, cy) = ((content.left + content.right) * 0.5, (content.top + content.bottom) * 0.5);
    let outer = Rect { left: cx - orx, right: cx + orx, top: cy - ory, bottom: cy + ory };
    run.advance = outer.right;
    Ok((hollow_oval(cx, cy, (orx, ory), (rx, ry))?, outer))
}

fn translate_run(run: &mut TextRun, dx: f64, dy: f64) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    for glyph in &mut run.glyphs {
        glyph.x += dx;
        glyph.y += dy;
    }
    if !run.glyphs.is_empty() {
        run.bounds = run.bounds.translated(dx, dy);
        run.ascent = (-run.bounds.top).max(0.0);
        run.descent = run.bounds.bottom.max(0.0);
    }
}

fn solid_rect(left: f64, top: f64, right: f64, bottom: f64) -> Result<String, Error> {
    let mut path = String::new();
    write!(
        path,
        "M{} {}L{} {}L{} {}L{} {}Z",
        Number(left),
        Number(-top),
        Number(right),
        Number(-top),
        Number(right),
        Number(-bottom),
        Number(left),
        Number(-bottom),
    )
    .map_err(|_| failure("Invalid markup underline coordinate."))?;
    Ok(path)
}

fn hollow_box(outer: Rect, inner: Rect) -> Result<String, Error> {
    let mut path = String::new();
    write!(
        path,
        "M{} {}L{} {}L{} {}L{} {}ZM{} {}L{} {}L{} {}L{} {}Z",
        Number(outer.left),
        Number(-outer.top),
        Number(outer.right),
        Number(-outer.top),
        Number(outer.right),
        Number(-outer.bottom),
        Number(outer.left),
        Number(-outer.bottom),
        Number(inner.left),
        Number(-inner.top),
        Number(inner.left),
        Number(-inner.bottom),
        Number(inner.right),
        Number(-inner.bottom),
        Number(inner.right),
        Number(-inner.top),
    )
    .map_err(|_| failure("Invalid markup box coordinate."))?;
    Ok(path)
}

fn hollow_oval(cx: f64, cy: f64, outer: (f64, f64), inner: (f64, f64)) -> Result<String, Error> {
    let mut path = String::new();
    oval_loop(&mut path, cx, -cy, outer.0, outer.1, true)?;
    oval_loop(&mut path, cx, -cy, inner.0, inner.1, false)?;
    Ok(path)
}

fn oval_loop(
    out: &mut String,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    clockwise: bool,
) -> Result<(), Error> {
    const K: f64 = 0.552_284_75;
    let (kx, ky) = (rx * K, ry * K);
    let s = if clockwise { 1.0 } else { -1.0 };
    write!(
        out,
        "M{} {}C{} {} {} {} {} {}C{} {} {} {} {} {}C{} {} {} {} {} {}C{} {} {} {} {} {}Z",
        Number(cx),
        Number(cy + ry),
        Number(cx + s * kx),
        Number(cy + ry),
        Number(cx + s * rx),
        Number(cy + ky),
        Number(cx + s * rx),
        Number(cy),
        Number(cx + s * rx),
        Number(cy - ky),
        Number(cx + s * kx),
        Number(cy - ry),
        Number(cx),
        Number(cy - ry),
        Number(cx - s * kx),
        Number(cy - ry),
        Number(cx - s * rx),
        Number(cy - ky),
        Number(cx - s * rx),
        Number(cy),
        Number(cx - s * rx),
        Number(cy + ky),
        Number(cx - s * kx),
        Number(cy + ry),
        Number(cx),
        Number(cy + ry),
    )
    .map_err(|_| failure("Invalid markup circle coordinate."))
}
