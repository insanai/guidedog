//! Native music policy and assets. The independent libraries own reading and engraving.

mod assets;
mod diagnostics;
mod fonts;
mod options;
mod source;

pub use fonts::{MusicFonts, MusicSource, load_music_source};

use gd_engrave::{Engraving, Fonts};
use gd_env::OutputFormat;
use guidedoc::{Document, Fail, NodeId};

use crate::figure::{Drawing, replace};
use crate::page::Page;
use crate::rewrite::Rewriter;

pub(crate) fn replace_music(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let source;
    let fallback;
    let fonts = if let Some(shared) = page.music {
        &shared.loaded
    } else {
        source = load_music_source(page.config());
        fallback = source.load_fonts();
        &fallback.loaded
    };
    let fonts = match fonts {
        Ok(fonts) => fonts,
        Err(error) => {
            diagnostics::warn(page, rw.doc, id, error.to_string());
            return rw.copy_node(page, id);
        }
    };
    match draw(page, rw.doc, id, fonts) {
        Ok(drawing) => replace(page, rw, id, drawing),
        Err(message) => {
            if !message.is_empty() {
                diagnostics::warn(page, rw.doc, id, message);
            }
            rw.copy_node(page, id)
        }
    }
}

fn read(
    page: &mut Page<'_>,
    doc: Document<'_>,
    id: NodeId,
    ledger: &gd_host::Ledger<'_>,
) -> Result<gd_lilypond::Loaded, String> {
    let mut origin = source::origin(page, doc, id);
    let mut text = doc.node_text(id).to_owned();
    if let Some(name) = doc.custom_attribute(id, "lilypond-file") {
        let file = source::read(page, &origin, name).map_err(|e| e.message)?;
        origin = file.name.into();
        text = file.text;
    }
    let mut files = vec![origin.clone()];
    let mut include =
        |request: gd_lilypond::IncludeRequest<'_>| source::include(page, &mut files, request);
    let reserve = |bytes| ledger.charge(bytes);
    let result = gd_lilypond::load_score(
        &text,
        &origin.to_string_lossy(),
        &mut gd_lilypond::Options {
            include: Some(&mut include),
            reserve: Some(&reserve),
            ..Default::default()
        },
    );
    match result {
        Ok(loaded) => {
            for error in &loaded.warnings {
                diagnostics::report(page, doc, id, error, &loaded.files);
            }
            Ok(loaded)
        }
        Err(failed) => {
            diagnostics::report(page, doc, id, &failed.error, &failed.files);
            Err(String::new())
        }
    }
}

fn draw<'d>(
    page: &mut Page<'_>,
    doc: Document<'d>,
    id: NodeId,
    fonts: &Fonts<'_>,
) -> Result<Drawing<'d>, String> {
    let ledger = page.ledger.budget().map(gd_host::Ledger::new).unwrap_or_default();
    let result = draw_held(page, doc, id, fonts, &ledger);
    if ledger.refused() > 0 {
        page.ledger.charge(ledger.refused());
    }
    result
}

fn draw_held<'d>(
    page: &mut Page<'_>,
    doc: Document<'d>,
    id: NodeId,
    fonts: &Fonts<'_>,
    ledger: &gd_host::Ledger<'_>,
) -> Result<Drawing<'d>, String> {
    let loaded = read(page, doc, id, ledger)?;
    let mut reserve = |bytes| ledger.charge(bytes);
    let mut options = options::dimensions(page, doc, id)?;
    options.reserve = Some(&mut reserve);
    let engraving = page
        .engraver
        .engrave_score(&loaded.score, fonts, &mut options)
        .map_err(|e| e.to_string())?;
    for error in &engraving.warnings {
        diagnostics::report(page, doc, id, error, &loaded.files);
    }
    if page.format == OutputFormat::Pdf && engraving.width > options.line_width + 0.01 {
        diagnostics::warn(
            page,
            doc,
            id,
            "Music exceeds the declared PDF text width and will be scaled to fit.".into(),
        );
    }
    let alt = diagnostics::description(page, &engraving.summary);
    let path = assets::save(page, doc, id, &engraving, None, &alt, ledger)?;
    let parts = pdf_parts(page, doc, id, &engraving, &alt, ledger)?;
    let mut narrow = None;
    if page.format == OutputFormat::Html && options.line_width > 225.0 {
        options.line_width = 225.0;
        options.staff_size = options.staff_size.min(14.0);
        if let Ok(drawing) = page.engraver.engrave_score(&loaded.score, fonts, &mut options) {
            narrow = Some(assets::save(page, doc, id, &drawing, None, &alt, ledger)?);
        }
    }
    // The image and its SVG carry the same localized description.
    Ok(Drawing {
        path,
        source: doc.node_text(id),
        class: "lilypond-music",
        interactive: false,
        adaptive: page.format == OutputFormat::Html
            && doc.custom_attribute(id, "format") != Some("png"),
        alt: Some(alt),
        narrow,
        parts,
    })
}

fn pdf_parts(
    page: &mut Page<'_>,
    doc: Document<'_>,
    id: NodeId,
    drawing: &Engraving,
    alt: &str,
    ledger: &gd_host::Ledger<'_>,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut parts = Vec::new();
    if page.format == OutputFormat::Pdf && drawing.systems.len() > 1 {
        for system in 0..drawing.systems.len() {
            parts.push(assets::save(page, doc, id, drawing, Some(system), alt, ledger)?);
        }
    }
    Ok(parts)
}
