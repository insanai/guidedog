use gd_lilypond::SourceFiles;
use guidedoc::{Document, NodeId};

use crate::page::{Page, Warning};

pub(super) fn report(
    page: &mut Page<'_>,
    doc: Document<'_>,
    id: NodeId,
    error: &gd_score::Error,
    files: &SourceFiles,
) {
    let mut report = gd_lilypond::error_report(error, files);
    report.severity = gd_report::Severity::Warning;
    if error.span.file.0 == 0 && doc.custom_attribute(id, "lilypond-file").is_none() {
        let span = doc.node(id).span;
        let source = doc.source_str(span.source);
        let music = files.file(error.span.file).map(|f| f.text.as_str()).unwrap_or("");
        let map = doc.custom_attribute(id, "lilypond-map").unwrap_or("");
        if let (Some(start), Some(end)) =
            (mapped(music, map, error.span.start), mapped(music, map, error.span.end))
        {
            report = report.at(&page.info.relative, source, start, end);
        } else {
            crate::page::locate(doc, span, &mut report);
        }
    }
    page.reports.push(report);
}

fn mapped(source: &str, map: &str, offset: usize) -> Option<usize> {
    let head = source.get(..offset.min(source.len()))?;
    let line = head.bytes().filter(|b| *b == b'\n').count();
    let column = head.rsplit_once('\n').map(|(_, tail)| tail.len()).unwrap_or(head.len());
    map.split(',').nth(line)?.parse::<usize>().ok()?.checked_add(column)
}

pub(super) fn warn(page: &mut Page<'_>, doc: Document<'_>, id: NodeId, message: String) {
    page.warn(
        doc,
        doc.node(id).span,
        Warning {
            code: "build.lilypond",
            title: "MUSIC NOT ENGRAVED",
            category: "",
            message,
            hint: "Check the supported notation and source roots. The source is shown as code.",
        },
    );
}

pub(super) fn scaled(page: &mut Page<'_>, doc: Document<'_>, id: NodeId) {
    page.warn(doc, doc.node(id).span, Warning {
        code: "build.lilypond-scale", title: "MUSIC SCALED TO FIT", category: "",
        message: "Music exceeds the declared PDF text width and will be scaled to fit.".into(),
        hint: "Widen the music or correct the template margin declaration and lilypond_pdf_width.",
    });
}

pub(super) fn description(page: &Page<'_>, summary: &gd_engrave::Summary) -> String {
    let text = page.view.ui.ngettext(
        "Music for %s staff.",
        "Music for %s staves.",
        u64::from(summary.staves),
    );
    let mut alt = text.replace("%s", &summary.staves.to_string());
    if !summary.instruments.is_empty() {
        alt.push(' ');
        alt.push_str(&summary.instruments.join(", "));
        alt.push('.');
    }
    if let Some(key) = summary.key {
        alt.push(' ');
        alt.push_str(
            &page.view.ui.gettext("Key signature %s.").replace("%s", &key_name(page, key)),
        );
    }
    if let Some(meter) = &summary.meter {
        alt.push(' ');
        alt.push_str(
            &page
                .view
                .ui
                .gettext("Time signature %s.")
                .replace("%s", &format!("{}/{}", meter.numerator, meter.denominator)),
        );
    }
    alt.push(' ');
    alt.push_str(
        &page
            .view
            .ui
            .gettext("Measures %s.")
            .replace("%s", &format!("{}-{}", summary.measures.0, summary.measures.1)),
    );
    if summary.lyrics {
        alt.push(' ');
        alt.push_str(&page.view.ui.gettext("With lyrics."));
    }
    alt
}

fn key_name(page: &Page<'_>, key: gd_score::Key) -> String {
    use gd_score::{Alter, Mode};
    let step = ["C", "D", "E", "F", "G", "A", "B"][key.tonic.step.index() as usize];
    let accidental = match key.tonic.alter {
        Alter::DoubleFlat => "double flat",
        Alter::Flat => "flat",
        Alter::Natural => "",
        Alter::Sharp => "sharp",
        Alter::DoubleSharp => "double sharp",
    };
    let mode = match key.mode {
        Mode::Major => "major",
        Mode::Minor => "minor",
        Mode::Dorian => "Dorian",
        Mode::Phrygian => "Phrygian",
        Mode::Lydian => "Lydian",
        Mode::Mixolydian => "Mixolydian",
        Mode::Locrian => "Locrian",
    };
    let mut name = step.to_string();
    if !accidental.is_empty() {
        name.push(' ');
        name.push_str(&page.view.ui.gettext(accidental));
    }
    name.push(' ');
    name.push_str(&page.view.ui.gettext(mode));
    name
}
