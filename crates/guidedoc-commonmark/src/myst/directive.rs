//! Directives as myst-parser 0.16.1 reads them: a fence whose info string starts with
//! `{name}` (``` or ~~~, and ::: with "colon_fence"). The rest of the info line is the
//! argument, or, for a directive that takes none, the first line of content. Options
//! open the content, as a YAML block between "---" lines or as ":key: value" lines,
//! which are YAML without their leading colons. A blank line after them is dropped.

use guidedoc::{AttributeKey, Fail, NodeKind, SourceSpan, arg_span, arg_static};

use super::blocks::field_marker;
use super::front::value_span;
use super::yaml::{ScalarType, Yaml, YamlKind, YamlNode, bool_true, indent_of, scalar_type};
use super::{sections_here, trim_right, trim_space};
use crate::blocks::{Bid, BlockKind, Skeleton, parse_skeleton};
use crate::emit::open_block;
use crate::extension::{Directive, DirectiveOption, DirectiveResult, DirectiveShape, Line};
use crate::host::Host;
use crate::inline::{leaf_is_image, parse_leaf};
use crate::messages::{
    DIRECTIVE_NO_HANDLER, DIRECTIVE_OPTION_VALUE, DIRECTIVE_OPTIONS, DIRECTIVE_UNKNOWN, FIGURE_MD,
};
use crate::settings::Rule;

/// Tells whether a fenced code block is a directive: its info string starts `{name}`.
pub(crate) fn is_directive_fence(h: &Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> bool {
    let b = &sk.blocks[id];
    let info = sk.arena_str(b.info_start, b.info_end);
    h.on(Rule::MystDirective) && info.len() > 2 && info.starts_with('{') && info.contains('}')
}

/// Runs the directive a fenced block holds.
pub(crate) fn run_fence_directive(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    top: bool,
) -> Result<(), Fail> {
    let blk = sk.blocks[id];
    let info = sk.arena_str(blk.info_start, blk.info_end);
    let close = info.find('}').unwrap_or(info.len());
    let name = &info[1..close];
    let at = sk.source_offset(id, blk.info_start);
    let name_span = sk.span(at + 1, at + close);
    let rest = &info[close + 1..];
    let lead = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let mut first =
        Line { text: trim_right(&rest[lead..]), offset: (at + close + 1 + lead) as u32 };
    let a = first.offset as usize;
    if let Some(src) = sk.src.get(a..a + first.text.len()).filter(|s| *s == first.text) {
        first.text = src;
    }
    let content = sk.block_lines(id, 1);
    let call = Call { name, name_span, span: sk.block_span(id), top };
    run_directive(h, sk, &call, first, &content, Vec::new())
}

/// A directive's name and where it stands.
pub(crate) struct Call<'n> {
    pub name: &'n str,
    pub name_span: SourceSpan,
    pub span: SourceSpan,
    pub top: bool,
}

/// Checks a directive's name, reads its options, and runs its handler; `extra` options
/// follow the written ones.
pub(crate) fn run_directive<'d>(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    call: &Call<'d>,
    first: Line<'d>,
    content: &[Line<'d>],
    extra: Vec<DirectiveOption<'d>>,
) -> Result<(), Fail> {
    if call.name.eq_ignore_ascii_case("figure-md") {
        return figure_md(h, sk, call, first, content);
    }
    let Some(ext) = h.ext() else {
        h.report_span(DIRECTIVE_NO_HANDLER, call.name_span);
        return Ok(());
    };
    let Some(shape) = ext.shape(h, call.name) else {
        h.report_span(DIRECTIVE_UNKNOWN, call.name_span);
        return Ok(());
    };
    let block = option_block(content, shape);
    let yaml = match parse_options(h, call.name_span, &block.yaml) {
        Some(yaml) => yaml,
        None => return Ok(()),
    };
    let Some(mut options) = options_of(h, sk, call.name_span, &yaml) else { return Ok(()) };
    options.extend(extra);
    let mut body = block.body;
    let mut argument = Vec::new();
    if !first.text.is_empty() {
        if shape.argument {
            argument.push(first);
        } else {
            body.insert(0, first);
        }
    }
    let d = Directive {
        name: call.name,
        name_span: call.name_span,
        span: call.span,
        source: sk.source,
        argument,
        options,
        content: body,
        sections: call.top && sections_here(h, sk),
    };
    if ext.directive(h, &d)? == DirectiveResult::Unknown {
        h.report_span(DIRECTIVE_UNKNOWN, call.name_span);
    }
    Ok(())
}

/// The option lines (YAML) that open a directive's content, and the content after them.
struct OptionBlock<'d> {
    yaml: Vec<Line<'d>>,
    body: Vec<Line<'d>>,
}

fn is_dashes(t: &str) -> bool {
    let s = trim_right(t);
    s.len() >= 3 && s.bytes().all(|c| c == b'-')
}

fn option_block<'d>(content: &[Line<'d>], shape: DirectiveShape) -> OptionBlock<'d> {
    let mut yaml = Vec::new();
    let mut body = content;
    if !shape.options {
        // Without options, option-like lines stay content.
    } else if content.first().is_some_and(|l| is_dashes(l.text)) {
        let k = 1 + content[1..].iter().take_while(|l| !is_dashes(l.text)).count();
        yaml = dedent_lines(&content[1..k]);
        body = &content[(k + 1).min(content.len())..];
    } else if content.first().is_some_and(|l| field_marker(trim_space(l.text)) > 0) {
        // Option lines are ":name: value"; ":::" and other text is content.
        let k = content.iter().take_while(|l| field_marker(trim_space(l.text)) > 0).count();
        yaml = content[..k]
            .iter()
            .map(|l| {
                let lead = indent_of(l.text) + 1;
                Line { text: &l.text[lead..], offset: l.offset + lead as u32 }
            })
            .collect();
        body = &content[k..];
    }
    if body.first().is_some_and(|l| trim_space(l.text).is_empty()) {
        body = &body[1..];
    }
    OptionBlock { yaml, body: body.to_vec() }
}

