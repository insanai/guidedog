//! The document model's plain data: ids, spans, statuses, node kinds and payload rows.
//! AST.md is the normative mapping from source constructs to these types.

use std::ops::{BitOr, BitOrAssign, Index, IndexMut};

/// A node's index in its snapshot's preorder node table. `ROOT` is the Document node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NodeId(pub u32);

impl NodeId {
    /// Returns the id as a `usize` index into the node table.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Returns the id of node `index`; indices past `u32::MAX` give `INVALID_NODE`.
    #[inline]
    pub fn from_index(index: usize) -> NodeId {
        NodeId(u32::try_from(index).unwrap_or(u32::MAX))
    }

    /// Returns the id `n` nodes after this one (saturating).
    #[inline]
    pub const fn plus(self, n: u32) -> NodeId {
        NodeId(self.0.saturating_add(n))
    }
}

impl std::ops::Add<u32> for NodeId {
    type Output = NodeId;
    fn add(self, n: u32) -> NodeId {
        self.plus(n)
    }
}

impl std::ops::Sub<u32> for NodeId {
    type Output = NodeId;
    fn sub(self, n: u32) -> NodeId {
        NodeId(self.0.saturating_sub(n))
    }
}

/// An index into a workspace's source table; 0 is the main source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SourceId(pub u16);

impl SourceId {
    /// The main source of a conversion.
    pub const MAIN: SourceId = SourceId(0);

    /// Returns the id as a `usize` index into the source table.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// "No row": a node's payload when it has none.
pub const NONE: u32 = u32::MAX;
/// "No node": returned where a node may be absent, as Odin's `INVALID_NODE`.
pub const INVALID_NODE: NodeId = NodeId(u32::MAX);
/// The root Document node of every snapshot.
pub const ROOT: NodeId = NodeId(0);

/// A byte range in a snapshot's text pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Text {
    /// First byte in the pool.
    pub start: u32,
    /// Length in bytes.
    pub len: u32,
}

impl Text {
    /// Returns the range `start..start + len`, which `Document::text` slices.
    #[inline]
    pub const fn range(self) -> std::ops::Range<usize> {
        self.start as usize..self.start as usize + self.len as usize
    }

    /// Tells whether the text is empty.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// A half-open byte range in one source of the workspace's source table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceSpan {
    /// The source the bytes belong to.
    pub source: SourceId,
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
}

impl SourceSpan {
    /// Makes a span of `source` from `start` to `end`.
    #[inline]
    pub const fn new(source: SourceId, start: u32, end: u32) -> SourceSpan {
        SourceSpan { source, start, end }
    }

    /// Tells whether the span is the zero span (no source position).
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.source.0 == 0 && self.start == 0 && self.end == 0
    }
}

/// One line of a source as readers hand it to each other: its text without the line
/// break, and the source offset of its first byte. Borrowed from the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineView<'s> {
    /// The line's text, without its break.
    pub text: &'s str,
    /// Offset of `text` in its source.
    pub offset: u32,
}

/// Returns where `inner` starts in `outer` when `inner` is a slice of `outer`'s bytes,
/// and `None` otherwise (an empty `inner` included). It compares addresses only.
pub fn offset_in(outer: &str, inner: &str) -> Option<usize> {
    if inner.is_empty() || outer.is_empty() {
        return None;
    }
    let a = outer.as_ptr() as usize;
    let x = inner.as_ptr() as usize;
    if x < a || x - a > outer.len() {
        return None;
    }
    let at = x - a;
    if inner.len() > outer.len() - at {
        return None;
    }
    Some(at)
}

/// A node's attributes: rows `first..first + count` of the attribute table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttributeRange {
    /// First attribute row.
    pub first: u32,
    /// Number of rows.
    pub count: u32,
}

impl AttributeRange {
    /// Returns the rows as a range of indices.
    #[inline]
    pub const fn range(self) -> std::ops::Range<usize> {
        self.first as usize..self.first as usize + self.count as usize
    }
}

