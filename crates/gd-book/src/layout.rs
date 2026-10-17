//! A standalone record carries its own PDF layout and structured front matter.

use std::fmt::Write as _;

use gd_env::DocId;
use gd_link::{Discussion, discussion_record};
use gd_report::Report;
use guidedoc::Workspace;

use crate::compose::{Book, Composer, typst_string};

impl Composer<'_, '_> {
    pub(crate) fn write_article_cover(
        &self,
        out: &mut Book,
        book: &gd_conf::PdfDocument,
        root: DocId,
    ) {
        let doc = self.view.doc(root);
        let page = &doc.page;
        let title = if book.title.is_empty() { &doc.title } else { &book.title };
        let _ = write!(
            out.source,
            "#show: book.with(title: {}, lang: {}, paper: {}",
            typst_string(title),
            typst_string(&self.view.config.language),
            typst_string(paper_size(&self.view.config.pdf.paper_size))
        );
        for (key, value) in [
            ("date", &page.date),
            ("updated", &page.updated),
            ("category", &page.category),
            ("summary", &page.summary),
        ] {
            let _ = write!(out.source, ", {key}: {}", typst_string(value));
        }
        for (key, values) in [("authors", &page.authors), ("tags", &page.tags)] {
            let _ = write!(out.source, ", {key}: (");
            for value in values {
                let _ = write!(out.source, "{},", typst_string(value));
            }
            out.source.push(')');
        }
        out.source.push_str(")\n");
    }

    /// Reads the root's metadata using the same contract as the PDF link pass.
    /// Object failures fail the composition instead of silently changing its layout.
    pub(crate) fn discussion(
        &self,
        root: DocId,
        out: &mut Book,
        reports: &mut Vec<Report>,
    ) -> Option<Discussion> {
        let object = self.view.doc(root).object.as_ref()?;
        let mut workspace = Workspace::new();
        match self.view.objects.load_object(&mut workspace, object, self.view.limits) {
            Ok(doc) => discussion_record(doc),
            Err(error) => {
                out.failed += 1;
                reports.push(error.problem.report());
                None
            }
        }
    }
}

/// Uses the same supported paper identifiers for books, records and articles.
pub(crate) fn paper_size(configured: &str) -> &'static str {
    match configured {
        "letter" | "us-letter" => "us-letter",
        _ => "a4",
    }
}

/// Writes metadata as quoted Typst strings, so names and titles never become code.
pub(crate) fn record_arguments(out: &mut String, record: &Discussion, catalog: &str) {
    let id = format!("{} {}", record.prefix.to_uppercase(), record.number);
    let _ = write!(
        out,
        ", record: (id: {}, catalog: {}, fields: (",
        typst_string(&id),
        typst_string(catalog),
    );
    for (label, value) in &record.fields {
        let _ = write!(out, "({}, {}),", typst_string(label), typst_string(value));
    }
    out.push_str("))");
}

/// Tracks all local template modules and assets, including directory membership.
/// Canonical folder identities stop link cycles; confinement and a file limit bound work.
pub(crate) fn template_inputs(
    view: &gd_resolve::BuildView<'_>,
    out: &mut Book,
    reports: &mut Vec<Report>,
) {
    let mut pending = vec![view.config.source_dir.join("_templates")];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(folder) = pending.pop() {
        if gd_host::confinement(view.roots, &folder) != gd_host::Confinement::Inside {
            continue;
        }
        let Ok(canonical) = std::fs::canonicalize(&folder) else { continue };
        if !seen.insert(canonical) {
            continue;
        }
        out.files.insert(folder.clone());
        let entries = match std::fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) => {
                template_failure(out, reports, &folder, &error.to_string());
                return;
            }
        };
        for entry in entries {
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(error) => {
                    template_failure(out, reports, &folder, &error.to_string());
                    return;
                }
            };
            if out.files.len() >= 16_384 {
                template_failure(out, reports, &folder, "More than 16384 template inputs.");
                return;
            }
            if gd_host::confinement(view.roots, &path) != gd_host::Confinement::Inside {
                continue;
            }
            out.files.insert(path.clone());
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
}

fn template_failure(out: &mut Book, reports: &mut Vec<Report>, path: &std::path::Path, why: &str) {
    out.failed += 1;
    reports.push(Report {
        path: path.display().to_string(),
        ..Report::error("build.template", "CANNOT TRACK PDF TEMPLATES", why)
            .hint("Keep template files readable and move unrelated files out of _templates.")
    });
}
