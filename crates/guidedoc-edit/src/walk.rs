//! The walk from the reader's document tree to blocks: each node with a span in the
//! main source becomes a block, nodes without one are looked through, and the source
//! between blocks becomes source blocks (`gaps`).

use guidedoc::{Document, ListStyle, Node, NodeId, NodeKind, ROOT};
use guidedoc_sphinx::Config;

use crate::gaps;
use crate::lines::{
    indent_of, is_blank, line_at, line_end_trimmed, line_start, line_starts, next_line,
    previous_line, trim_end,
};
use crate::map::{BlockKind, DirectiveParts};
use crate::parts::{self, Ctx};
use crate::{Format, Span};

/// A block before it is numbered: the fields of `Block` and its children.
#[derive(Debug, Clone)]
pub(crate) struct Draft {
    pub kind: BlockKind,
    pub span: Span,
    pub content: Vec<Span>,
    pub level: u8,
    pub directive: Option<DirectiveParts>,
    pub children: Vec<Draft>,
}

impl Draft {
    /// A block without content or children.
    pub(crate) fn leaf(kind: BlockKind, span: Span) -> Draft {
        Draft { kind, span, content: Vec::new(), level: 0, directive: None, children: Vec::new() }
    }
}

/// Where children are read: the part of the text they lie in, how many Markdown quote
/// markers start each line, and the section depth (for reStructuredText title levels).
#[derive(Debug, Clone, Copy)]
struct Area {
    region: Span,
    quotes: usize,
    depth: u8,
}

/// The walk over one document.
pub(crate) struct Walker<'d, 't> {
    doc: Document<'d>,
    ctx: Ctx<'t>,
    max: usize,
    count: usize,
    /// More blocks than `max` were found.
    pub truncated: bool,
}

impl<'d, 't> Walker<'d, 't> {
    /// A walk over `doc`, read from `text`, listing at most `max` blocks.
    pub(crate) fn new(
        doc: Document<'d>,
        text: &'t str,
        format: Format,
        config: &'t Config,
        max: usize,
    ) -> Walker<'d, 't> {
        Walker { doc, ctx: Ctx { text, format, config }, max, count: 0, truncated: false }
    }

    /// The top-level blocks.
    pub(crate) fn document(&mut self) -> Vec<Draft> {
        let region = trim_end(self.ctx.text, Span::new(0, self.ctx.text.len()));
        self.children(ROOT, Area { region, quotes: 0, depth: 0 }, true)
    }

    /// The blocks of `parent`'s children inside `area`, in order, with source blocks
    /// for the text between them when `gaps` is set.
    fn children(&mut self, parent: NodeId, area: Area, fill: bool) -> Vec<Draft> {
        let mut out = Vec::new();
        for child in self.doc.children(parent) {
            self.node(child, area, &mut out);
        }
        out.sort_by_key(|d| d.span.start);
        let mut out = nest(out);
        if fill && !self.truncated {
            gaps::fill(&self.ctx, &mut out, area.region, area.quotes);
        }
        out
    }

    fn node(&mut self, id: NodeId, area: Area, out: &mut Vec<Draft>) {
        if self.count >= self.max {
            self.truncated = true;
            return;
        }
        let node = self.doc.node(id);
        if is_inline(node.kind) {
            return;
        }
        let span = self.span_of(node).filter(|s| area.region.contains(*s));
        let draft = span.and_then(|span| self.block(id, span, area));
        match draft {
            Some(draft) => {
                self.count += 1;
                out.push(draft);
            }
            None => {
                for child in self.doc.children(id) {
                    self.node(child, area, out);
                }
            }
        }
    }

    /// The node's span in the main source, without trailing white space.
    fn span_of(&self, node: &Node) -> Option<Span> {
        let (start, end) = (node.span.start as usize, node.span.end as usize);
        let text = self.ctx.text;
        let ok = node.span.source.0 == 0 && start < end && end <= text.len();
        let ok = ok && text.is_char_boundary(start) && text.is_char_boundary(end);
        ok.then(|| trim_end(text, Span::new(start, end))).filter(|s| !s.is_empty())
    }

