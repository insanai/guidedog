//! The html and dirhtml builders: one unit per document, kept when its inputs and
//! lookups are unchanged; the rest rendered on worker threads in batches and written in
//! docname order; then the site unit and the search index. Also the singlehtml builder.

use gd_env::{Asset, Digest, DocId, Lookup, LookupKind, Rendered};
use gd_host::{Ledger, Problem};
use gd_outputs::{Generation, SITE_UNIT, UnitRecord};
use gd_report::Report;
use gd_resolve::{lookup_answer, page_inputs};

use crate::digests::html_settings;
use crate::escape::push_escaped;
use crate::render::render_bodies;
use crate::search::write_search;
use crate::site::Site;
use crate::site_files::{additional_page_replaces, write_site_files};

/// Documents rendered per batch and thread: a batch's bodies are held until written.
const BATCH_PER_THREAD: usize = 16;

/// Writes the html or dirhtml output.
pub(crate) fn write_html(
    site: &Site<'_, '_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let mut assets = Vec::new();
    write_documents(site, generation, reports, &mut assets)?;
    crate::collections::write(site, generation, reports, &mut assets)?;
    write_site_unit(site, generation, None, &assets, reports)?;
    write_search(site.view, generation, reports)
}

/// Writes every document's page, reusing unchanged ones. The images and downloads the
/// pages need are added to `assets`. Fails when a page cannot be written (after every
/// page was tried, so each failure is reported) or a template fails.
fn write_documents(
    site: &Site<'_, '_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
    assets: &mut Vec<Asset>,
) -> Result<(), Problem> {
    let view = site.view;
    let common = html_settings(view);
    let mut todo: Vec<(DocId, Digest)> = Vec::new();
    for (id, d) in view.env.iter().filter(|(_, d)| !d.failed) {
        let inputs = page_inputs(view, id, &common);
        let answer = |lookup: &Lookup| match lookup.kind {
            LookupKind::Template => site.layout.template_digest(&lookup.key),
            LookupKind::Navigation => {
                crate::navigation_trace::answer(view, &d.docname, lookup, &site.articles)
            }
            _ => lookup_answer(view, &d.docname, lookup),
        };
        if let Some(old) = generation.reuse(&d.docname, inputs, answer) {
            reports.extend(old.reports.iter().cloned());
            assets.extend(old.assets.iter().cloned());
        } else {
            todo.push((id, inputs));
        }
    }
    let batch = view.options.jobs.threads(todo.len()) * BATCH_PER_THREAD;
    for chunk in todo.chunks(batch.max(1)) {
        let docs: Vec<DocId> = chunk.iter().map(|(id, _)| *id).collect();
        let rendered = render_bodies(view, site.labels, &docs);
        for (&(id, inputs), page) in chunk.iter().zip(rendered) {
            let Some(page) = page else { return Err(gd_host::interrupted_problem()) };
            write_page(site, generation, id, inputs, page, reports, assets)?;
        }
        if gd_host::was_interrupted() {
            return Err(gd_host::interrupted_problem());
        }
    }
    generation.check_written()
}

/// Writes one rendered document as its unit: the page laid out (unless a page of
/// `html_additional_pages` takes its place), with its record.
fn write_page(
    site: &Site<'_, '_>,
    generation: &mut Generation,
    id: DocId,
    inputs: Digest,
    page: Rendered,
    reports: &mut Vec<Report>,
    assets: &mut Vec<Asset>,
) -> Result<(), Problem> {
    let view = site.view;
    let docname = &view.doc(id).docname;
    let mut unit = generation.unit(docname);
    let mut own = page.reports;
    if page.failed {
        reports.append(&mut own);
        unit.fail();
        return Ok(());
    }
    if !additional_page_replaces(view, docname) {
        let laid_out = site.document_page(id, &page.body, &mut own);
        let html = match laid_out {
            Ok(html) => html,
            Err(problem) => {
                reports.append(&mut own);
                return Err(problem);
            }
        };
        unit.emit_text(&view.page_name(docname), &html)?;
    }
    let lookups = page.trace.lookups().iter();
    let mut lookups: Vec<_> = lookups
        .map(|l| {
            let digest = if l.kind == LookupKind::Navigation {
                crate::navigation_trace::answer(view, docname, l, &site.articles)
            } else {
                lookup_answer(view, docname, l)
            };
            (l.clone(), digest)
        })
        .collect();
    for lookup in site.layout.take_navigation_lookups() {
        let digest = crate::navigation_trace::answer(view, docname, &lookup, &site.articles);
        lookups.push((lookup, digest));
    }
    for name in site.layout.take_template_loads() {
        let digest = site.layout.template_digest(&name);
        lookups.push((Lookup::new(LookupKind::Template, &name), digest));
    }
    assets.extend(page.assets.iter().cloned());
    reports.extend(own.iter().cloned());
    unit.finish(UnitRecord { inputs, assets: page.assets, lookups, reports: own });
    Ok(())
}