/// The outcome class of a stage. `Ok` is kept so builders can report their state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Status {
    /// Success.
    #[default]
    Ok,
    /// The input (or a document, request or registry) is invalid.
    InvalidInput,
    /// The input asks for something no adapter supports.
    Unsupported,
    /// The policy refused something.
    Policy,
    /// Memory ran out; `CapacityNeed` says where.
    Capacity,
    /// A configured limit was reached.
    Limit,
    /// The caller cancelled the conversion.
    Cancelled,
    /// A bug in an adapter or the core.
    Internal,
}

/// A storage region named by a capacity failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Region {
    /// No region.
    #[default]
    None,
    /// The node table.
    Nodes,
    /// The text pool.
    Text,
    /// The attribute table.
    Attributes,
    /// The list payload table.
    Lists,
    /// The link payload table.
    Links,
    /// The cell payload table.
    Cells,
    /// The source table.
    Sources,
    /// Working memory of an adapter.
    Scratch,
    /// The report store.
    Reports,
    /// The rendered output.
    Output,
    /// The source map.
    SourceMap,
}

impl Region {
    /// Returns the static name diagnostics write ("nodes").
    pub const fn name(self) -> &'static str {
        match self {
            Region::None => "none",
            Region::Nodes => "nodes",
            Region::Text => "text",
            Region::Attributes => "attributes",
            Region::Lists => "lists",
            Region::Links => "links",
            Region::Cells => "cells",
            Region::Sources => "sources",
            Region::Scratch => "scratch",
            Region::Reports => "reports",
            Region::Output => "output",
            Region::SourceMap => "source-map",
        }
    }
}

/// What a `Capacity` failure needs: the region and the bytes or rows it lacked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CapacityNeed {
    /// The region that ran out.
    pub region: Region,
    /// The least size that is needed (or the shortfall, when not exact).
    pub minimum: u64,
    /// Whether `minimum` is the exact size needed.
    pub exact: bool,
}

/// Every kind of node. AST.md documents each kind's children and fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
#[allow(missing_docs)] // AST.md documents every kind.
pub enum NodeKind {
    /// The root.
    #[default]
    Document,
    // Block structure.
    Title,
    Subtitle,
    Section,
    Heading,
    Paragraph,
    BlockQuote,
    Attribution,
    List,
    ListItem,
    DefinitionList,
    DefinitionItem,
    Term,
    Classifier,
    Definition,
    FieldList,
    Field,
    FieldName,
    FieldBody,
    OptionList,
    OptionItem,
    OptionGroup,
    Option,
    OptionArgument,
    OptionDescription,
    LineBlock,
    Line,
    CodeBlock,
    Preformatted,
    DoctestBlock,
    MathBlock,
    RawBlock,
    ThematicBreak,
    Table,
    TableHead,
    TableBody,
    Row,
    Cell,
    Figure,
    Caption,
    Legend,
    Admonition,
    Topic,
    Sidebar,
    Rubric,
    Container,
    Compound,
    Footnote,
    Citation,
    Target,
    Comment,
    SubstitutionDefinition,
    Contents,
    Directive,
    // Sphinx layer blocks.
    Toctree,
    ToctreeEntry,
    Only,
    Glossary,
    VersionModified,
    Desc,
    DescSignature,
    DescContent,
    Hlist,
    ProductionList,
    Production,
    Index,
    // Inline content.
    Text,
    Emphasis,
    Strong,
    Code,
    Link,
    Reference,
    Image,
    FootnoteReference,
    CitationReference,
    SubstitutionReference,
    InlineTarget,
    Math,
    RawInline,
    SoftBreak,
    HardBreak,
    Subscript,
    Superscript,
    TitleReference,
    Abbreviation,
    Role,
    // Sphinx layer inlines.
    Xref,
    DescName,
    DescAddname,
    DescAnnotation,
    DescParameterlist,
    DescParameter,
    DescOptional,
    DescReturns,
}

