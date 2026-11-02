//! The source map: blocks with their spans, read with Guidedog's readers.

use guidedoc::{Workspace, read_document, read_extension};

use crate::parts::Ctx;
use crate::walk::{Draft, Walker};
use crate::{Format, MapOptions, Span};

/// What a block is. The names ([`BlockKind::name`]) are stable: they appear in JSON
/// and capability reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// A section: its title, then its body; the span runs to the next section of the
    /// same or a higher level.
    Section,
    /// A section title or Markdown heading; the content is the title text, the span
    /// includes the adornment or the `#` markers. `level` counts from 1.
    Title,
    /// A paragraph of inline text.
    Paragraph,
    /// A bullet list; its children are list items.
    BulletList,
    /// An enumerated (ordered) list; its children are list items.
    EnumeratedList,
    /// A definition list; its children are definition items.
    DefinitionList,
    /// One item of a list. The content is the text of its first paragraph; the children
    /// are its blocks, that paragraph first.
    ListItem,
    /// A term and its definition. The content is the term; the children are the
    /// definition's blocks.
    DefinitionItem,
    /// A field list; its children are fields.
    FieldList,
    /// One field. The content is the field name; the children are the body's blocks.
    Field,
    /// A literal or code block. The content is the code, without indentation or fences.
    LiteralBlock,
    /// A directive of another kind, such as `toctree` or `math`.
    Directive,
    /// An admonition, such as `note` or `warning`; its children are its blocks.
    Admonition,
    /// A table, written as a grid, a simple table, a pipe table, or a table directive.
    Table,
    /// A figure: an image with a caption and legend.
    Figure,
    /// A stand-alone image.
    Image,
    /// A figure's caption; the content is its text.
    Caption,
    /// A block quote; its children are its blocks.
    BlockQuote,
    /// A comment; the content is its text.
    Comment,
    /// A hyperlink target, such as `.. _name:` or `(name)=`.
    Target,
    /// A substitution definition, such as `.. |name| replace:: text`.
    SubstitutionDefinition,
    /// A transition (thematic break).
    Transition,
    /// Any other construct, edited as source only.
    Source,
}

impl BlockKind {
    /// Every kind, in declaration order.
    pub const ALL: [BlockKind; 23] = [
        BlockKind::Section,
        BlockKind::Title,
        BlockKind::Paragraph,
        BlockKind::BulletList,
        BlockKind::EnumeratedList,
        BlockKind::DefinitionList,
        BlockKind::ListItem,
        BlockKind::DefinitionItem,
        BlockKind::FieldList,
        BlockKind::Field,
        BlockKind::LiteralBlock,
        BlockKind::Directive,
        BlockKind::Admonition,
        BlockKind::Table,
        BlockKind::Figure,
        BlockKind::Image,
        BlockKind::Caption,
        BlockKind::BlockQuote,
        BlockKind::Comment,
        BlockKind::Target,
        BlockKind::SubstitutionDefinition,
        BlockKind::Transition,
        BlockKind::Source,
    ];

    /// The kind's stable name, such as `bullet-list`.
    pub fn name(self) -> &'static str {
        match self {
            BlockKind::Section => "section",
            BlockKind::Title => "title",
            BlockKind::Paragraph => "paragraph",
            BlockKind::BulletList => "bullet-list",
            BlockKind::EnumeratedList => "enumerated-list",
            BlockKind::DefinitionList => "definition-list",
            BlockKind::ListItem => "list-item",
            BlockKind::DefinitionItem => "definition-item",
            BlockKind::FieldList => "field-list",
            BlockKind::Field => "field",
            BlockKind::LiteralBlock => "literal-block",
            BlockKind::Directive => "directive",
            BlockKind::Admonition => "admonition",
            BlockKind::Table => "table",
            BlockKind::Figure => "figure",
            BlockKind::Image => "image",
            BlockKind::Caption => "caption",
            BlockKind::BlockQuote => "block-quote",
            BlockKind::Comment => "comment",
            BlockKind::Target => "target",
            BlockKind::SubstitutionDefinition => "substitution-definition",
            BlockKind::Transition => "transition",
            BlockKind::Source => "source",
        }
    }

    /// The kind called `name`, as [`BlockKind::name`] writes it.
    pub fn from_name(name: &str) -> Option<BlockKind> {
        BlockKind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// The identity of a block within one map: its index in [`SourceMap::blocks`], which
/// lists blocks in document order (a parent before its children).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct BlockId(pub u32);

impl BlockId {
    /// The index into [`SourceMap::blocks`].
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// One option of a directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OptionParts {
    /// The option's lines, value included, without the final line break.
    pub span: Span,
    /// The option's name, without the colons.
    pub name: Span,
    /// The value; `None` for a flag.
    pub value: Option<Span>,
}

/// The parts of a block written as a directive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectiveParts {
    /// The directive's name as written, such as `code-block`.
    pub name: Span,
    /// The arguments, possibly over several lines.
    pub arguments: Option<Span>,
    /// The options, in source order.
    pub options: Vec<OptionParts>,
    /// The options are a MyST YAML block (`---` lines) rather than `:name:` fields.
    pub yaml_options: bool,
    /// The content, from its first to its last non-blank line.
    pub content: Option<Span>,
}

