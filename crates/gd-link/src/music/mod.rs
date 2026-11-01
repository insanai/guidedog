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
    if page.format == OutputFormat::Text {
        return show_unengraved(page, rw, id, None);
    }
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
            return show_unengraved(page, rw, id, None);
        }
    };
    let mut external_text = None;
    match draw(page, rw.doc, id, fonts, &mut external_text) {
        Ok(drawing) => replace(page, rw, id, drawing),
        Err(message) => {
            if !message.is_empty() {
                diagnostics::warn(page, rw.doc, id, message);
            }
            show_unengraved(page, rw, id, external_text.as_deref())
        }
    }
}

/// Shows a score that is not engraved. Inline source stays as code. A file argument shows
/// the file's source when it was or can be read, and otherwise says which file was not
/// engraved, rather than presenting its path as LilyPond code.
fn show_unengraved(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
    text: Option<&str>,
) -> Result<(), Fail> {
    if let Some(text) = text {
        return copy_code_with_text(page, rw, id, text);
    }
    let Some(name) = rw.doc.custom_attribute(id, guidedoc::SCORE_FILE_ATTRIBUTE) else {
        return rw.copy_node(page, id);
    };
    if let Ok(file) = source::read(page, &source::origin(page, rw.doc, id), name) {
        return copy_code_with_text(page, rw, id, &file.text);
    }
    let span = rw.doc.node(id).span;
    let paragraph = rw.open(guidedoc::NodeKind::Paragraph, span)?;
    rw.moved_to(id, paragraph);
    // The engine's label is the catalog message, so build and conversion say the same.
    let msgid = guidedoc::ENGLISH_LABELS[guidedoc::Label::UnengravedScore as usize];
    let message = page.view.ui.gettext(msgid).replace("%s", name);
    rw.add_plain(&message, span)?;
    rw.close(paragraph)
}

fn copy_code_with_text(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
    text: &str,
) -> Result<(), Fail> {
    let n = *rw.doc.node(id);
    let copy = rw.b.begin_node(n.kind, n.span)?;
    rw.moved_to(id, copy);
    rw.b.set_value(copy, n.value);
    rw.b.add_flags(copy, n.flags);
    let text_id = rw.b.add_text(text)?;
    rw.b.set_text(copy, text_id);
    // The file has become literal source, so renderers must no longer treat it as a request.
    rw.copy_attributes(id, copy, |key, name| {
        key != guidedoc::AttributeKey::Custom || name != guidedoc::SCORE_FILE_ATTRIBUTE
    })?;
    rw.b.copy_payload(rw.doc, id, copy)?;
    for child in rw.doc.children(id) {
        rw.copy_node(page, child)?;
    }
    rw.b.end_node(copy)
}

fn read(
    page: &mut Page<'_>,
    doc: Document<'_>,
    id: NodeId,
    ledger: &gd_host::Ledger<'_>,
    external_text: &mut Option<String>,
) -> Result<gd_lilypond::Loaded, String> {
    let mut origin = source::origin(page, doc, id);
    let mut text = std::borrow::Cow::Borrowed(doc.node_text(id));
    if let Some(name) = doc.custom_attribute(id, guidedoc::SCORE_FILE_ATTRIBUTE) {
        let file = source::read(page, &origin, name).map_err(|e| e.message)?;
        origin = file.name.into();
        *external_text = Some(file.text.clone());
        text = std::borrow::Cow::Owned(file.text);
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
    external_text: &mut Option<String>,
) -> Result<Drawing<'d>, String> {
    let ledger = page.ledger.budget().map(gd_host::Ledger::new).unwrap_or_default();
    let result = draw_held(page, doc, id, fonts, &ledger, external_text);
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
    external_text: &mut Option<String>,
) -> Result<Drawing<'d>, String> {
    let loaded = read(page, doc, id, ledger, external_text)?;
    let mut reserve = |bytes| ledger.charge(bytes);
    let mut options = options::dimensions(page, doc, id)?;
    options.reserve = Some(&mut reserve);
    let charge_scratch = |bytes| ledger.charge(bytes);
    let release_scratch = |bytes| ledger.discharge(bytes);
    options.scratch =
        gd_engrave::Scratch { reserve: Some(&charge_scratch), release: Some(&release_scratch) };
    let engraving = page
        .engraver
        .engrave_score(&loaded.score, fonts, &mut options)
        .map_err(|e| e.to_string())?;
    note_font_sources(page, &engraving);
    for error in &engraving.warnings {
        diagnostics::report(page, doc, id, error, &loaded.files);
    }
    if page.format == OutputFormat::Pdf && engraving.width > options.line_width + 0.01 {
        diagnostics::scaled(page, doc, id);
    }
    let alt = diagnostics::description(page, &engraving.summary);
    let parts = pdf_parts(page, doc, id, &engraving, &alt, ledger)?;
    let path = if let Some(first) = parts.first() {
        first.clone()
    } else {
        assets::save(page, doc, id, &engraving, None, &alt, ledger)?
    };
    let mut narrow = None;
    // The narrow pass charges its own estimate to this score's ledger while the wide
    // charge is still held. The double count is conservative and ends with the ledger.
    if page.format == OutputFormat::Html && engraving.width > 225.0 {
        options.line_width = 225.0;
        options.staff_size = options.staff_size.min(14.0);
        if let Ok(drawing) = page.engraver.engrave_score(&loaded.score, fonts, &mut options) {
            note_font_sources(page, &drawing);
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

fn note_font_sources(page: &mut Page<'_>, engraving: &Engraving) {
    for system in &engraving.systems {
        for mark in &system.marks {
            if let gd_engrave::Mark::Text { run, .. } = &mark.mark {
                for source in &run.sources {
                    page.trace.note_key(gd_env::LookupKind::FileContent, source);
                }
            }
        }
    }
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
