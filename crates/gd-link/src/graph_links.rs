//! Links in drawn graphs: an inheritance diagram's nodes carry `URL="gd-xref:py:class:
//! <name>"`, a reference the link pass resolves like `:py:class:`. In HTML the node then
//! links to the class's description, relative to `_images/`, where the drawing is
//! served; elsewhere, and when the class has no description, the URL is emptied, and
//! Graphviz draws the node without a link.

use std::borrow::Cow;

use gd_env::{BuilderKind, OutputFormat};
use gd_resolve::{XrefQuery, relative_url, resolve_xref};

use crate::page::Page;

/// The prefix of a node URL that names a class to link.
const XREF_URL: &str = "URL=\"gd-xref:";

/// The graph's source with its class references resolved, and whether any became a
/// link (the page then shows the drawing so that its links work).
pub(crate) fn resolve_graph_links<'s>(
    page: &mut Page<'_>,
    source: &'s str,
) -> (Cow<'s, str>, bool) {
    if !source.contains(XREF_URL) {
        return (Cow::Borrowed(source), false);
    }
    let mut out = String::with_capacity(source.len());
    let mut linked = false;
    let mut rest = source;
    while let Some(start) = rest.find(XREF_URL) {
        out.push_str(&rest[..start]);
        let after = &rest[start + XREF_URL.len()..];
        let Some(end) = after.find('"') else {
            rest = &rest[start..];
            break;
        };
        let reference = unescape(&after[..end]);
        let url = class_url(page, &reference).unwrap_or_default();
        linked |= !url.is_empty();
        out.push_str("URL=\"");
        out.push_str(&url.replace('\\', "\\\\").replace('"', "\\\""));
        out.push('"');
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    (Cow::Owned(out), linked)
}

/// The URL from `_images/` to the description `reference` (`py:class:<name>`) names,
/// in HTML; `None` elsewhere or when nothing describes it.
fn class_url(page: &mut Page<'_>, reference: &str) -> Option<String> {
    if page.format != OutputFormat::Html {
        return None;
    }
    let (reftype, target) = match reference.split_once(':').map(|(d, r)| (d, r.split_once(':'))) {
        Some((domain, Some((role, target)))) => (format!("{domain}:{role}"), target),
        _ => return None,
    };
    let query = XrefQuery {
        from: page.id,
        reftype,
        target: target.to_owned(),
        format: page.format,
        ..XrefQuery::default()
    };
    let to = resolve_xref(&page.view, &query, &mut page.trace).ok()?;
    if to.no_link {
        return None;
    }
    let Some(doc) = to.doc.filter(|_| to.url.is_empty()) else {
        return (!to.url.is_empty()).then(|| to.url.clone());
    };
    let view = &page.view;
    let mut url = relative_url("_images/graph.svg", &view.page_name(&view.doc(doc).docname));
    if view.kind == BuilderKind::Dirhtml {
        url = url.strip_suffix("index.html").unwrap_or(&url).to_owned();
    }
    let fragment = view.page_anchor(doc, &to.anchor);
    if !fragment.is_empty() {
        url.push('#');
        url.push_str(&fragment);
    }
    Some(url)
}

/// A quoted DOT string's text.
fn unescape(text: &str) -> String {
    text.replace("\\\"", "\"").replace("\\\\", "\\")
}