/// Removes the indentation the lines share.
fn dedent_lines<'d>(lines: &[Line<'d>]) -> Vec<Line<'d>> {
    let least = lines
        .iter()
        .filter(|l| !trim_space(l.text).is_empty())
        .map(|l| indent_of(l.text))
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| {
            let n = least.min(indent_of(l.text));
            Line { text: &l.text[n..], offset: l.offset + n as u32 }
        })
        .collect()
}

/// Parses option YAML, reporting what is not a mapping; `None` after a report.
fn parse_options(h: &mut Host<'_, '_>, name_span: SourceSpan, lines: &[Line<'_>]) -> Option<Yaml> {
    if lines.is_empty() {
        return Some(Yaml::default());
    }
    match super::yaml::yaml_parse(lines) {
        Err(e) => {
            h.report(DIRECTIVE_OPTIONS, name_span, arg_span(name_span), arg_static(e.why));
            None
        }
        Ok((y, root)) => match y.nodes[root].kind {
            YamlKind::Mapping | YamlKind::Null => Some(y),
            _ => {
                let why = "expected a mapping of option names to values";
                h.report(DIRECTIVE_OPTIONS, name_span, arg_span(name_span), arg_static(why));
                None
            }
        },
    }
}

/// The options of parsed option YAML. A value must be a scalar; an empty, null, or
/// true value is a flag.
fn options_of<'y>(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    name_span: SourceSpan,
    y: &'y Yaml,
) -> Option<Vec<DirectiveOption<'y>>> {
    let mut out = Vec::new();
    let Some(root) = (!y.nodes.is_empty()).then_some(0) else { return Some(out) };
    for c in y.children(root) {
        let node: &YamlNode = &y.nodes[c];
        let span = if sk.source == h.ctx.source { value_span(h, node.offset) } else { name_span };
        if matches!(node.kind, YamlKind::Mapping | YamlKind::Sequence) {
            h.report(DIRECTIVE_OPTION_VALUE, span, arg_span(name_span), arg_span(span));
            return None;
        }
        let flag = match scalar_type(node) {
            ScalarType::Null => true,
            ScalarType::Bool => bool_true(&node.text),
            _ => false,
        };
        let value = if flag || node.kind != YamlKind::Scalar { "" } else { node.text.as_str() };
        out.push(DirectiveOption { name: &node.key, value: text_lines(value, node.offset), span });
    }
    Some(out)
}

/// Splits a value at line breaks into lines that start at `offset`; trailing empty
/// lines are dropped.
pub(crate) fn text_lines(text: &str, offset: u32) -> Vec<Line<'_>> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for part in text.split('\n') {
        lines.push(Line { text: part, offset: offset + start as u32 });
        start += part.len() + 1;
    }
    while lines.last().is_some_and(|l| l.text.is_empty()) {
        lines.pop();
    }
    lines
}

/// MyST's figure-md: an image (Markdown or HTML) and a caption paragraph make a
/// figure; the argument names it.
fn figure_md<'d>(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    call: &Call<'d>,
    first: Line<'d>,
    content: &[Line<'d>],
) -> Result<(), Fail> {
    let block = option_block(content, DirectiveShape::default());
    let Some(yaml) = parse_options(h, call.name_span, &block.yaml) else { return Ok(()) };
    let Some(options) = options_of(h, sk, call.name_span, &yaml) else { return Ok(()) };
    if block.body.is_empty() {
        h.report_span(FIGURE_MD, call.span);
        return Ok(());
    }
    let q = parse_skeleton(h, &block.body, sk.source, sk.src, false, false)?;
    let image = q.blocks[0].first;
    let caption = image.and_then(|i| q.blocks[i].next);
    let kind = image.map_or(BlockKind::Document, |i| q.blocks[i].kind);
    let image_ok = match image {
        Some(i) if kind == BlockKind::Paragraph => leaf_is_image(h, &q, i)?,
        Some(_) => kind == BlockKind::HtmlBlock,
        None => false,
    };
    let caption_ok = caption
        .is_none_or(|c| q.blocks[c].kind == BlockKind::Paragraph && q.blocks[c].next.is_none());
    let (Some(image), true, true) = (image, image_ok, caption_ok) else {
        h.report_span(FIGURE_MD, call.span);
        return Ok(());
    };
    let fig = open_block(h, NodeKind::Figure, call.span)?;
    for o in &options {
        let value = o.value.first().map_or("", |l| l.text);
        let key = match o.name {
            "class" => AttributeKey::Class,
            "width" => AttributeKey::Width,
            "align" => AttributeKey::Align,
            "name" => {
                let name = h.normalized_name(value)?;
                h.b().add_attribute(fig, AttributeKey::Name, name)?;
                continue;
            }
            _ => continue,
        };
        h.b().add_attribute_string(fig, key, value)?;
    }
    if !first.text.is_empty() {
        let name = h.normalized_name(first.text)?;
        h.b().add_attribute(fig, AttributeKey::Name, name)?;
    }
    if kind == BlockKind::HtmlBlock {
        super::html::html_images(h, &q, image)?;
    } else {
        parse_leaf(h, &q, image)?;
    }
    if let Some(c) = caption {
        let node = h.b().begin_node(NodeKind::Caption, q.block_span(c))?;
        parse_leaf(h, &q, c)?;
        h.b().end_node(node)?;
    }
    h.b().end_node(fig)
}