/// One block of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// What it is.
    pub kind: BlockKind,
    /// The whole block, markup included, from its first byte to its last non-blank one.
    pub span: Span,
    /// The text an author edits, without markup: one span per source line, so the
    /// editable text is these spans joined by line breaks. Empty when the block has no
    /// text of its own (its children have it).
    pub content: Vec<Span>,
    /// A title's level from 1; 0 for other blocks.
    pub level: u8,
    /// The parts of a block written as a directive.
    pub directive: Option<DirectiveParts>,
    /// The block that contains it; `None` at the top level.
    pub parent: Option<BlockId>,
    /// The blocks it contains, in order.
    pub children: Vec<BlockId>,
}

/// The blocks of one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
    /// The format it was read as.
    pub format: Format,
    /// Every block, a parent before its children, siblings in document order.
    pub blocks: Vec<Block>,
    /// The top-level blocks.
    pub roots: Vec<BlockId>,
    /// The document has more blocks than the map lists.
    pub truncated: bool,
    /// The length of the text it was made from, in bytes.
    pub len: usize,
}

impl SourceMap {
    /// The block `id`; `None` when the map has no such block.
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.get(id.index())
    }

    /// The first block of kind `kind` in document order.
    pub fn find(&self, kind: BlockKind) -> Option<BlockId> {
        let index = self.blocks.iter().position(|b| b.kind == kind)?;
        Some(BlockId(index as u32))
    }

    /// The innermost block whose span holds byte `offset`: what a click at that place
    /// selects.
    pub fn block_at(&self, offset: usize) -> Option<BlockId> {
        let mut found = None;
        let mut level = &self.roots;
        while let Some(&id) = level.iter().find(|&&id| {
            let span = self.blocks[id.index()].span;
            span.start <= offset && offset < span.end.max(span.start + 1)
        }) {
            found = Some(id);
            level = &self.blocks[id.index()].children;
        }
        found
    }

    /// The siblings of `id`: its parent's children, or the top-level blocks.
    pub fn siblings(&self, id: BlockId) -> &[BlockId] {
        match self.block(id).and_then(|b| b.parent) {
            Some(parent) => &self.blocks[parent.index()].children,
            None => &self.roots,
        }
    }

    /// The editable text of block `id` in `text`: its content lines joined by line
    /// breaks; empty for a block without content or an unknown id.
    pub fn content_text(&self, id: BlockId, text: &str) -> String {
        let Some(block) = self.block(id) else { return String::new() };
        let lines: Vec<&str> = block.content.iter().map(|s| s.text(text)).collect();
        lines.join("\n")
    }
}

/// Reads `text` as `options` say and returns its block map. A document the reader
/// cannot read at all (a resource limit, invalid configuration) maps to one source
/// block covering the text; the reader's warnings and errors do not stop the map.
pub fn load_source_map(text: &str, options: &MapOptions<'_>) -> SourceMap {
    let mut ws = Workspace::new();
    let default_config = guidedoc_sphinx::Config::default();
    let config = options.sphinx.unwrap_or(&default_config);
    let drafts = read_drafts(&mut ws, text, options, config);
    let (drafts, truncated) = drafts.unwrap_or_else(|| {
        let whole = crate::lines::trim_end(text, Span::new(0, text.len()));
        let ctx = Ctx { text, format: options.format, config };
        (crate::gaps::chunks(&ctx, whole, 0), false)
    });
    let mut map = SourceMap {
        format: options.format,
        blocks: Vec::new(),
        roots: Vec::new(),
        truncated,
        len: text.len(),
    };
    for draft in drafts {
        let id = flatten(&mut map, draft, None);
        map.roots.push(id);
    }
    map
}

fn read_drafts(
    ws: &mut Workspace,
    text: &str,
    options: &MapOptions<'_>,
    config: &guidedoc_sphinx::Config,
) -> Option<(Vec<Draft>, bool)> {
    let myst =
        guidedoc_sphinx::myst_settings(config.clone(), options.myst.cloned().unwrap_or_default());
    let (reader, extension) = match options.format {
        Format::Rst => (guidedoc_sphinx::READER, read_extension(config)),
        Format::Markdown => (guidedoc_commonmark::MYST_READER, read_extension(&myst)),
    };
    let mut request = guidedoc::default_request("document", text, reader.id, "");
    request.read.promote_title = false;
    request.read.extension = Some(extension);
    request.policy.keep_documents_with_errors = true;
    request.policy.includes = false;
    let doc = read_document(ws, &reader, &request).ok()?;
    let mut walker = Walker::new(doc, text, options.format, config, options.max_blocks);
    let drafts = walker.document();
    Some((drafts, walker.truncated))
}

fn flatten(map: &mut SourceMap, draft: Draft, parent: Option<BlockId>) -> BlockId {
    let id = BlockId(map.blocks.len() as u32);
    map.blocks.push(Block {
        kind: draft.kind,
        span: draft.span,
        content: draft.content,
        level: draft.level,
        directive: draft.directive,
        parent,
        children: Vec::new(),
    });
    let children: Vec<BlockId> =
        draft.children.into_iter().map(|c| flatten(map, c, Some(id))).collect();
    map.blocks[id.index()].children = children;
    id
}
