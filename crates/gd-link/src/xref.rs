//! Cross-references in the tree (the AST half of xref.odin): each Xref node becomes an
//! `XrefQuery`, `gd_resolve::resolve_xref` answers it, and the node is replaced by a
//! link, its title, or its content with a warning.

use gd_env::OutputFormat;
use gd_resolve::{Resolved, Unresolved, XrefQuery, resolve_xref};
use guidedoc::{AttributeKey, Document, Fail, LinkKind, NodeId, NodeKind, SourceSpan};

use crate::page::{Page, flag, locate};
use crate::rewrite::Rewriter;

/// Replaces the Xref `id` by what it resolves to.
pub(crate) fn replace_xref(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let query = xref_query(page, doc, id);
    let span = doc.node(id).span;
    let to = match resolve_xref(&page.view, &query, &mut page.trace) {
        Ok(to) => to,
        Err(why) => {
            if why.warn {
                report(page, doc, &why, span);
            }
            // An :external: reference shows its content; any other stays an Xref,
            // which renderers show as its content.
            return if query.external {
                rw.rewrite_children(page, id)
            } else {
                rw.copy_node(page, id)
            };
        }
    };
    if let Some(warning) = &to.warning {
        report(page, doc, warning, span);
    }
    if let Some(asset) = &to.asset
        && page.format != OutputFormat::Pdf
    {
        page.assets.push(asset.clone());
    }
    if to.doc.is_none() && !to.external.is_empty() {
        return write_external(page, rw, id, &to);
    }
    let explicit = query.explicit;
    if to.no_link {
        if explicit || to.title.is_empty() || to.keep_content {
            return rw.rewrite_children(page, id);
        }
        return rw.add_plain(&to.title, span);
    }
    let download = query.reftype == "download" && !query.target.contains("://");
    let (destination, kind) = link_destination(page, &to);
    let link = rw.open_link(&destination, "", kind, span)?;
    rw.moved_to(id, link);
    // A local attachment is saved, not opened as a page (notably HTML templates).
    if download && page.format == OutputFormat::Html {
        rw.add_custom(link, "download", "1")?;
    }
    // A :numref: with an explicit title has it filled with the number.
    let titled = (!explicit || query.reftype == "numref") && !to.title.is_empty();
    if titled && !to.keep_content && query.reftype != "term" && !domain_role(&query.reftype) {
        rw.add_plain(&to.title, span)?;
    } else {
        rw.rewrite_children(page, id)?;
    }
    rw.close(link)
}

/// What the reader left on the Xref node, as a query.
fn xref_query(page: &Page<'_>, doc: Document<'_>, id: NodeId) -> XrefQuery {
    let reftype = doc.custom_attribute(id, "reftype").unwrap_or("");
    let target = doc.link_info(id).map_or("", |l| doc.text(l.name));
    let (module, class) = xref_context(doc, id, reftype);
    let mut content = String::new();
    doc.plain_text(id, &mut content);
    let context = doc
        .attributes(id)
        .iter()
        .filter(|a| a.key == AttributeKey::Custom)
        .map(|a| (doc.text(a.name).to_string(), doc.text(a.value).to_string()))
        .collect();
    XrefQuery {
        from: page.id,
        reftype: reftype.to_string(),
        target: target.to_string(),
        module: module.to_string(),
        class: class.to_string(),
        refspecific: flag(doc, id, "refspecific"),
        slug: doc.custom_attribute(id, "anchor").unwrap_or("").to_string(),
        external: flag(doc, id, "intersphinx"),
        format: page.format,
        explicit: flag(doc, id, "refexplicit"),
        content,
        inventory: doc.custom_attribute(id, "inventory").unwrap_or("").to_string(),
        context,
    }
}

/// The context a reference is written in, by its domain: the module (or package,
/// program) and the class (or object, parent scope) relative names start from.
fn xref_context<'d>(doc: Document<'d>, id: NodeId, reftype: &str) -> (&'d str, &'d str) {
    let attribute = |name: &str| doc.custom_attribute(id, name).unwrap_or("");
    let domain = reftype.split_once(':').map_or("", |(domain, _)| domain);
    match domain {
        "js" => (attribute("js:module"), attribute("js:object")),
        "c" => ("", attribute("c:parent")),
        "cpp" => ("", attribute("cpp:parent")),
        "odin" => (attribute("odin:package"), attribute("odin:type")),
        "rust" => (attribute("rust:module"), ""),
        _ if reftype == "option" || reftype == "std:option" => (attribute("std:program"), ""),
        _ => (attribute("py:module"), attribute("py:class")),
    }
}

/// Whether a role belongs to a domain (`py:func`), whose links show their content.
fn domain_role(reftype: &str) -> bool {
    reftype.contains(':') && reftype != "math:numref"
}

/// The link to a resolved target: its URL for HTML and text, a project anchor key
/// (`<document key>-<anchor>`) in the book.
pub(crate) fn link_destination(page: &Page<'_>, to: &Resolved) -> (String, LinkKind) {
    let Some(doc) = to.doc.filter(|_| to.url.is_empty()) else {
        return (to.url.clone(), LinkKind::External);
    };
    if page.format == OutputFormat::Pdf {
        let mut key = page.view.doc_key(doc).to_string();
        if !to.anchor.is_empty() {
            key.push('-');
            key.push_str(&to.anchor);
        }
        return (key, LinkKind::Project);
    }
    (page.view.page_url(Some(page.id), doc, &to.anchor), LinkKind::External)
}

/// Writes the link an inventory gives: to its URL, titled with the other project,
/// showing the reference's content or the inventory's name for it in the content's
/// own kind (code stays code), as Sphinx rebuilds contnode.
fn write_external(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
    to: &Resolved,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let span = doc.node(id).span;
    let link = rw.open_link(&to.url, &to.external, LinkKind::External, span)?;
    rw.moved_to(id, link);
    if to.keep_content {
        rw.rewrite_children(page, id)?;
    } else {
        let first = doc.first_child(id).map(|c| doc.kind(c));
        let kind = if first == Some(NodeKind::Code) { NodeKind::Code } else { NodeKind::Text };
        rw.b.add_text_leaf(kind, span, &to.title)?;
    }
    rw.close(link)
}

/// Reports a warning about a reference at the reference.
fn report(page: &mut Page<'_>, doc: Document<'_>, why: &Unresolved, span: SourceSpan) {
    let mut report = why.report(&page.info.relative);
    locate(doc, span, &mut report);
    page.reports.push(report);
}