    /// The block for node `id`; `None` for a node to look through.
    fn block(&mut self, id: NodeId, span: Span, area: Area) -> Option<Draft> {
        let kind = self.doc.kind(id);
        if let Some(parts) = parts::directive(&self.ctx, span) {
            return Some(self.directive(id, kind, span, parts, area));
        }
        let leaf = |k| Some(Draft::leaf(k, span));
        match kind {
            NodeKind::Section => Some(self.section(id, span, area)),
            NodeKind::Title | NodeKind::Heading => self.title(id, span, area.depth + 1),
            NodeKind::Paragraph => Some(self.lines(BlockKind::Paragraph, span, area.quotes)),
            NodeKind::Caption => Some(self.lines(BlockKind::Caption, span, area.quotes)),
            NodeKind::BlockQuote => Some(self.quote(id, span, area)),
            NodeKind::List => Some(self.list(id, span, area)),
            NodeKind::ListItem => Some(self.item(id, span, area)),
            NodeKind::DefinitionList => {
                Some(self.container(id, span, area, BlockKind::DefinitionList))
            }
            NodeKind::DefinitionItem => Some(self.definition(id, span, area)),
            NodeKind::FieldList => Some(self.container(id, span, area, BlockKind::FieldList)),
            NodeKind::Field => Some(self.field(id, span, area)),
            NodeKind::CodeBlock | NodeKind::Preformatted | NodeKind::DoctestBlock => {
                Some(self.literal(span))
            }
            NodeKind::Figure => Some(self.container(id, span, area, BlockKind::Figure)),
            NodeKind::Admonition => Some(self.container(id, span, area, BlockKind::Admonition)),
            NodeKind::Table => leaf(BlockKind::Table),
            NodeKind::Image => leaf(BlockKind::Image),
            NodeKind::ThematicBreak => leaf(BlockKind::Transition),
            NodeKind::Target => leaf(BlockKind::Target),
            NodeKind::SubstitutionDefinition => leaf(BlockKind::SubstitutionDefinition),
            NodeKind::Comment => Some(self.comment(span)),
            NodeKind::RawBlock if span.text(self.ctx.text).starts_with("<!--") => {
                Some(self.comment(span))
            }
            NodeKind::Legend
            | NodeKind::Term
            | NodeKind::Classifier
            | NodeKind::Definition
            | NodeKind::FieldName
            | NodeKind::FieldBody
            | NodeKind::DescContent
            | NodeKind::DescSignature => None,
            _ => leaf(BlockKind::Source),
        }
    }

    fn container(&mut self, id: NodeId, span: Span, area: Area, kind: BlockKind) -> Draft {
        let inner = Area { region: span, ..area };
        Draft { children: self.children(id, inner, true), ..Draft::leaf(kind, span) }
    }

    /// A block whose content is its own lines: a paragraph or a caption.
    fn lines(&self, kind: BlockKind, span: Span, quotes: usize) -> Draft {
        let content = text_lines(self.ctx.text, span, quotes);
        Draft { content, ..Draft::leaf(kind, span) }
    }

    fn section(&mut self, id: NodeId, span: Span, area: Area) -> Draft {
        let title_node = self
            .doc
            .children(id)
            .find(|&c| matches!(self.doc.kind(c), NodeKind::Heading | NodeKind::Title));
        let title = title_node.and_then(|t| {
            self.span_of(self.doc.node(t)).and_then(|s| self.title(t, s, area.depth + 1))
        });
        let Some(title) = title else { return Draft::leaf(BlockKind::Source, span) };
        let end = match self.ctx.format {
            Format::Rst => span.end,
            Format::Markdown => self.markdown_section_end(id, title.level, area.region.end),
        };
        let span = Span::new(title.span.start, end.max(title.span.end));
        let body =
            Area { region: Span::new(title.span.end, span.end), depth: area.depth + 1, ..area };
        let mut children = vec![title];
        let rest = self.children(id, body, true);
        children.extend(rest.into_iter().filter(|d| d.span.start >= body.region.start));
        Draft { children, ..Draft::leaf(BlockKind::Section, span) }
    }

    /// A Markdown section ends before the next heading of its level or a higher one;
    /// the reader's section node spans its heading only.
    fn markdown_section_end(&self, id: NodeId, level: u8, limit: usize) -> usize {
        let text = self.ctx.text;
        let after = self.doc.subtree_end(id).index();
        let next = self.doc.nodes()[after..]
            .iter()
            .filter(|n| n.kind == NodeKind::Section && n.span.source.0 == 0)
            .filter_map(|n| self.span_of(n))
            .find(|s| s.start < limit && markdown_level(text, *s).is_some_and(|l| l <= level));
        let end = next.map_or(limit, |s| s.start);
        trim_end(text, Span::new(0, end.min(limit))).end
    }