impl NodeKind {
    /// Every kind, in declaration order (`ALL[k as usize] == k`).
    pub const ALL: [NodeKind; 94] = [
        NodeKind::Document,
        NodeKind::Title,
        NodeKind::Subtitle,
        NodeKind::Section,
        NodeKind::Heading,
        NodeKind::Paragraph,
        NodeKind::BlockQuote,
        NodeKind::Attribution,
        NodeKind::List,
        NodeKind::ListItem,
        NodeKind::DefinitionList,
        NodeKind::DefinitionItem,
        NodeKind::Term,
        NodeKind::Classifier,
        NodeKind::Definition,
        NodeKind::FieldList,
        NodeKind::Field,
        NodeKind::FieldName,
        NodeKind::FieldBody,
        NodeKind::OptionList,
        NodeKind::OptionItem,
        NodeKind::OptionGroup,
        NodeKind::Option,
        NodeKind::OptionArgument,
        NodeKind::OptionDescription,
        NodeKind::LineBlock,
        NodeKind::Line,
        NodeKind::CodeBlock,
        NodeKind::Preformatted,
        NodeKind::DoctestBlock,
        NodeKind::MathBlock,
        NodeKind::RawBlock,
        NodeKind::ThematicBreak,
        NodeKind::Table,
        NodeKind::TableHead,
        NodeKind::TableBody,
        NodeKind::Row,
        NodeKind::Cell,
        NodeKind::Figure,
        NodeKind::Caption,
        NodeKind::Legend,
        NodeKind::Admonition,
        NodeKind::Topic,
        NodeKind::Sidebar,
        NodeKind::Rubric,
        NodeKind::Container,
        NodeKind::Compound,
        NodeKind::Footnote,
        NodeKind::Citation,
        NodeKind::Target,
        NodeKind::Comment,
        NodeKind::SubstitutionDefinition,
        NodeKind::Contents,
        NodeKind::Directive,
        NodeKind::Toctree,
        NodeKind::ToctreeEntry,
        NodeKind::Only,
        NodeKind::Glossary,
        NodeKind::VersionModified,
        NodeKind::Desc,
        NodeKind::DescSignature,
        NodeKind::DescContent,
        NodeKind::Hlist,
        NodeKind::ProductionList,
        NodeKind::Production,
        NodeKind::Index,
        NodeKind::Text,
        NodeKind::Emphasis,
        NodeKind::Strong,
        NodeKind::Code,
        NodeKind::Link,
        NodeKind::Reference,
        NodeKind::Image,
        NodeKind::FootnoteReference,
        NodeKind::CitationReference,
        NodeKind::SubstitutionReference,
        NodeKind::InlineTarget,
        NodeKind::Math,
        NodeKind::RawInline,
        NodeKind::SoftBreak,
        NodeKind::HardBreak,
        NodeKind::Subscript,
        NodeKind::Superscript,
        NodeKind::TitleReference,
        NodeKind::Abbreviation,
        NodeKind::Role,
        NodeKind::Xref,
        NodeKind::DescName,
        NodeKind::DescAddname,
        NodeKind::DescAnnotation,
        NodeKind::DescParameterlist,
        NodeKind::DescParameter,
        NodeKind::DescOptional,
        NodeKind::DescReturns,
    ];
}

impl NodeKind {
    /// Returns the kind numbered `v` (its declaration order), or `None`.
    pub fn from_u8(v: u8) -> Option<NodeKind> {
        NodeKind::ALL.get(v as usize).copied()
    }
}

/// One node flag; `NodeFlags` holds a set of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NodeFlag {
    /// List: items render without paragraph spacing.
    Tight,
    /// Reference or Target: anonymous hyperlink.
    Anonymous,
    /// Footnote or reference: auto-numbered.
    Auto,
    /// Footnote or reference: auto-symbol.
    Symbol,
    /// Contents: only the enclosing section.
    Local,
    /// Row or Cell: belongs to a header.
    Header,
    /// Target or Footnote: something refers to it.
    Referenced,
    /// Created by resolution rather than markup.
    Generated,
}

impl NodeFlag {
    /// Every flag, in bit order.
    pub const ALL: [NodeFlag; 8] = [
        NodeFlag::Tight,
        NodeFlag::Anonymous,
        NodeFlag::Auto,
        NodeFlag::Symbol,
        NodeFlag::Local,
        NodeFlag::Header,
        NodeFlag::Referenced,
        NodeFlag::Generated,
    ];

