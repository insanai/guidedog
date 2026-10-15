//! A Guidedoc tree brought into the common form.

use guidedoc::{
    AdmonitionKind, AttributeKey, Document, LinkKind, ListStyle, NodeId, NodeKind, ROOT,
    admonition_name, kind_name,
};

use super::norm::{Norm, link_key, xref_key};

/// Converts a document.
pub fn from_guidedoc(doc: Document<'_>) -> Norm {
    let mut out = Norm::new("document");
    children(doc, ROOT, &mut out);
    out
}

fn children(doc: Document<'_>, id: NodeId, out: &mut Norm) {
    for c in doc.children(id) {
        node(doc, c, out);
    }
}

fn node(doc: Document<'_>, id: NodeId, out: &mut Norm) {
    let n = doc.node(id);
    match n.kind {
        NodeKind::Text => out.text(doc.node_text(id)),
        NodeKind::SoftBreak | NodeKind::HardBreak => out.text("\n"),
        NodeKind::Admonition => {
            let a = out.child("admonition");
            let kind = AdmonitionKind::from_value(n.value).unwrap_or_default();
            a.attr("kind", admonition_name(kind));
            children(doc, id, a);
        }
        NodeKind::CodeBlock
        | NodeKind::MathBlock
        | NodeKind::Comment
        | NodeKind::RawBlock
        | NodeKind::Math
        | NodeKind::Code => {
            let k = if n.kind == NodeKind::RawBlock { "raw" } else { kind_name(n.kind) };
            let c = out.child(k);
            if let Some(lang) = doc.attribute(id, AttributeKey::Language) {
                c.attr("language", lang);
            }
            if n.kind == NodeKind::RawBlock {
                c.attr("format", doc.attribute(id, AttributeKey::Format).unwrap_or(""));
            }
            let mut text = doc.node_text(id).trim_end_matches('\n');
            if n.kind == NodeKind::MathBlock {
                text = text.trim();
            }
            c.text(text);
        }
        _ => element(doc, id, out),
    }
}

/// Handles the kinds that are dropped or unwrapped; false for the others.
fn special(doc: Document<'_>, id: NodeId, out: &mut Norm) -> bool {
    match doc.kind(id) {
        NodeKind::RawInline => {
            let c = out.child("raw");
            c.attr("format", doc.attribute(id, AttributeKey::Format).unwrap_or(""));
            c.text(doc.node_text(id));
        }
        NodeKind::Index | NodeKind::SubstitutionDefinition => {}
        NodeKind::SubstitutionReference => {
            if let Some(l) = doc.link_info(id).filter(|l| l.kind == LinkKind::Internal) {
                children(doc, l.resolved, out);
            }
        }
        NodeKind::Toctree => {
            out.child("toctree");
        }
        NodeKind::FieldList => {
            let docinfo = doc
                .attributes(id)
                .iter()
                .any(|a| a.key == AttributeKey::Class && doc.text(a.value).contains("docinfo"));
            return docinfo;
        }
        // Sphinx's literal_strong and literal_emphasis, which the Docutils side unwraps.
        NodeKind::Strong | NodeKind::Emphasis => {
            if doc.custom_attribute(id, "literal") != Some("1") {
                return false;
            }
            children(doc, id, out);
        }
        NodeKind::FootnoteReference => {
            out.child("footnote-reference");
        }
        NodeKind::Role => children(doc, id, out),
        NodeKind::Contents => {
            out.child("contents");
        }
        NodeKind::Directive => {
            let name = doc.attribute(id, AttributeKey::Name).unwrap_or("");
            if matches!(name, "header" | "footer" | "acks") {
                children(doc, id, out.child(name));
            }
        }
        NodeKind::Preformatted => {
            let text_only = doc
                .children(id)
                .all(|c| matches!(doc.kind(c), NodeKind::Text | NodeKind::SoftBreak));
            if !text_only {
                return false;
            }
            children(doc, id, out.child("code-block"));
        }
        _ => return false,
    }
    true
}

fn element(doc: Document<'_>, id: NodeId, out: &mut Norm) {
    if special(doc, id, out) {
        return;
    }
    let kind = doc.kind(id);
    let c = out.child(kind_name(kind));
    match kind {
        NodeKind::Link => {
            let l = doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
            if l.kind == LinkKind::Internal {
                c.kind = "xref".into();
                c.attr("target", "#");
                return;
            }
            c.attr("dest", &link_key(doc.text(l.destination)));
        }
        NodeKind::Xref => {
            let l = doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
            c.attr("target", &xref_key(doc.text(l.name)));
        }
        NodeKind::Image => {
            let l = doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
            let uri = doc.text(l.destination);
            c.attr("uri", uri.strip_prefix('/').unwrap_or(uri));
            return;
        }
        NodeKind::List => c.attr("style", list_style(doc.list_info(id).style)),
        _ => {}
    }
    children(doc, id, c);
}

fn list_style(s: ListStyle) -> &'static str {
    match s {
        ListStyle::Bullet => "bullet",
        ListStyle::Arabic => "arabic",
        ListStyle::LowerAlpha => "loweralpha",
        ListStyle::UpperAlpha => "upperalpha",
        ListStyle::LowerRoman => "lowerroman",
        ListStyle::UpperRoman => "upperroman",
    }
}