    fn title(&self, id: NodeId, span: Span, depth: u8) -> Option<Draft> {
        match self.ctx.format {
            Format::Rst => Some(rst_title(self.ctx.text, span, depth)),
            Format::Markdown => self.markdown_title(id, span),
        }
    }

    fn markdown_title(&self, id: NodeId, span: Span) -> Option<Draft> {
        let text = self.ctx.text;
        let level = markdown_level(text, span)?;
        let inline: Vec<Span> = self
            .doc
            .children(id)
            .filter_map(|c| {
                let n = self.doc.node(c);
                let s = Span::new(n.span.start as usize, n.span.end as usize);
                (n.span.source.0 == 0 && span.contains(s) && !s.is_empty()).then_some(s)
            })
            .collect();
        let content = match (inline.first(), inline.last()) {
            (Some(first), Some(last)) => text_lines(text, Span::new(first.start, last.end), 0),
            _ => Vec::new(),
        };
        Some(Draft { content, level, ..Draft::leaf(BlockKind::Title, span) })
    }

    fn quote(&mut self, id: NodeId, span: Span, area: Area) -> Draft {
        let quotes = area.quotes + usize::from(self.ctx.format == Format::Markdown);
        let inner = Area { region: span, quotes, ..area };
        Draft {
            children: self.children(id, inner, true),
            ..Draft::leaf(BlockKind::BlockQuote, span)
        }
    }

    fn list(&mut self, id: NodeId, span: Span, area: Area) -> Draft {
        let kind = match self.doc.list_info(id).style {
            ListStyle::Bullet => BlockKind::BulletList,
            _ => BlockKind::EnumeratedList,
        };
        let inner = Area { region: span, ..area };
        Draft { children: self.children(id, inner, false), ..Draft::leaf(kind, span) }
    }

    fn item(&mut self, id: NodeId, span: Span, area: Area) -> Draft {
        let text = self.ctx.text;
        let line_end = line_end_trimmed(text, span.start).min(span.end);
        let marker = span.start
            + text[span.start..line_end].find([' ', '\t']).unwrap_or(line_end - span.start);
        let body =
            marker + text[marker..line_end].len() - text[marker..line_end].trim_start().len();
        let inner = Area { region: Span::new(body, span.end), ..area };
        let children = self.children(id, inner, true);
        let content = match children.first() {
            Some(first) if first.kind == BlockKind::Paragraph && first.span.start < line_end => {
                first.content.clone()
            }
            _ => Vec::new(),
        };
        Draft { content, children, ..Draft::leaf(BlockKind::ListItem, span) }
    }

    fn definition(&mut self, id: NodeId, span: Span, area: Area) -> Draft {
        let mut content = Vec::new();
        let mut children = Vec::new();
        for child in self.doc.children(id) {
            let node = self.doc.node(child);
            let Some(child_span) = self.span_of(node) else { continue };
            match node.kind {
                NodeKind::Term if content.is_empty() => content.push(child_span),
                NodeKind::Definition => {
                    let inner = Area { region: child_span, ..area };
                    children = self.children(child, inner, true);
                }
                _ => {}
            }
        }
        Draft { content, children, ..Draft::leaf(BlockKind::DefinitionItem, span) }
    }

    fn field(&mut self, id: NodeId, span: Span, area: Area) -> Draft {
        let mut content = Vec::new();
        let mut children = Vec::new();
        for child in self.doc.children(id) {
            let node = self.doc.node(child);
            let Some(child_span) = self.span_of(node) else { continue };
            match node.kind {
                NodeKind::FieldName => content.push(child_span),
                NodeKind::FieldBody => {
                    let inner = Area { region: child_span, ..area };
                    children = self.children(child, inner, true);
                }
                _ => {}
            }
        }
        Draft { content, children, ..Draft::leaf(BlockKind::Field, span) }
    }

    /// A literal block not written as a directive: an RST `::` block, a Markdown fence
    /// or indented code, or a doctest block.
    fn literal(&self, span: Span) -> Draft {
        let text = self.ctx.text;
        let first = line_at(text, span.start).trim_start();
        let fenced = self.ctx.format == Format::Markdown
            && (first.starts_with("```") || first.starts_with("~~~"));
        if fenced {
            let body = Span::new(next_line(text, span.start), line_start(text, span.end));
            let content = if body.start < body.end {
                code_lines(text, trim_end(text, body))
            } else {
                Vec::new()
            };
            return Draft { content, ..Draft::leaf(BlockKind::LiteralBlock, span) };
        }
        let start = line_start(text, span.start);
        let content = code_lines(text, Span::new(start, span.end));
        let marker = previous_nonblank(text, start).filter(|&l| line_at(text, l).trim() == "::");
        let span = Span::new(marker.unwrap_or(start), span.end);
        Draft { content, ..Draft::leaf(BlockKind::LiteralBlock, span) }
    }

