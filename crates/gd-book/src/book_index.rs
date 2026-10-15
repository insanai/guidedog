//! The book's general index (book_index.odin).

use std::collections::HashMap;
use std::fmt::Write as _;

use gd_env::DocId;
use gd_resolve::{BuildView, IndexLink, create_index};

use crate::compose::{typst_markup, typst_string};

/// Ends the book with the general index: the terms `create_index` collects for
/// genindex.html, each link the label of its target in the book (`key-anchor`), which
/// the preamble's `gd-index` turns into the pages those labels fall on. Writes nothing
/// when the project has no index entries.
pub(crate) fn write_book_index(view: &BuildView<'_>, out: &mut String, docs: &[DocId]) {
    let groups = create_index(view);
    if groups.is_empty() {
        return;
    }
    // create_index links to page#anchor; the book's label for it is key-anchor.
    let keys: HashMap<String, &str> = docs
        .iter()
        .filter(|&&id| !view.doc(id).failed)
        .map(|&id| (view.page_name(&view.doc(id).docname), view.doc_key(id)))
        .collect();
    let title = typst_markup(&view.ui.gettext("Index"));
    let _ = write!(out, "\n#heading(level: 1)[{title}]\n#gd-index(\n");
    for group in &groups {
        let _ = writeln!(out, "({}, (", typst_string(&group.name));
        for term in &group.terms {
            let _ = write!(out, "({}, ", typst_string(&term.name));
            write_index_links(out, &term.links, &keys);
            out.push_str(", (");
            for sub in &term.subs {
                let _ = write!(out, "({}, ", typst_string(&sub.name));
                write_index_links(out, &sub.links, &keys);
                out.push_str("),");
            }
            out.push_str(")),\n");
        }
        out.push_str(")),\n");
    }
    out.push_str(")\n");
}

/// Writes `((main, label), ...)` for the links that land in the book.
fn write_index_links(out: &mut String, links: &[IndexLink], keys: &HashMap<String, &str>) {
    out.push('(');
    for link in links {
        let (page, anchor) = link.uri.split_once('#').unwrap_or((&link.uri, ""));
        let Some(key) = keys.get(page) else { continue };
        let label = if anchor.is_empty() { key.to_string() } else { format!("{key}-{anchor}") };
        let _ = write!(out, "({}, {}),", link.main, typst_string(&label));
    }
    out.push(')');
}
