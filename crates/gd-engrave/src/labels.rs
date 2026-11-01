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
        let (text, style, size) = match &annotation.kind {
            AnnotationKind::Text(text, style) => (text.as_str(), *style, 2.4),
            AnnotationKind::Tempo(text) | AnnotationKind::Rehearsal(text) => {
                (text.as_str(), TextStyle::Bold, 2.5)
            }
            _ => continue,
        };
        text_bands(bands, fonts, text, style, (size, false), work, &mut options.reserve)?;
    }
    if let EventKind::ChordName(text, _) = &event.kind {
        text_bands(
            bands,
            fonts,
            text,
            TextStyle::Regular,
            (2.5, false),
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
                    (2.4, true),
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
    metrics: (f64, bool),
    work: &mut usize,
    reserve: &mut Option<&mut crate::Reserve<'_>>,
) -> Result<(), Error> {
    let (size, centered) = metrics;
    let run = crate::text::shape(fonts, text, size, style, &mut Vec::new(), work, reserve)?;
    let mut bounds = run.bounds;
    let x = if centered { -run.advance / 2.0 } else { 0.0 };
    bounds = bounds.translated(x, if centered { 7.0 } else { -5.0 });
    bounds.left -= 0.3;
    bounds.right += 0.3;
    add_rect(bands, bounds, work)
}