    fn comment(&self, span: Span) -> Draft {
        let text = self.ctx.text;
        let raw = span.text(text);
        let content =
            if let Some(inner) = raw.strip_prefix("<!--").and_then(|r| r.strip_suffix("-->")) {
                let lead = inner.len() - inner.trim_start().len();
                let from = span.start + 4 + lead;
                let kept = inner.trim();
                if kept.is_empty() || kept.contains('\n') {
                    Vec::new()
                } else {
                    vec![Span::new(from, from + kept.len())]
                }
            } else {
                comment_lines(text, span)
            };
        Draft { content, ..Draft::leaf(BlockKind::Comment, span) }
    }

    fn directive(
        &mut self,
        id: NodeId,
        kind: NodeKind,
        span: Span,
        parts: DirectiveParts,
        area: Area,
    ) -> Draft {
        let name = parts.name.text(self.ctx.text).to_ascii_lowercase();
        let block_kind = directive_kind(kind, &name);
        let mut draft = Draft::leaf(block_kind, span);
        match (block_kind, parts.content) {
            (BlockKind::Image, _) | (_, None) => {}
            (BlockKind::LiteralBlock | BlockKind::Table, Some(content)) => {
                draft.content = code_lines(self.ctx.text, content);
            }
            (_, Some(content)) => {
                let inner = Area { region: content, ..area };
                draft.children = self.children(id, inner, true);
                if block_kind == BlockKind::Directive
                    && draft.children.iter().all(|c| c.kind == BlockKind::Source)
                {
                    draft.children.clear();
                    draft.content = code_lines(self.ctx.text, content);
                }
            }
        }
        if block_kind == BlockKind::Table {
            draft.content.clear();
        }
        draft.directive = Some(parts);
        draft
    }
}

/// `blocks`, sorted by start, with each block that lies inside an earlier one moved
/// into it: a reader may move a node out of its section (Docutils moves a final
/// transition), while the source keeps it inside. A block that overlaps another
/// without lying inside it is dropped.
fn nest(blocks: Vec<Draft>) -> Vec<Draft> {
    let mut out: Vec<Draft> = Vec::with_capacity(blocks.len());
    for block in blocks {
        match out.last_mut() {
            Some(last) if block.span.start < last.span.end => {
                if last.span.contains(block.span) {
                    adopt(last, block);
                }
            }
            _ => out.push(block),
        }
    }
    out
}

/// Places `orphan` inside `parent`: in the child that contains it, in place of the
/// source block the gap filler made for its text, or among the children by position.
fn adopt(parent: &mut Draft, orphan: Draft) {
    let at = parent.children.iter().position(|c| c.span.contains(orphan.span));
    match at {
        Some(i) if parent.children[i].kind == BlockKind::Source => parent.children[i] = orphan,
        Some(i) if parent.children[i].span != orphan.span => {
            adopt(&mut parent.children[i], orphan)
        }
        Some(_) => {}
        None => {
            let i = parent.children.partition_point(|c| c.span.start < orphan.span.start);
            parent.children.insert(i, orphan);
        }
    }
}

/// The block kind of a directive whose node is `kind`.
fn directive_kind(kind: NodeKind, name: &str) -> BlockKind {
    match kind {
        NodeKind::Admonition => BlockKind::Admonition,
        NodeKind::Figure => BlockKind::Figure,
        NodeKind::Image => BlockKind::Image,
        NodeKind::Table => BlockKind::Table,
        NodeKind::CodeBlock if matches!(name, "code" | "code-block" | "sourcecode") => {
            BlockKind::LiteralBlock
        }
        _ => BlockKind::Directive,
    }
}

/// Tells whether a node kind is inline content (declared from `Text` on). An image is
/// inline in a paragraph and a block on its own; a block never holds an inline image
/// directly, so we map every image we meet.
fn is_inline(kind: NodeKind) -> bool {
    kind as usize >= NodeKind::Text as usize && kind != NodeKind::Image
}

