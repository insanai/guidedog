//! Front matter: a YAML mapping between "---" lines at the start of the document. Its
//! substitutions and html_meta keys configure the document; the other keys become a
//! field list, which reStructuredText resolution turns into bibliographic data
//! (Docutils' DocInfo), as Sphinx does with MyST front matter.

use guidedoc::{Arg, AttributeKey, Fail, NodeId, NodeKind, SourceSpan, arg_span, arg_text};

use super::yaml::{Yaml, YamlKind, yaml_parse};
use super::{inline_lines, trim_right};
use crate::extension::Line;
use crate::host::Host;
use crate::messages::{FRONT_MATTER, FRONT_MATTER_TYPE, HTML_META};
use crate::settings::Rule;

const BIBLIOGRAPHIC_FIELDS: [&str; 12] = [
    "author",
    "authors",
    "organization",
    "address",
    "contact",
    "version",
    "revision",
    "status",
    "date",
    "copyright",
    "dedication",
    "abstract",
];

/// Emits the front matter and returns the lines after it.
pub(crate) fn front_matter<'l, 's>(
    h: &mut Host<'_, '_>,
    lines: &'l [Line<'s>],
) -> Result<&'l [Line<'s>], Fail> {
    let mut body = lines;
    let opens = lines.first().is_some_and(|l| trim_right(l.text) == "---");
    if h.on(Rule::FrontMatter) && opens {
        let end = lines[1..].iter().position(|l| matches!(trim_right(l.text), "---" | "..."));
        if let Some(end) = end.map(|e| e + 1) {
            body = &lines[end + 1..];
            if read_front_matter(h, &lines[1..end], lines[0])? {
                return Ok(body);
            }
        }
    }
    emit_meta(h, None, SourceSpan::new(h.ctx.source, 0, 0))?;
    Ok(body)
}

/// Reads the front matter; true when it emitted html_meta itself.
fn read_front_matter(
    h: &mut Host<'_, '_>,
    lines: &[Line<'_>],
    first: Line<'_>,
) -> Result<bool, Fail> {
    let source = h.ctx.source;
    let mut span = SourceSpan::new(source, first.offset, first.offset + first.text.len() as u32);
    let (y, root) = match yaml_parse(lines) {
        Ok(parsed) => parsed,
        Err(e) => {
            if let Some(line) = lines.get(e.line.min(lines.len().saturating_sub(1))) {
                span = SourceSpan::new(source, line.offset, line.offset + line.text.len() as u32);
            }
            h.report(FRONT_MATTER, span, guidedoc::arg_static(e.why), Arg::None);
            return Ok(false);
        }
    };
    match y.nodes[root].kind {
        YamlKind::Null => return Ok(false),
        YamlKind::Mapping => {}
        _ => {
            h.report(FRONT_MATTER_TYPE, span, Arg::None, Arg::None);
            return Ok(false);
        }
    }
    if let Some(subs) = y.child(root, "substitutions") {
        super::subs::add_substitutions(h, &y, subs, "");
    }
    let fields: Vec<usize> = y
        .children(root)
        .filter(|&c| !matches!(y.nodes[c].key.as_str(), "substitutions" | "html_meta"))
        .collect();
    if !fields.is_empty() {
        let list = h.b().begin_node(NodeKind::FieldList, span)?;
        for c in fields {
            front_field(h, &y, c, span)?;
        }
        h.b().end_node(list)?;
    }
    let Some(meta) = y.child(root, "html_meta") else { return Ok(false) };
    emit_meta(h, Some((&y, meta)), span)?;
    Ok(true)
}

/// Emits one field; bibliographic values are Markdown, others text.
fn front_field(h: &mut Host<'_, '_>, y: &Yaml, id: usize, span: SourceSpan) -> Result<(), Fail> {
    let key = y.nodes[id].key.as_str();
    let b = h.b();
    let field = b.begin_node(NodeKind::Field, span)?;
    let name = b.begin_node(NodeKind::FieldName, span)?;
    b.add_text_leaf(NodeKind::Text, span, key)?;
    b.end_node(name)?;
    let body = b.begin_node(NodeKind::FieldBody, span)?;
    let mut value = String::new();
    y.write_value(&mut value, id);
    if !value.is_empty() {
        let para = h.b().begin_node(NodeKind::Paragraph, span)?;
        if BIBLIOGRAPHIC_FIELDS.contains(&key) {
            if let Some(src) = h.ctx.add_source("<front matter>", &value) {
                let one = [Line { text: &value, offset: 0 }];
                inline_lines(h, &one, src)?;
            }
        } else {
            h.b().add_text_leaf(NodeKind::Text, span, &value)?;
        }
        h.b().end_node(para)?;
    }
    h.b().end_node(body)?;
    h.b().end_node(field)
}

/// Emits html_meta, the front matter's (`front`) merged over `Settings::html_meta`, as
/// the meta directive's node: Directive{Name "meta"} with a Custom attribute per tag.
fn emit_meta(
    h: &mut Host<'_, '_>,
    front: Option<(&Yaml, usize)>,
    span: SourceSpan,
) -> Result<(), Fail> {
    let settings = h.settings;
    if settings.html_meta.is_empty() && front.is_none() {
        return Ok(());
    }
    let id = h.b().begin_node(NodeKind::Directive, span)?;
    let name = h.b().add_text("meta")?;
    h.b().add_attribute(id, AttributeKey::Name, name)?;
    for m in &settings.html_meta {
        if front.is_some_and(|(y, mapping)| y.child(mapping, &m.key).is_some()) {
            continue;
        }
        meta_tag(h, id, &m.key, &m.content, span)?;
    }
    if let Some((y, mapping)) = front {
        for c in y.children(mapping) {
            let n = &y.nodes[c];
            let value = if n.kind == YamlKind::Scalar { n.text.as_str() } else { "" };
            let vs = value_span(h, n.offset);
            meta_tag(h, id, &n.key, value, vs)?;
        }
    }
    h.b().end_node(id)
}

/// Checks a meta key ("name", "name attr=value", "attr=value") and adds it.
fn meta_tag(
    h: &mut Host<'_, '_>,
    id: NodeId,
    key: &str,
    content: &str,
    span: SourceSpan,
) -> Result<(), Fail> {
    if content.is_empty() {
        h.report(HTML_META, span, arg_span(span), arg_text("it has no content"));
        return Ok(());
    }
    if key.split(' ').filter(|p| !p.is_empty()).skip(1).any(|p| !p.contains('=')) {
        h.report(HTML_META, span, arg_span(span), arg_text("a part after the first has no '='"));
        return Ok(());
    }
    h.b().add_custom_attribute(id, key, content)
}

/// The span of the rest of the main source's line from `offset`.
pub(crate) fn value_span(h: &Host<'_, '_>, offset: u32) -> SourceSpan {
    let text = h.ctx.source_of(h.ctx.source);
    let start = (offset as usize).min(text.len());
    let end = text[start..].find(['\n', '\r']).map_or(text.len(), |e| start + e);
    SourceSpan::new(h.ctx.source, start as u32, end as u32)
}

/// Drops the front matter of an included document, as MyST ignores it.
pub(crate) fn skip_front_matter<'l, 's>(lines: &'l [Line<'s>]) -> &'l [Line<'s>] {
    if lines.first().is_none_or(|l| trim_right(l.text) != "---") {
        return lines;
    }
    match lines[1..].iter().position(|l| matches!(trim_right(l.text), "---" | "...")) {
        Some(k) => &lines[k + 2..],
        None => lines,
    }
}