    /// Returns the static name dumps write ("tight").
    pub const fn name(self) -> &'static str {
        match self {
            NodeFlag::Tight => "tight",
            NodeFlag::Anonymous => "anonymous",
            NodeFlag::Auto => "auto",
            NodeFlag::Symbol => "symbol",
            NodeFlag::Local => "local",
            NodeFlag::Header => "header",
            NodeFlag::Referenced => "referenced",
            NodeFlag::Generated => "generated",
        }
    }
}

/// A set of `NodeFlag`s in one byte (saved objects store the byte as it is).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NodeFlags(pub u8);

impl NodeFlags {
    /// The empty set.
    pub const EMPTY: NodeFlags = NodeFlags(0);
    /// `{Tight}`.
    pub const TIGHT: NodeFlags = NodeFlags::of(NodeFlag::Tight);
    /// `{Anonymous}`.
    pub const ANONYMOUS: NodeFlags = NodeFlags::of(NodeFlag::Anonymous);
    /// `{Auto}`.
    pub const AUTO: NodeFlags = NodeFlags::of(NodeFlag::Auto);
    /// `{Symbol}`.
    pub const SYMBOL: NodeFlags = NodeFlags::of(NodeFlag::Symbol);
    /// `{Local}`.
    pub const LOCAL: NodeFlags = NodeFlags::of(NodeFlag::Local);
    /// `{Header}`.
    pub const HEADER: NodeFlags = NodeFlags::of(NodeFlag::Header);
    /// `{Referenced}`.
    pub const REFERENCED: NodeFlags = NodeFlags::of(NodeFlag::Referenced);
    /// `{Generated}`.
    pub const GENERATED: NodeFlags = NodeFlags::of(NodeFlag::Generated);

    /// Returns the set holding only `flag`.
    pub const fn of(flag: NodeFlag) -> NodeFlags {
        NodeFlags(1 << flag as u8)
    }

    /// Tells whether `flag` is in the set (Odin `flag in flags`).
    #[inline]
    pub const fn has(self, flag: NodeFlag) -> bool {
        self.0 & (1 << flag as u8) != 0
    }

    /// Tells whether every flag of `other` is in the set.
    #[inline]
    pub const fn contains(self, other: NodeFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Tells whether the sets share a flag (Odin `flags & other != {}`).
    #[inline]
    pub const fn intersects(self, other: NodeFlags) -> bool {
        self.0 & other.0 != 0
    }

    /// Tells whether the set is empty.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Adds every flag of `other` (Odin `flags += other`).
    #[inline]
    pub fn insert(&mut self, other: NodeFlags) {
        self.0 |= other.0;
    }

    /// Removes every flag of `other` (Odin `flags -= other`).
    #[inline]
    pub fn remove(&mut self, other: NodeFlags) {
        self.0 &= !other.0;
    }
}

impl BitOr for NodeFlags {
    type Output = NodeFlags;
    fn bitor(self, o: NodeFlags) -> NodeFlags {
        NodeFlags(self.0 | o.0)
    }
}

impl BitOrAssign for NodeFlags {
    fn bitor_assign(&mut self, o: NodeFlags) {
        self.0 |= o.0;
    }
}

impl From<NodeFlag> for NodeFlags {
    fn from(f: NodeFlag) -> NodeFlags {
        NodeFlags::of(f)
    }
}

/// One node of the preorder node table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Node {
    /// What the node is.
    pub kind: NodeKind,
    /// Its flags.
    pub flags: NodeFlags,
    /// The id after its last descendant.
    pub subtree_end: u32,
    /// A small per-kind scalar (AST.md), otherwise zero.
    pub value: i32,
    /// Its text in the pool.
    pub text: Text,
    /// Its payload row, or `NONE`.
    pub payload: u32,
    /// Where it comes from.
    pub span: SourceSpan,
    /// Its attribute rows.
    pub attributes: AttributeRange,
}

impl Index<NodeId> for Vec<Node> {
    type Output = Node;
    fn index(&self, id: NodeId) -> &Node {
        &self[id.index()]
    }
}

impl IndexMut<NodeId> for Vec<Node> {
    fn index_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self[id.index()]
    }
}

impl Index<NodeId> for [Node] {
    type Output = Node;
    fn index(&self, id: NodeId) -> &Node {
        &self[id.index()]
    }
}