/// An RST title block: the title text, with the overline and underline around it.
pub(crate) fn rst_title(text: &str, span: Span, level: u8) -> Draft {
    let line = line_start(text, span.start);
    let over = previous_line(text, line).filter(|&l| is_adornment(line_at(text, l)));
    let under_at = next_line(text, line);
    let under =
        (under_at < text.len() && under_at > line && is_adornment(line_at(text, under_at)))
            .then_some(under_at);
    let start = over.unwrap_or(line);
    let end = under.map_or(span.end, |u| line_end_trimmed(text, u));
    Draft { content: vec![span], level, ..Draft::leaf(BlockKind::Title, Span::new(start, end)) }
}

/// Tells whether `line` is a section adornment: one punctuation character repeated.
pub(crate) fn is_adornment(line: &str) -> bool {
    let line = line.trim_end();
    let mut chars = line.chars();
    let Some(first) = chars.next() else { return false };
    line.len() >= 2 && first.is_ascii_punctuation() && chars.all(|c| c == first)
}

/// The level of the Markdown heading at `span`: the number of `#`, or 1 and 2 for an
/// `=` and a `-` underline.
pub(crate) fn markdown_level(text: &str, span: Span) -> Option<u8> {
    let first = line_at(text, span.start).trim_start();
    let hashes = first.len() - first.trim_start_matches('#').len();
    if (1..=6).contains(&hashes) {
        return Some(hashes as u8);
    }
    let last = line_at(text, span.end.saturating_sub(1)).trim();
    match last.chars().next() {
        Some('=') => Some(1),
        Some('-') => Some(2),
        _ => None,
    }
}

/// The lines of an inline text block: the first from `span.start`, the others without
/// their indentation and up to `quotes` Markdown quote markers.
pub(crate) fn text_lines(text: &str, span: Span, quotes: usize) -> Vec<Span> {
    let mut out = Vec::new();
    for (i, start) in line_starts(text, span).into_iter().enumerate() {
        let end = line_end_trimmed(text, start).min(span.end);
        let from =
            if i == 0 { span.start.max(start) } else { strip_prefix(text, start, end, quotes) };
        out.push(Span::new(from.min(end), end));
    }
    out
}

/// The text lines of an RST comment (`..` and indented lines) or a MyST comment (`%`
/// starting every line), without their markers and the spaces after them; empty lines
/// are left out.
fn comment_lines(text: &str, span: Span) -> Vec<Span> {
    let raw = span.text(text);
    let percent = raw.starts_with('%');
    let marker = if raw.starts_with("..") { 2 } else { usize::from(percent) };
    let mut out = Vec::new();
    for (i, start) in line_starts(text, span).into_iter().enumerate() {
        let end = line_end_trimmed(text, start).min(span.end);
        let mut from = if i == 0 { span.start + marker } else { skip_ws(text, start, end) };
        if i > 0 && percent && text[from.min(end)..end].starts_with('%') {
            from += 1;
        }
        let from = skip_ws(text, from.min(end), end);
        if from < end {
            out.push(Span::new(from, end));
        }
    }
    out
}

/// The lines of code: each without the indentation all non-blank lines share; blank
/// lines as empty spans.
pub(crate) fn code_lines(text: &str, span: Span) -> Vec<Span> {
    let starts = line_starts(text, Span::new(line_start(text, span.start), span.end));
    let indent = starts
        .iter()
        .map(|&s| line_at(text, s))
        .filter(|l| !is_blank(l))
        .map(indent_of)
        .min()
        .unwrap_or(0);
    starts
        .into_iter()
        .map(|s| {
            let end = line_end_trimmed(text, s).min(span.end);
            if is_blank(&text[s..end]) { Span::new(end, end) } else { Span::new(s + indent, end) }
        })
        .collect()
}

fn strip_prefix(text: &str, mut at: usize, end: usize, quotes: usize) -> usize {
    let mut quotes = quotes;
    loop {
        at = skip_ws(text, at, end);
        if quotes > 0 && text[at..end].starts_with('>') {
            at += 1;
            quotes -= 1;
        } else {
            return at;
        }
    }
}

fn skip_ws(text: &str, at: usize, end: usize) -> usize {
    let at = at.min(end);
    end - text[at..end].trim_start_matches([' ', '\t']).len()
}

fn previous_nonblank(text: &str, at: usize) -> Option<usize> {
    let mut line = previous_line(text, at)?;
    while is_blank(line_at(text, line)) {
        line = previous_line(text, line)?;
    }
    Some(line)
}
