//! Text participates in horizontal clearance before line breaking chooses systems.

use gd_score::{
    Anchor, AnnotationKind, Error, Event, EventKind, MarkupRole, Moment, Score, TextStyle,
};

use crate::Fonts;
use crate::limits::charge;
use crate::rods::{Bands, add_rect};

/// Adds the bands of a markup mark in its role's style and size.
fn markup_bands(
    bands: &mut Bands,
    (markup, role): (&gd_score::Markup, MarkupRole),
    prefix: f64,
    fonts: &Fonts<'_>,
    work: &mut usize,
    options: &mut crate::Options<'_>,
) -> Result<(), Error> {
    let (style, size) = match role {
        MarkupRole::Script => (TextStyle::Regular, 2.4),
        MarkupRole::Tempo | MarkupRole::Rehearsal => (TextStyle::Bold, 2.5),
    };
    let mut warnings = Vec::new();
    let mut ctx = crate::markup::Context {
        fonts,
        space: 1.0,
        warnings: &mut warnings,
        work,
        reserve: &mut options.reserve,
    };
    let run = crate::markup::shape(&mut ctx, markup, size, style)?;
    run_bands(bands, &run, (prefix, false, false), work)
}

pub(crate) fn event(
    bands: &mut Bands,
    prefix: f64,
    event: &Event,
    fonts: &Fonts<'_>,
    work: &mut usize,
    options: &mut crate::Options<'_>,
) -> Result<(), Error> {
    for annotation in &event.marks {
        charge(work, 1)?;
        match &annotation.kind {
            AnnotationKind::Dynamic(text) => {
                let y = if annotation.direction == gd_score::Direction::Up { -5.0 } else { 5.0 };
                let width = dynamic_width(fonts, text)?;
                add_rect(
                    bands,
                    crate::Rect {
                        left: prefix,
                        right: prefix + width + 0.4,
                        top: y - 2.0,
                        bottom: y + 1.0,
                    },
                    work,
                )?;
            }
            AnnotationKind::Text(text, style) => {
                text_bands(
                    bands,
                    fonts,
                    text,
                    *style,
                    (prefix, 2.4, false, false),
                    work,
                    &mut options.reserve,
                )?;
            }
            AnnotationKind::Tempo(text) | AnnotationKind::Rehearsal(text) => {
                text_bands(
                    bands,
                    fonts,
                    text,
                    TextStyle::Bold,
                    (prefix, 2.5, false, false),
                    work,
                    &mut options.reserve,
                )?;
            }
            AnnotationKind::Markup(markup, role) => {
                markup_bands(bands, (markup, *role), prefix, fonts, work, options)?;
            }
            AnnotationKind::Breath => {
                // A breathing sign takes its own room after the note, as LilyPond gives it
                // a column of its own, so the next note always moves aside.
                let x = prefix + crate::decorations::BREATH_X;
                let rect = crate::Rect { left: x - 0.2, right: x + 1.0, top: -4.0, bottom: 4.0 };
                add_rect(bands, rect, work)?;
            }
            AnnotationKind::Articulation(_) | AnnotationKind::Fingering(_) => {}
        }
    }
    if let EventKind::ChordName(text, _) = &event.kind {
        text_bands(
            bands,
            fonts,
            text,
            TextStyle::Regular,
            (prefix, 2.5, false, false),
            work,
            &mut options.reserve,
        )?;
    }
    Ok(())
}

pub(crate) fn lyrics(
    bands: &mut Bands,
    score: &Score,
    column: (Moment, f64),
    staff: usize,
    fonts: &Fonts<'_>,
    work: &mut usize,
    options: &mut crate::Options<'_>,
) -> Result<(), Error> {
    let (at, prefix) = column;
    for verse in &score.lyrics {
        if score.voices[verse.voice.0 as usize].staff.0 as usize != staff {
            continue;
        }
        for syllable in &verse.syllables {
            charge(work, 1)?;
            let moment = match syllable.anchor {
                Anchor::Moment(moment) => moment,
                Anchor::Note(reference) => score.events[reference.event.0 as usize].at,
            };
            if moment == at {
                text_bands(
                    bands,
                    fonts,
                    &syllable.text,
                    TextStyle::Regular,
                    (prefix, 2.4, true, syllable.hyphen),
                    work,
                    &mut options.reserve,
                )?;
            }
        }
    }
    Ok(())
}

fn text_bands(
    bands: &mut Bands,
    fonts: &Fonts<'_>,
    text: &str,
    style: TextStyle,
    metrics: (f64, f64, bool, bool),
    work: &mut usize,
    reserve: &mut Option<&mut crate::Reserve<'_>>,
) -> Result<(), Error> {
    let (prefix, size, centered, hyphen) = metrics;
    let run = crate::text::shape(fonts, text, size, style, &mut Vec::new(), work, reserve)?;
    run_bands(bands, &run, (prefix, centered, hyphen), work)
}

fn run_bands(
    bands: &mut Bands,
    run: &crate::TextRun,
    metrics: (f64, bool, bool),
    work: &mut usize,
) -> Result<(), Error> {
    let (prefix, centered, hyphen) = metrics;
    let x = prefix + if centered { -run.advance / 2.0 } else { 0.0 };
    let mut bounds = run.bounds.translated(x, if centered { 7.0 } else { -5.0 });
    let padding = if centered { run.word_space.max(0.3) } else { 0.3 };
    bounds.left -= padding;
    bounds.right += padding + if hyphen { 0.6 } else { 0.0 };
    add_rect(bands, bounds, work)
}

pub(crate) fn dynamic_width(fonts: &Fonts<'_>, text: &str) -> Result<f64, Error> {
    let mut cursor = 0.0f64;
    let mut right = 0.0f64;
    for character in text.chars() {
        let name = crate::decorations::dynamic_glyph(character)
            .ok_or_else(|| crate::limits::font_error("Unknown dynamic mark."))?;
        let glyph = fonts.glyph(name)?;
        if let Some(data) = fonts.glyph_data(glyph) {
            right = right.max(cursor + data.bounds.right);
            cursor += data.advance + 0.05;
        } else {
            cursor += 0.85;
            right = right.max(cursor);
        }
    }
    Ok(right.max(cursor - 0.05).max(0.0))
}

/// The name a staff or group shows on a line: the full name on the first system, the
/// abbreviation afterwards.
pub(crate) fn line_label<'a>(
    first_line: bool,
    full: (&'a str, Option<&'a gd_score::Markup>),
    short: (&'a str, Option<&'a gd_score::Markup>),
) -> (&'a str, Option<&'a gd_score::Markup>) {
    if first_line { full } else { short }
}

/// The advance of an instrument name in points; zero when there is none.
pub(crate) fn label_advance<'cb>(
    fonts: &Fonts<'_>,
    label: (&str, Option<&gd_score::Markup>),
    space: f64,
    work: &mut usize,
    reserve: &mut Option<&'cb mut crate::Reserve<'cb>>,
) -> Result<f64, Error> {
    let mut warnings = Vec::new();
    match label {
        (_, Some(markup)) => {
            let mut ctx =
                crate::markup::Context { fonts, space, warnings: &mut warnings, work, reserve };
            Ok(crate::markup::shape(&mut ctx, markup, space * 2.3, TextStyle::Regular)?.advance)
        }
        ("", None) => Ok(0.0),
        (name, None) => Ok(crate::text::shape(
            fonts,
            name,
            space * 2.3,
            TextStyle::Regular,
            &mut warnings,
            work,
            reserve,
        )?
        .advance),
    }
}