/// The site unit: the single page of singlehtml (`single`: the root document and its
/// page's body), then the files every page shares.
fn write_site_unit(
    site: &Site<'_, '_>,
    generation: &mut Generation,
    single: Option<(DocId, &str)>,
    assets: &[Asset],
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let view = site.view;
    let mut own = Vec::new();
    let mut unit = generation.unit(SITE_UNIT);
    let written = match single {
        Some((root, body)) => site
            .single_page(root, body, &mut own)
            .and_then(|html| unit.emit_text(&view.page_name(&view.config.root_doc), &html)),
        None => Ok(()),
    };
    let written = written.and_then(|()| write_site_files(site, &mut unit, assets, &mut own));
    reports.extend(own.iter().cloned());
    written?;
    unit.finish(UnitRecord { reports: own, ..UnitRecord::default() });
    Ok(())
}

/// Writes the singlehtml output: every document of the root toctree, in reading
/// order, on the root document's page. Each document's anchors carry its key, so the
/// same section name in two documents stays apart.
pub(crate) fn write_singlehtml(
    site: &Site<'_, '_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let view = site.view;
    let order: Vec<DocId> =
        view.tables.order.iter().copied().filter(|&id| !view.doc(id).failed).collect();
    let rendered = render_bodies(view, site.labels, &order);
    // The page lives until it is written; what it holds is charged to the budget.
    let kept = Ledger::new(view.budget);
    let mut body = String::new();
    let mut assets = Vec::new();
    for (&id, page) in order.iter().zip(rendered) {
        let Some(mut page) = page else { return Err(gd_host::interrupted_problem()) };
        let d = view.doc(id);
        if !page.failed && !kept.charge(2 * page.body.len()) {
            let bytes = 2 * page.body.len();
            let p = gd_host::budget_problem(
                "Adding this document to the page",
                &d.relative,
                bytes,
                Some(view.budget),
            );
            page.reports.push(p.report());
            page.failed = true;
        }
        reports.append(&mut page.reports);
        if page.failed {
            generation.unit(&d.docname).fail();
            continue;
        }
        assets.append(&mut page.assets);
        body.push_str("<section class=\"singlehtml-document\" id=\"");
        push_escaped(&mut body, &view.page_anchor(id, ""));
        body.push_str("\">\n");
        body.push_str(&page.body);
        body.push_str("\n</section>\n");
    }
    generation.check_written()?;
    let Some(root) = view.doc_named(&view.config.root_doc) else { return Ok(()) };
    report_duplicate_ids(&body, reports);
    write_site_unit(site, generation, Some((root, &body)), &assets, reports)?;
    write_search(view, generation, reports)
}

/// Warns about an id two elements of the single page share, which makes links to it
/// ambiguous.
fn report_duplicate_ids(body: &str, reports: &mut Vec<Report>) {
    let mut seen = std::collections::HashSet::new();
    let mut rest = body;
    while let Some(i) = rest.find(" id=\"") {
        rest = &rest[i + 5..];
        let Some(end) = rest.find('"') else { break };
        let id = &rest[..end];
        if !seen.insert(id) {
            reports.push(
                Report::warning(
                    "build.singlehtml.id",
                    "DUPLICATE ID",
                    &format!("Two elements of the single page have the id {id}."),
                )
                .hint("Rename one of the labels or documents whose names make this id."),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_ids_on_the_single_page_are_reported_once_each() {
        let mut reports = Vec::new();
        report_duplicate_ids("<a id=\"x\"></a><b id=\"y\"></b><c id=\"x\"></c>", &mut reports);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].message.contains("id x"));
    }
}
