//! The bodies of the pages made from the whole project: the search page, the general
//! index (as Sphinx's genindex.html, genindex-split.html and genindex-single.html) and
//! the Python module index (as Sphinx's domainindex.html).

use gd_resolve::{BuildView, IndexGroup, IndexLink, IndexTerm, ModindexEntry, ModindexLetter};

use crate::escape::push_escaped;

/// The search page's body: a form the theme's script answers.
pub(crate) fn search_body(view: &BuildView<'_>) -> String {
    let search = view.ui.gettext("Search");
    let mut out = String::from("<h1 id=\"search-documentation\">");
    push_escaped(&mut out, &search);
    out.push_str(
        "</h1>\n<form class=\"search-page\" action=\"\" method=\"get\">\n\
         <input id=\"search-page-input\" type=\"search\" name=\"q\" aria-label=\"",
    );
    push_escaped(&mut out, &search);
    out.push_str("\">\n<button type=\"submit\">");
    push_escaped(&mut out, &search);
    out.push_str(
        "</button>\n</form>\n<ul id=\"search-page-results\" \
         class=\"search-page-results\"></ul>\n<noscript><p>",
    );
    push_escaped(&mut out, &view.ui.gettext("Search needs JavaScript. Use the index instead."));
    out.push_str("</p></noscript>");
    out
}

/// The page of the whole index: Sphinx's genindex.html.
pub(crate) fn genindex_body(view: &BuildView<'_>, groups: &[IndexGroup]) -> String {
    let mut out = String::from("<h1 id=\"index\">");
    push_escaped(&mut out, &view.ui.gettext("Index"));
    out.push_str("</h1>\n\n<div class=\"genindex-jumpbox\">\n");
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            out.push_str(" | ");
        }
        out.push_str("<a href=\"#");
        push_escaped(&mut out, &g.name);
        out.push_str("\"><strong>");
        push_escaped(&mut out, &g.name);
        out.push_str("</strong></a>\n");
    }
    out.push_str("</div>\n");
    for g in groups {
        out.push_str("<h2 id=\"");
        push_escaped(&mut out, &g.name);
        out.push_str("\">");
        push_escaped(&mut out, &g.name);
        out.push_str("</h2>\n");
        write_index_table(&mut out, &g.terms);
    }
    out
}

/// The letters page of a split index: Sphinx's genindex-split.html.
pub(crate) fn genindex_split_body(view: &BuildView<'_>, groups: &[IndexGroup]) -> String {
    let mut out = String::from("<h1 id=\"index\">");
    push_escaped(&mut out, &view.ui.gettext("Index"));
    out.push_str("</h1>\n\n<h2>");
    push_escaped(&mut out, &view.ui.gettext("Index pages by letter"));
    out.push_str("</h2>\n\n<div class=\"genindex-jumpbox\">\n<p>");
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            out.push_str("| ");
        }
        out.push_str("<a href=\"");
        push_escaped(&mut out, &split_index_page(&g.name));
        out.push_str("\"><strong>");
        push_escaped(&mut out, &g.name);
        out.push_str("</strong></a>\n");
    }
    out.push_str("<br>\n<a href=\"genindex-all.html\"><strong>");
    push_escaped(&mut out, &view.ui.gettext("Full index on one page"));
    out.push_str("</strong></a></p>\n</div>\n");
    out
}

/// One letter's page of a split index: Sphinx's genindex-single.html.
pub(crate) fn genindex_single_body(view: &BuildView<'_>, group: &IndexGroup) -> String {
    let mut out = String::from("<h1 id=\"index\">");
    // The message holds markup (&ndash;), so only the letter is escaped.
    let title = view.ui.gettext("Index &ndash; %(key)s");
    let (before, after) = title.split_once("%(key)s").unwrap_or((&title, ""));
    out.push_str(before);
    push_escaped(&mut out, &group.name);
    out.push_str(after);
    out.push_str("</h1>\n");
    write_index_table(&mut out, &group.terms);
    out
}

/// A letter's page: `genindex-A.html`, as Sphinx names it.
pub(crate) fn split_index_page(name: &str) -> String {
    format!("genindex-{name}.html")
}

/// Writes a group's terms in two columns, split as Sphinx's `slice_index` does:
/// balanced by lines, counting subterms.
fn write_index_table(out: &mut String, terms: &[IndexTerm]) {
    out.push_str("<table style=\"width: 100%\" class=\"indextable genindextable\"><tr>\n");
    let total: usize = terms.iter().map(|t| 1 + t.subs.len()).sum();
    let per = total / 2;
    let mut split = 0;
    let mut count = 0;
    for t in terms {
        count += 1 + t.subs.len();
        split += 1;
        if count >= per {
            break;
        }
    }
    for column in [&terms[..split], &terms[split..]] {
        if column.is_empty() {
            continue;
        }
        out.push_str("  <td style=\"width: 33%; vertical-align: top;\"><ul>\n");
        for t in column {
            out.push_str("      <li>");
            write_index_links(out, &t.name, &t.links);
            if !t.subs.is_empty() {
                out.push_str("\n      <ul>\n");
                for s in &t.subs {
                    out.push_str("        <li>");
                    write_index_links(out, &s.name, &s.links);
                    out.push_str("</li>\n");
                }
                out.push_str("      </ul>");
            }
            out.push_str("</li>\n");
        }
        out.push_str("  </ul></td>\n");
    }
    out.push_str("</tr></table>\n");
}

