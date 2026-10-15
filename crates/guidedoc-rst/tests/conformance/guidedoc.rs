//! Guidedoc documents in the common form.

use guidedoc::{
    AdmonitionKind, AttributeKey, Document, LinkKind, ListStyle, NodeFlag, NodeId, NodeKind, ROOT,
    admonition_name, kind_name,
};
use guidedoc_rst_inline::footnote_symbol;

use super::norm::*;

struct State<'d> {
    doc: Document<'d>,
    signing: bool,
    sectnum: bool,
    expanding: usize,
    /// Header, footer, and meta, wherever they appear.
    decor: Vec<Norm>,
}

pub fn from_guidedoc(doc: Document<'_>) -> Norm {
    let mut st = State { doc, signing: false, sectnum: false, expanding: 0, decor: Vec::new() };
    let mut out = Norm::new("document");
    children(&mut st, ROOT, &mut out);
    out.children.append(&mut st.decor);
    if st.sectnum {
        out.attr("sectnum", "1");
    }
    hoist(&mut out);
    out
}

fn children(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    for c in doc.children(id) {
        node(st, c, out);
    }
}

fn classes(st: &State<'_>, id: NodeId, n: &mut Norm, extra: &str, implicit: &[&str]) {
    let mut s = String::new();
    for a in st.doc.attributes(id).iter().filter(|a| a.key == AttributeKey::Class) {
        s.push(' ');
        s.push_str(st.doc.text(a.value));
    }
    s.push(' ');
    s.push_str(extra);
    set_classes(n, &s, implicit);
}

fn source_of<'d>(st: &State<'d>, id: NodeId) -> &'d str {
    st.doc.source_text(st.doc.node(id).span)
}

/// A child of `kind` with the node's classes and children.
fn plain_node(st: &mut State<'_>, id: NodeId, out: &mut Norm, kind: &str) {
    let n = out.child(kind);
    classes(st, id, n, "", &[]);
    children(st, id, n);
}

fn node(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let n = doc.node(id);
    match n.kind {
        NodeKind::Text => out.text(doc.node_text(id)),
        NodeKind::SoftBreak | NodeKind::HardBreak => out.text("\n"),
        NodeKind::List => list(st, id, out),
        NodeKind::CodeBlock
        | NodeKind::DoctestBlock
        | NodeKind::MathBlock
        | NodeKind::Comment
        | NodeKind::RawBlock => {
            leaf_text(st, id, out);
        }
        NodeKind::Preformatted => preformatted(st, id, out),
        NodeKind::Table | NodeKind::Cell => table_part(st, id, out),
        NodeKind::Footnote | NodeKind::Citation => {
            let f = out.child(kind_name(n.kind));
            f.attr("label", &footnote_label(st, id));
            classes(st, id, f, "", &[]);
            children(st, id, f);
        }
        NodeKind::Contents => contents(st, id, out),
        NodeKind::Directive => directive(st, id, out),
        NodeKind::Admonition => {
            let a = out.child("admonition");
            let kind = AdmonitionKind::from_value(n.value).unwrap_or(AdmonitionKind::Generic);
            a.attr("kind", admonition_name(kind));
            classes(st, id, a, "", &[]);
            children(st, id, a);
        }
        NodeKind::Paragraph => paragraph(st, id, out),
        NodeKind::Field => field(st, id, out),
        NodeKind::Option => {
            let o = out.child("option");
            o.text(doc.node_text(id));
            children(st, id, o);
        }
        NodeKind::OptionArgument => {
            let a = out.child("option-argument");
            a.attr("delimiter", &format!("'{}'", char::from(n.value as u8)));
            a.text(doc.node_text(id));
        }
        NodeKind::Figure => {
            let f = out.child("figure");
            for (key, name) in [(AttributeKey::Width, "width"), (AttributeKey::Align, "align")] {
                if let Some(v) = doc.attribute(id, key) {
                    f.attr(name, v);
                }
            }
            classes(st, id, f, "", &[]);
            children(st, id, f);
        }
        kind => {
            if !inline(st, id, out) {
                plain_node(st, id, out, kind_name(kind));
            }
        }
    }
}

fn table_part(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let n = doc.node(id);
    let t = out.child(kind_name(n.kind));
    if n.kind == NodeKind::Table {
        t.attr("cols", &n.value.to_string());
    }
    let info = doc.cell_info(id);
    if n.kind == NodeKind::Cell && (info.row_span > 1 || info.col_span > 1) {
        t.attr("span", &format!("{}x{}", info.row_span, info.col_span));
    }
    classes(st, id, t, "", &[]);
    children(st, id, t);
}