/// A Document node's `value`: who read it, which decides the ids anchors give.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum DocumentReading {
    /// Read as Docutils reads it.
    #[default]
    Docutils = 0,
    /// Read by the Sphinx layer.
    Sphinx = 1,
}

/// An Admonition node's `value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum AdmonitionKind {
    /// The generic `admonition` directive.
    #[default]
    Generic,
    #[allow(missing_docs)]
    Attention,
    #[allow(missing_docs)]
    Caution,
    #[allow(missing_docs)]
    Danger,
    #[allow(missing_docs)]
    Error,
    #[allow(missing_docs)]
    Hint,
    #[allow(missing_docs)]
    Important,
    #[allow(missing_docs)]
    Note,
    #[allow(missing_docs)]
    Tip,
    #[allow(missing_docs)]
    Warning,
    #[allow(missing_docs)]
    Seealso,
    #[allow(missing_docs)]
    Todo,
}

impl AdmonitionKind {
    /// Every kind, in value order.
    pub const ALL: [AdmonitionKind; 12] = [
        AdmonitionKind::Generic,
        AdmonitionKind::Attention,
        AdmonitionKind::Caution,
        AdmonitionKind::Danger,
        AdmonitionKind::Error,
        AdmonitionKind::Hint,
        AdmonitionKind::Important,
        AdmonitionKind::Note,
        AdmonitionKind::Tip,
        AdmonitionKind::Warning,
        AdmonitionKind::Seealso,
        AdmonitionKind::Todo,
    ];

    /// Returns the kind a node's `value` holds, or `None` when out of range.
    pub fn from_value(v: i32) -> Option<AdmonitionKind> {
        usize::try_from(v).ok().and_then(|i| AdmonitionKind::ALL.get(i).copied())
    }
}

/// A list's numbering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ListStyle {
    #[allow(missing_docs)]
    #[default]
    Bullet,
    #[allow(missing_docs)]
    Arabic,
    #[allow(missing_docs)]
    LowerAlpha,
    #[allow(missing_docs)]
    UpperAlpha,
    #[allow(missing_docs)]
    LowerRoman,
    #[allow(missing_docs)]
    UpperRoman,
}

impl ListStyle {
    /// Every style, in declaration order.
    pub const ALL: [ListStyle; 6] = [
        ListStyle::Bullet,
        ListStyle::Arabic,
        ListStyle::LowerAlpha,
        ListStyle::UpperAlpha,
        ListStyle::LowerRoman,
        ListStyle::UpperRoman,
    ];

    /// Returns the static name dumps write ("loweralpha"), Odin's `LIST_STYLE_NAMES`.
    pub const fn name(self) -> &'static str {
        match self {
            ListStyle::Bullet => "bullet",
            ListStyle::Arabic => "arabic",
            ListStyle::LowerAlpha => "loweralpha",
            ListStyle::UpperAlpha => "upperalpha",
            ListStyle::LowerRoman => "lowerroman",
            ListStyle::UpperRoman => "upperroman",
        }
    }
}

/// A List node's payload row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListInfo {
    /// Numbering style.
    pub style: ListStyle,
    /// '.', ')', or '(' for "(1)"; the bullet character for bullets.
    pub delimiter: u8,
    /// First number.
    pub start: i32,
}

impl Default for ListInfo {
    fn default() -> ListInfo {
        ListInfo { style: ListStyle::Bullet, delimiter: 0, start: 1 }
    }
}

/// What a link row's destination means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum LinkKind {
    /// Destination is a URL or path.
    #[default]
    External,
    /// Destination is an address; HTML adds `mailto:`.
    Email,
    /// `resolved` is a node of this document.
    Internal,
    /// The name awaits resolution.
    Unresolved,
    /// Destination is a project anchor key in another document.
    Project,
}

impl LinkKind {
    /// Every kind, in declaration order.
    pub const ALL: [LinkKind; 5] = [
        LinkKind::External,
        LinkKind::Email,
        LinkKind::Internal,
        LinkKind::Unresolved,
        LinkKind::Project,
    ];

    /// Returns the static name dumps write.
    pub const fn name(self) -> &'static str {
        match self {
            LinkKind::External => "external",
            LinkKind::Email => "email",
            LinkKind::Internal => "internal",
            LinkKind::Unresolved => "unresolved",
            LinkKind::Project => "project",
        }
    }
}

