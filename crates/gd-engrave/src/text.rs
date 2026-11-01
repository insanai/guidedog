use gd_score::{Error, TextStyle};

use crate::fonts::outline;
use crate::limits::{charge, font_error};
use crate::{Fonts, Rect};

/// One outlined text glyph in points, relative to its run's baseline.
#[derive(Clone, Debug)]
pub struct TextGlyph {
    /// SVG path with upward font y, already scaled to points.
    pub outline: String,
    /// Horizontal placement in points.
    pub x: f64,
    /// Downward vertical placement in points.
    pub y: f64,
}

/// Shaped text with real ink geometry; export never depends on browser fonts.
#[derive(Clone, Debug, Default)]
pub struct TextRun {
    /// Glyph outlines in visual order.
    pub glyphs: Vec<TextGlyph>,
    /// Shaped horizontal advance in points, including spaces.
    pub advance: f64,
    /// Composite ink bounds relative to the run's baseline.
    pub bounds: Rect,
    /// Face ascent in points.
    pub ascent: f64,
    /// Positive face descent in points.
    pub descent: f64,
    /// Opaque identities of the text font sources actually selected during shaping.
    pub sources: Vec<String>,
}

pub(crate) fn shape(
    fonts: &Fonts<'_>,
    text: &str,
    size: f64,
    style: TextStyle,
    warnings: &mut Vec<Error>,
    work: &mut usize,
) -> Result<TextRun, Error> {
    charge(work, text.len())?;
    if text.is_empty() {
        return Ok(TextRun::default());
    }
    if text.len() > 65_536 || !size.is_finite() || size <= 0.0 {
        return Err(font_error("Text exceeds the shaping bounds."));
    }
    let faces = match style {
        TextStyle::Bold if !fonts.bold.is_empty() => &fonts.bold,
        TextStyle::Italic if !fonts.italic.is_empty() => &fonts.italic,
        _ => &fonts.regular,
    };
    let Some(first) = faces.first() else {
        return Err(font_error("The score contains text but no text face was supplied."));
    };
    let first_face = first.face()?;
    let initial = shaped(&first_face, text);
    let infos = initial.glyph_infos();
    let positions = initial.glyph_positions();
    let mut clusters: Vec<usize> = infos.iter().map(|g| g.cluster as usize).collect();
    clusters.push(text.len());
    clusters.sort_unstable();
    clusters.dedup();
    let mut run = TextRun::default();
    note_source(&mut run, first);
    let mut at = 0;
    while at < infos.len() {
        charge(work, 1)?;
        let end = at + infos[at..].iter().take_while(|g| g.cluster == infos[at].cluster).count();
        if infos[at..end].iter().any(|g| g.glyph_id == 0) {
            let start = infos[at].cluster as usize;
            let last = clusters.partition_point(|c| *c <= start);
            let end_byte = clusters.get(last).copied().unwrap_or(text.len());
            let cluster =
                text.get(start..end_byte).ok_or_else(|| font_error("Invalid shaping cluster."))?;
            let mut selected = None;
            for candidate in faces {
                charge(work, 1)?;
                if candidate.covers(cluster) {
                    let face = candidate.face()?;
                    if cluster.chars().all(|c| c.is_control() || face.glyph_index(c).is_some()) {
                        selected = Some((candidate, face));
                        break;
                    }
                }
            }
            let (source, face) = selected.unwrap_or_else(|| {
                warnings.push(
                    font_error(format!("No supplied face can draw the text cluster {cluster:?}."))
                        .warning(),
                );
                (first, first_face.clone())
            });
            note_source(&mut run, source);
            let fallback = shaped(&face, cluster);
            append(
                &mut run,
                &face,
                fallback.glyph_infos(),
                fallback.glyph_positions(),
                size,
                work,
            )?;
        } else {
            append(&mut run, &first_face, &infos[at..end], &positions[at..end], size, work)?;
        }
        at = end;
    }
    Ok(run)
}

fn shaped(face: &rustybuzz::Face<'_>, text: &str) -> rustybuzz::GlyphBuffer {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    rustybuzz::shape(face, &[], buffer)
}

fn append(
    run: &mut TextRun,
    face: &rustybuzz::Face<'_>,
    infos: &[rustybuzz::GlyphInfo],
    positions: &[rustybuzz::GlyphPosition],
    size: f64,
    work: &mut usize,
) -> Result<(), Error> {
    let scale = size / f64::from(face.units_per_em());
    run.ascent = run.ascent.max(f64::from(face.ascender()) * scale);
    run.descent = run.descent.max(-f64::from(face.descender()) * scale);
    for (info, position) in infos.iter().zip(positions) {
        charge(work, 1)?;
        let glyph = ttf_parser::GlyphId(
            u16::try_from(info.glyph_id)
                .map_err(|_| font_error("A shaped glyph identifier exceeds OpenType's range."))?,
        );
        let (x, y) = (
            run.advance + f64::from(position.x_offset) * scale,
            -f64::from(position.y_offset) * scale,
        );
        if face.glyph_bounding_box(glyph).is_some() {
            let (path, bounds) = outline(face, glyph, scale)?;
            let bounds = bounds.translated(x, y);
            run.bounds = if run.glyphs.is_empty() { bounds } else { run.bounds.union(bounds) };
            run.glyphs
                .try_reserve(1)
                .map_err(|_| font_error("Cannot reserve text glyph outlines."))?;
            run.glyphs.push(TextGlyph { outline: path, x, y });
        }
        run.advance += f64::from(position.x_advance) * scale;
    }
    Ok(())
}

fn note_source(run: &mut TextRun, face: &crate::fonts::TextFace<'_>) {
    if let Some(id) = face.data.source.and_then(|source| source.id())
        && !run.sources.iter().any(|source| source == id)
    {
        run.sources.push(id.to_string());
    }
}