fn list(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let l = out.child("list");
    let info = doc.list_info(id);
    if info.style == ListStyle::Bullet {
        l.attr("style", "bullet");
        let delimiter = char::from(info.delimiter).to_string();
        l.attr("bullet", doc.custom_attribute(id, "bullet").unwrap_or(&delimiter));
    } else {
        l.attr("style", info.style.name());
        let delimiter = if info.delimiter == b'(' {
            "()".to_owned()
        } else {
            char::from(info.delimiter).to_string()
        };
        l.attr("delimiter", &delimiter);
        if info.start != 1 {
            l.attr("start", &info.start.to_string());
        }
    }
    classes(st, id, l, "", &[]);
    children(st, id, l);
}

fn leaf_text(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let n = doc.node(id);
    let mut text = doc.node_text(id);
    let c = match n.kind {
        NodeKind::CodeBlock => {
            text = text.strip_suffix('\n').unwrap_or(text);
            let c = out.child("code-block");
            classes(st, id, c, doc.attribute(id, AttributeKey::Language).unwrap_or(""), &[]);
            if n.value > 0 {
                c.attr("numbered", "1");
            }
            c
        }
        NodeKind::RawBlock => {
            text = text.strip_suffix('\n').unwrap_or(text);
            let c = out.child("raw-block");
            c.attr("format", doc.attribute(id, AttributeKey::Format).unwrap_or(""));
            classes(st, id, c, "", &[]);
            c
        }
        kind => {
            let c = out.child(kind_name(kind));
            classes(st, id, c, "", &[]);
            c
        }
    };
    c.text(text);
}

/// Preformatted without markup compares as a literal block, as Docutils'
/// literal_block has no separate parsed form.
fn preformatted(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let end = doc.subtree_end(id).index();
    let markup = (id.index() + 1..end).any(|i| {
        !matches!(
            doc.kind(NodeId::from_index(i)),
            NodeKind::Text | NodeKind::SoftBreak | NodeKind::HardBreak
        )
    });
    plain_node(st, id, out, if markup { "preformatted" } else { "code-block" });
}

fn footnote_label(st: &State<'_>, id: NodeId) -> String {
    let n = st.doc.node(id);
    if n.flags.has(NodeFlag::Symbol) {
        let mut s = String::new();
        footnote_symbol(n.value, &mut s);
        return s;
    }
    if n.flags.has(NodeFlag::Auto) && n.value > 0 {
        return n.value.to_string();
    }
    st.doc.node_text(id).to_owned()
}

fn contents(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let c = out.child("contents");
    let local = doc.node(id).flags.has(NodeFlag::Local);
    match doc.attribute(id, AttributeKey::Title) {
        Some(title) => c.attr("title", &collapse(title)),
        None if !local => c.attr("title", "Contents"),
        None => {}
    }
    if local {
        c.attr("local", "1");
    }
    classes(st, id, c, "", &[]);
}

fn directive(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    match doc.attribute(id, AttributeKey::Name).unwrap_or("") {
        name @ ("header" | "footer") => {
            let mut d = Norm::new(name);
            children(st, id, &mut d);
            if !st.signing {
                st.decor.push(d);
            }
        }
        "meta" => {
            for a in doc.attributes(id).iter().filter(|a| a.key == AttributeKey::Custom) {
                let mut m = Norm::new("meta");
                for (i, f) in doc.text(a.name).split_whitespace().enumerate() {
                    match f.split_once('=') {
                        Some((k, v)) if !(i == 0 && v.is_empty()) => m.attr(k, v),
                        Some((k, _)) => m.attr("name", k),
                        None if i == 0 => m.attr("name", f),
                        None => m.attr(f, ""),
                    }
                }
                m.attr("content", doc.text(a.value));
                if !st.signing {
                    st.decor.push(m);
                }
            }
        }
        "sectnum" => st.sectnum = true,
        "target-notes" => {
            out.child("target-notes");
        }
        "title" => {}
        name => {
            let kind = format!("?directive-{name}");
            let d = out.child(&kind);
            children(st, id, d);
        }
    }
}

/// Drops the paragraph that wraps a block-level image with a target.
fn paragraph(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    if let Some(first) = doc.first_child(id) {
        let alone = doc.subtree_end(first) == doc.subtree_end(id);
        let inner = first.index() + 1;
        let link = matches!(doc.kind(first), NodeKind::Link | NodeKind::Reference);
        if alone
            && link
            && source_of(st, id).starts_with("..")
            && inner < doc.node_count()
            && doc.kind(NodeId::from_index(inner)) == NodeKind::Image
        {
            node(st, first, out);
            return;
        }
    }
    plain_node(st, id, out, "paragraph");
}

fn field(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let f = out.child("field");
    classes(st, id, f, "", &[]);
    let Some(info) = doc.custom_attribute(id, "docinfo") else {
        children(st, id, f);
        return;
    };
    f.attr("docinfo", info);
    for c in doc.children(id).filter(|&c| doc.kind(c) != NodeKind::FieldName) {
        node(st, c, f);
    }
}