/// A link-like node's payload row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinkInfo {
    /// URL, path, address, or project key.
    pub destination: Text,
    /// Link title.
    pub title: Text,
    /// Normalized reference name, when named.
    pub name: Text,
    /// The referent, meaningful only when `kind` is Internal.
    pub resolved: NodeId,
    /// What the destination means.
    pub kind: LinkKind,
}

impl LinkInfo {
    /// The row `link_info` stands in for a node without one: unresolved, no target.
    pub const UNRESOLVED: LinkInfo = LinkInfo {
        destination: Text { start: 0, len: 0 },
        title: Text { start: 0, len: 0 },
        name: Text { start: 0, len: 0 },
        resolved: INVALID_NODE,
        kind: LinkKind::Unresolved,
    };
}

impl Default for LinkInfo {
    /// An External link with no text and `resolved = INVALID_NODE`.
    fn default() -> LinkInfo {
        LinkInfo { kind: LinkKind::External, ..LinkInfo::UNRESOLVED }
    }
}

/// A cell's alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Align {
    #[allow(missing_docs)]
    #[default]
    None,
    #[allow(missing_docs)]
    Left,
    #[allow(missing_docs)]
    Center,
    #[allow(missing_docs)]
    Right,
}

impl Align {
    /// Every alignment, in declaration order.
    pub const ALL: [Align; 4] = [Align::None, Align::Left, Align::Center, Align::Right];
}

/// A Cell node's payload row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellInfo {
    /// Rows spanned (at least 1).
    pub row_span: u16,
    /// Columns spanned (at least 1).
    pub col_span: u16,
    /// Alignment.
    pub align: Align,
}

impl Default for CellInfo {
    /// One row by one column, unaligned.
    fn default() -> CellInfo {
        CellInfo { row_span: 1, col_span: 1, align: Align::None }
    }
}

/// An attribute's key; `Custom` keys carry their name as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
#[allow(missing_docs)] // The names say what each key holds; AST.md documents their use.
pub enum AttributeKey {
    /// A key named by the attribute's `name` text.
    #[default]
    Custom,
    Class,
    Id,
    Name,
    Language,
    Alt,
    Width,
    Height,
    Scale,
    Align,
    Format,
    Title,
    Target,
    Start,
    Widths,
    Source,
}

impl AttributeKey {
    /// Every key, in declaration order.
    pub const ALL: [AttributeKey; 16] = [
        AttributeKey::Custom,
        AttributeKey::Class,
        AttributeKey::Id,
        AttributeKey::Name,
        AttributeKey::Language,
        AttributeKey::Alt,
        AttributeKey::Width,
        AttributeKey::Height,
        AttributeKey::Scale,
        AttributeKey::Align,
        AttributeKey::Format,
        AttributeKey::Title,
        AttributeKey::Target,
        AttributeKey::Start,
        AttributeKey::Widths,
        AttributeKey::Source,
    ];

    /// Returns the static name dumps write ("class"); "custom" for Custom.
    pub const fn name(self) -> &'static str {
        match self {
            AttributeKey::Custom => "custom",
            AttributeKey::Class => "class",
            AttributeKey::Id => "id",
            AttributeKey::Name => "name",
            AttributeKey::Language => "language",
            AttributeKey::Alt => "alt",
            AttributeKey::Width => "width",
            AttributeKey::Height => "height",
            AttributeKey::Scale => "scale",
            AttributeKey::Align => "align",
            AttributeKey::Format => "format",
            AttributeKey::Title => "title",
            AttributeKey::Target => "target",
            AttributeKey::Start => "start",
            AttributeKey::Widths => "widths",
            AttributeKey::Source => "source",
        }
    }
}

/// One attribute row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Attribute {
    /// The key.
    pub key: AttributeKey,
    /// The key's text when `key` is Custom.
    pub name: Text,
    /// The value.
    pub value: Text,
}

/// What children a kind may have; see `child_allowed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Content {
    /// No children.
    Nothing,
    /// Block children.
    Blocks,
    /// Inline children.
    Inlines,
    /// Children of specific kinds only.
    Structure,
}