/// The template's `indexentries` macro: the name links to the first target, the
/// others follow as `[1]`, `[2]`; main links are bold.
fn write_index_links(out: &mut String, name: &str, links: &[IndexLink]) {
    if links.is_empty() {
        push_escaped(out, name);
        return;
    }
    for (i, link) in links.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str("<a href=\"");
        push_escaped(out, &link.uri);
        out.push_str("\">");
        if link.main {
            out.push_str("<strong>");
        }
        if i == 0 {
            push_escaped(out, name);
        } else {
            out.push_str(&format!("[{i}]"));
        }
        if link.main {
            out.push_str("</strong>");
        }
        out.push_str("</a>");
    }
}

/// The module index page's body, in the markup of Sphinx's domainindex.html.
pub(crate) fn modindex_body(view: &BuildView<'_>, letters: &[ModindexLetter]) -> String {
    let mut out = String::from("<h1 id=\"python-module-index\">");
    push_escaped(&mut out, &view.ui.gettext("Python Module Index"));
    out.push_str("</h1>\n<div class=\"modindex-jumpbox\">");
    for (i, l) in letters.iter().enumerate() {
        if i > 0 {
            out.push_str(" | ");
        }
        out.push_str("<a href=\"#cap-");
        push_escaped(&mut out, &l.letter);
        out.push_str("\"><strong>");
        push_escaped(&mut out, &l.letter);
        out.push_str("</strong></a>");
    }
    out.push_str("</div>\n<table class=\"indextable modindextable\">\n");
    let mut group = 0;
    for l in letters {
        out.push_str("<tr class=\"pcap\"><td></td><td>&#160;</td><td></td></tr>\n");
        out.push_str("<tr class=\"cap\" id=\"cap-");
        push_escaped(&mut out, &l.letter);
        out.push_str("\"><td></td><td><strong>");
        push_escaped(&mut out, &l.letter);
        out.push_str("</strong></td><td></td></tr>\n");
        for e in &l.entries {
            if e.group == 1 {
                group += 1;
            }
            write_modindex_row(view, &mut out, e, group);
        }
    }
    out.push_str("</table>\n");
    out
}

fn write_modindex_row(view: &BuildView<'_>, out: &mut String, e: &ModindexEntry, group: u32) {
    if e.group == 2 {
        out.push_str(&format!("<tr class=\"cg-{group}\"><td></td><td>&#160;&#160;&#160;"));
    } else {
        out.push_str("<tr><td></td><td>");
    }
    if let Some(doc) = e.doc {
        // The index is at the output root, as genindex.html is.
        let page = view.page_name(&view.doc(doc).docname);
        out.push_str("<a href=\"");
        push_escaped(out, &format!("{page}#{}", view.page_anchor(doc, &e.anchor)));
        out.push_str("\">");
    }
    out.push_str("<code class=\"xref\">");
    push_escaped(out, &e.name);
    out.push_str("</code>");
    if e.doc.is_some() {
        out.push_str("</a>");
    }
    if !e.platform.is_empty() {
        out.push_str(" <em>(");
        push_escaped(out, &e.platform);
        out.push_str(")</em>");
    }
    out.push_str("</td><td>");
    if e.deprecated {
        out.push_str("<strong>");
        push_escaped(out, &view.ui.gettext("Deprecated"));
        out.push_str(":</strong> ");
    }
    out.push_str("<em>");
    push_escaped(out, &e.synopsis);
    out.push_str("</em></td></tr>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(name: &str, subs: usize) -> IndexTerm {
        let link = IndexLink { main: false, uri: format!("{name}.html#x") };
        IndexTerm {
            name: name.to_string(),
            links: vec![link],
            subs: (0..subs)
                .map(|i| gd_resolve::IndexSub { name: format!("s{i}"), links: Vec::new() })
                .collect(),
            ..IndexTerm::default()
        }
    }

    #[test]
    fn index_tables_balance_two_columns_by_lines() {
        let mut out = String::new();
        write_index_table(&mut out, &[term("a", 3), term("b", 0), term("c", 0)]);
        assert_eq!(out.matches("<td ").count(), 2);
        let second = &out[out.rfind("<td ").unwrap_or(0)..];
        assert!(second.contains("b.html") && !second.contains("a.html"));
    }

    #[test]
    fn later_links_are_numbered_and_main_ones_bold() {
        let mut out = String::new();
        let links = [
            IndexLink { main: true, uri: "a.html#1".into() },
            IndexLink { main: false, uri: "b.html#2".into() },
        ];
        write_index_links(&mut out, "f()", &links);
        assert_eq!(
            out,
            "<a href=\"a.html#1\"><strong>f()</strong></a>, <a href=\"b.html#2\">[1]</a>"
        );
    }
}