fn inline(st: &mut State<'_>, id: NodeId, out: &mut Norm) -> bool {
    let doc = st.doc;
    let n = doc.node(id);
    let internal = |id| doc.link_info(id).is_some_and(|l| l.kind == LinkKind::Internal);
    match n.kind {
        NodeKind::Code => {
            let c = out.child("code");
            classes(st, id, c, doc.attribute(id, AttributeKey::Language).unwrap_or(""), &[]);
            c.text(doc.node_text(id));
        }
        NodeKind::Math => {
            let c = out.child("math");
            classes(st, id, c, "", &[]);
            c.text(doc.node_text(id));
        }
        NodeKind::RawInline => {
            let c = out.child("raw-inline");
            c.attr("format", doc.attribute(id, AttributeKey::Format).unwrap_or(""));
            classes(st, id, c, "", &[]);
            let text = doc.node_text(id);
            c.text(text.strip_suffix('\n').unwrap_or(text));
        }
        NodeKind::Role => {
            let c = out.child("role");
            let has = doc.attribute(id, AttributeKey::Class).is_some();
            classes(st, id, c, if has { "" } else { doc.node_text(id) }, &[]);
            children(st, id, c);
        }
        NodeKind::Image => image(st, id, out),
        NodeKind::Link | NodeKind::Reference => reference(st, id, out),
        NodeKind::FootnoteReference if !internal(id) => out.text(source_of(st, id)),
        NodeKind::FootnoteReference => {
            let label = footnote_label(st, id);
            out.child("footnote-reference").text(&label);
        }
        NodeKind::CitationReference if !internal(id) => out.text(source_of(st, id)),
        NodeKind::CitationReference => out.child("citation-reference").text(doc.node_text(id)),
        NodeKind::SubstitutionReference => substitution(st, id, out),
        NodeKind::Target => {
            let t = out.child("target");
            match doc.link_info(id) {
                Some(l) if l.kind == LinkKind::External => {
                    t.attr("uri", &canonical_uri(doc.text(l.destination)))
                }
                Some(l) if l.kind == LinkKind::Email => {
                    t.attr("uri", &format!("mailto:{}", doc.text(l.destination)))
                }
                _ => {}
            }
            if n.flags.has(NodeFlag::Anonymous) {
                t.attr("anonymous", "1");
            }
            classes(st, id, t, "", &[]);
        }
        _ => return false,
    }
    true
}

fn image(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let c = out.child("image");
    let uri = doc.link_info(id).map_or("", |l| doc.text(l.destination));
    c.attr("uri", uri);
    let keys = [
        (AttributeKey::Alt, "alt"),
        (AttributeKey::Width, "width"),
        (AttributeKey::Height, "height"),
        (AttributeKey::Scale, "scale"),
        (AttributeKey::Align, "align"),
    ];
    for (k, name) in keys {
        if let Some(v) = doc.attribute(id, k) {
            c.attr(name, v);
        }
    }
    classes(st, id, c, "", &[]);
}

fn reference(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let l = doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
    if l.kind == LinkKind::Unresolved {
        out.text(source_of(st, id));
        return;
    }
    let to = (l.kind == LinkKind::Internal && !st.signing).then(|| signature_of(st, l.resolved));
    let r = out.child("reference");
    classes(st, id, r, "", &[]);
    match l.kind {
        LinkKind::External | LinkKind::Project => {
            r.attr("uri", &canonical_uri(doc.text(l.destination)))
        }
        LinkKind::Email => r.attr("uri", &format!("mailto:{}", doc.text(l.destination))),
        _ => {}
    }
    if let Some(to) = to {
        r.attr("to", &to);
    }
    children(st, id, r);
}

/// Expands a resolved substitution reference into its definition's content.
fn substitution(st: &mut State<'_>, id: NodeId, out: &mut Norm) {
    let doc = st.doc;
    let l = doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
    if l.kind != LinkKind::Internal || l.resolved == guidedoc::INVALID_NODE || st.expanding > 16 {
        out.text(source_of(st, id));
        return;
    }
    let def = l.resolved;
    if doc.custom_attribute(def, "ltrim").is_some() {
        out.child(LTRIM);
    }
    st.expanding += 1;
    children(st, def, out);
    st.expanding -= 1;
    if doc.custom_attribute(def, "rtrim").is_some() {
        out.child(RTRIM);
    }
}

fn signature_of(st: &mut State<'_>, id: NodeId) -> String {
    if id == guidedoc::INVALID_NODE || id.index() >= st.doc.node_count() {
        return "missing".into();
    }
    let mut tmp = Norm::new("tmp");
    let was = std::mem::replace(&mut st.signing, true);
    node(st, id, &mut tmp);
    st.signing = was;
    match tmp.children.first_mut() {
        None => "none".into(),
        Some(first) => {
            tidy(first, false);
            signature(first)
        }
    }
}
