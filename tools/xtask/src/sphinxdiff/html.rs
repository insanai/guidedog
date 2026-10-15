//! Plain scanning of the HTML both builders write: the document's body of a page, its
//! attribute values, internal links, numbers and link texts. Both builders write
//! regular, machine-made HTML, so searching for fixed spellings is enough and keeps the
//! comparison free of an HTML parser.

use std::collections::HashSet;

/// Which builder wrote a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Sphinx,
    Guidedog,
}

/// The document's body of a page, the theme's chrome left out: Sphinx's basic theme puts
/// it in `<div class="body" role="main">`, Guidedog's in `<article class="gd-article">`.
/// Returns `""` when the page has no such element.
pub(crate) fn body(html: &str, side: Side) -> &str {
    let (start, stops): (&str, &[&str]) = match side {
        Side::Sphinx => (
            "<div class=\"body\" role=\"main\">",
            &["<div class=\"sphinxsidebar\"", "<div class=\"clearer\"", "<div class=\"footer\""],
        ),
        Side::Guidedog => ("<article class=\"gd-article\">", &["</article>"]),
    };
    let Some(at) = html.find(start) else { return "" };
    let body = &html[at + start.len()..];
    let end = stops.iter().filter_map(|stop| body.find(stop)).min().unwrap_or(body.len());
    &body[..end]
}

/// The values of the attribute `name` in a piece of HTML, sorted.
pub(crate) fn attribute_values(html: &str, name: &str) -> Vec<String> {
    let needle = format!(" {name}=\"");
    let mut values = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find(&needle) {
        rest = &rest[at + needle.len()..];
        let Some(end) = rest.find('"') else { break };
        values.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    values.sort();
    values
}

/// The internal links of a page's body as `page#fragment` from the site's root, a
/// target linked twice listed twice (see [`counted`]); links to other sites, downloads
/// and images are left out.
pub(crate) fn link_targets(html: &str, page: &str) -> Vec<String> {
    let hrefs = attribute_values(html, "href");
    counted(hrefs.iter().filter_map(|href| internal_target(href, page)).collect())
}

/// Resolves a link on `page` to `page#fragment` from the site's root, or `None` when it
/// leads elsewhere: another site, a mail address, a download or an image.
pub(crate) fn internal_target(href: &str, page: &str) -> Option<String> {
    let href = href.replace("&amp;", "&");
    let href = href.trim();
    let elsewhere = ["_downloads/", "_images/"].iter().any(|dir| href.contains(dir));
    if href.is_empty() || href.contains("://") || href.starts_with("mailto:") || elsewhere {
        return None;
    }
    let (target, fragment) = href.split_once('#').unwrap_or((href, ""));
    let mut resolved = if target.is_empty() { page.to_string() } else { resolve(page, target) };
    if !fragment.is_empty() {
        resolved.push('#');
        resolved.push_str(fragment);
    }
    Some(resolved)
}

/// Resolves a relative `target` against the folder of `page`, both with `/`.
fn resolve(page: &str, target: &str) -> String {
    let mut parts: Vec<&str> = match page.rsplit_once('/') {
        Some((dir, _)) => dir.split('/').collect(),
        None => Vec::new(),
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

/// The numbers a page's body shows: section numbers (`section 2.1.`) and the numbers of
/// figures, tables and code blocks (`caption Fig. 2.1`). Both builders write them in
/// spans of the same classes. A number shown twice is listed twice (see [`counted`]).
pub(crate) fn numbers(html: &str) -> Vec<String> {
    let sections = span_texts(html, "section-number").into_iter().map(|t| format!("section {t}"));
    let captions = span_texts(html, "caption-number").into_iter().map(|t| format!("caption {t}"));
    counted(sections.chain(captions).collect())
}

/// The `:numref:` references of a Sphinx page as `target text`: the links whose content
/// is a `std-numref` span. Sorted.
pub(crate) fn numref_texts(html: &str, page: &str) -> Vec<String> {
    let numrefs = links(html, page).filter(|link| link.inner.contains("class=\"std std-numref\""));
    let mut texts: Vec<String> = numrefs.map(|link| link.described()).collect();
    texts.sort();
    texts
}

/// Every internal link of a page as `target text`, the text without its markup. Sorted.
/// A Guidedog page is checked to have each numref text Sphinx shows.
pub(crate) fn link_texts(html: &str, page: &str) -> Vec<String> {
    let mut texts: Vec<String> = links(html, page).map(|link| link.described()).collect();
    texts.sort();
    texts
}

/// One `<a href="...">` element that links within the site.
struct Link<'a> {
    /// The target from the site's root.
    target: String,
    /// The HTML between the start and end tags.
    inner: &'a str,
}

impl Link<'_> {
    fn described(&self) -> String {
        format!("{} {}", self.target, plain(self.inner))
    }
}

/// The internal links of a piece of HTML, in order.
fn links<'a>(html: &'a str, page: &'a str) -> impl Iterator<Item = Link<'a>> + 'a {
    let mut rest = html;
    std::iter::from_fn(move || {
        loop {
            let at = rest.find("<a ")?;
            rest = &rest[at..];
            let close = rest.find('>')?;
            let end = rest.find("</a>").filter(|&end| end >= close)?;
            let (tag, inner) = (&rest[..close], &rest[close + 1..end]);
            rest = &rest[end..];
            let Some(href_at) = tag.find(" href=\"") else { continue };
            let href = &tag[href_at + 7..];
            let href = &href[..href.find('"').unwrap_or(0)];
            if let Some(target) = internal_target(href, page) {
                return Some(Link { target, inner });
            }
        }
    })
}

/// The text of a piece of HTML: tags left out, `&lt;`, `&gt;` and `&amp;` read, and
/// runs of white space made one space.
pub(crate) fn plain(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The text of each `<span class="NAME">`, in order.
fn span_texts(html: &str, name: &str) -> Vec<String> {
    let needle = format!("<span class=\"{name}\">");
    let mut texts = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find(&needle) {
        rest = &rest[at + needle.len()..];
        let Some(end) = rest.find("</span>") else { break };
        texts.push(plain(&rest[..end]));
        rest = &rest[end..];
    }
    texts
}

/// Sorts a list and numbers repeated entries (`section 1.`, `section 1. (2)`), so a set
/// comparison sees how often each appears.
pub(crate) fn counted(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    let mut out = Vec::with_capacity(items.len());
    let mut repeat = 0;
    for (i, item) in items.iter().enumerate() {
        repeat = if i > 0 && items[i - 1] == *item { repeat + 1 } else { 1 };
        out.push(if repeat == 1 { item.clone() } else { format!("{item} ({repeat})") });
    }
    out
}

/// The entries of `a` that `b` does not have, in `a`'s order.
pub(crate) fn only<'a>(a: &'a [String], b: &[String]) -> Vec<&'a str> {
    let others: HashSet<&str> = b.iter().map(String::as_str).collect();
    a.iter().map(String::as_str).filter(|item| !others.contains(item)).collect()
}
