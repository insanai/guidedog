//! Text participates in horizontal clearance before line breaking chooses systems.

use gd_score::{Anchor, AnnotationKind, Error, Event, EventKind, Moment, Score, TextStyle};

use crate::Fonts;
use crate::limits::charge;
use crate::rods::{Bands, add_rect};

pub(crate) fn event(
    bands: &mut Bands,
    event: &Event,
    fonts: &Fonts<'_>,
    work: &mut usize,
    options: &mut crate::Options<'_>,
) -> Result<(), Error> {
    for annotation in &event.marks {
        charge(work, 1)?;
        if let AnnotationKind::Dynamic(text) = &annotation.kind {
            let y = if annotation.direction == gd_score::Direction::Up { -5.0 } else { 5.0 };
            let width = dynamic_width(fonts, text)?;
            add_rect(
                bands,
                crate::Rect { left: 0.0, right: width + 0.4, top: y - 2.0, bottom: y + 1.0 },
                work,
            )?;
            continue;
        }
        let (text, style, size) = match &annotation.kind {
            AnnotationKind::Text(text, style) => (text.as_str(), *style, 2.4),
            AnnotationKind::Tempo(text) | AnnotationKind::Rehearsal(text) => {
                (text.as_str(), TextStyle::Bold, 2.5)
            }
            _ => continue,
        };
        text_bands(bands, fonts, text, style, (size, false, false), work, &mut options.reserve)?;
    }
    if let EventKind::ChordName(text, _) = &event.kind {
        text_bands(
            bands,
            fonts,
            text,
            TextStyle::Regular,
            (2.5, false, false),
            work,
            &mut options.reserve,
        )?;
    }
    Ok(())
}

pub(crate) fn lyrics(
    bands: &mut Bands,
    score: &Score,
    at: Moment,
    staff: usize,
    fonts: &Fonts<'_>,
    work: &mut usize,
    options: &mut crate::Options<'_>,
) -> Result<(), Error> {
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
                    (2.4, true, syllable.hyphen),
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
    metrics: (f64, bool, bool),
    work: &mut usize,
    reserve: &mut Option<&mut crate::Reserve<'_>>,
) -> Result<(), Error> {
    let (size, centered, hyphen) = metrics;
    let run = crate::text::shape(fonts, text, size, style, &mut Vec::new(), work, reserve)?;
    let mut bounds = run.bounds;
    let x = if centered { -run.advance / 2.0 } else { 0.0 };
    bounds = bounds.translated(x, if centered { 7.0 } else { -5.0 });
    let padding = if centered { run.word_space.max(0.3) } else { 0.3 };
    bounds.left -= padding;
    bounds.right += padding + if hyphen { 0.6 } else { 0.0 };
    add_rect(bands, bounds, work)
}

pub(crate) fn dynamic_width(fonts: &Fonts<'_>, text: &str) -> Result<f64, Error> {
    let mut width = 0.0;
    for character in text.chars() {
        let name = crate::decorations::dynamic_glyph(character)
            .ok_or_else(|| crate::limits::font_error("Unknown dynamic mark."))?;
        let glyph = fonts.glyph(name)?;
        width += fonts.glyph_data(glyph).map(|data| data.advance).unwrap_or(0.8);
    }
    Ok(width)
}
