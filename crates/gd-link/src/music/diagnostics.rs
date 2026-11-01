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
    if error.span.is_unplaced() {
        crate::page::locate(doc, doc.node(id).span, &mut report);
    } else if error.span.file.0 == 0
        && doc.custom_attribute(id, guidedoc::SCORE_FILE_ATTRIBUTE).is_none()
    {
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
        let name = super::keys::key_name(&page.config().language, page.view.ui, key);
        alt.push(' ');
        alt.push_str(&page.view.ui.gettext("Key signature %s.").replace("%s", &name));
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
    if summary.measures.0 == summary.measures.1 {
        alt.push_str(
            &page.view.ui.gettext("Measure %s.").replace("%s", &summary.measures.0.to_string()),
        );
    } else {
        alt.push_str(
            &page
                .view
                .ui
                .gettext("Measures %s.")
                .replace("%s", &format!("{}-{}", summary.measures.0, summary.measures.1)),
        );
    }
    if summary.lyrics {
        alt.push(' ');
        alt.push_str(&page.view.ui.gettext("With lyrics."));
    }
    alt
}
