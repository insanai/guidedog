//! What a translatable message is: which nodes carry one (Sphinx's `is_translatable`),
//! how a node's msgid is written, and how index entries split into messages.

use guidedoc::{AttributeKey, Document, NodeFlag, NodeId, NodeKind, SourceSpan};

/// An extra kind of content `gettext_additional_targets` makes translatable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// `index`: index entries.
    Index,
    /// `literal-block`: literal and code blocks, parsed literals.
    LiteralBlock,
    /// `doctest-block`.
    DoctestBlock,
    /// `raw`.
    Raw,
    /// `image`: image alt text.
    Image,
}

impl Target {
    /// Every target, in declaration order.
    pub const ALL: [Target; 5] =
        [Target::Index, Target::LiteralBlock, Target::DoctestBlock, Target::Raw, Target::Image];

    /// Returns the name `gettext_additional_targets` uses for the target.
    pub fn name(self) -> &'static str {
        match self {
            Target::Index => "index",
            Target::LiteralBlock => "literal-block",
            Target::DoctestBlock => "doctest-block",
            Target::Raw => "raw",
            Target::Image => "image",
        }
    }
}

/// Returns the target a `gettext_additional_targets` name stands for, or `None` for an
/// unknown name.
pub fn parse_target(name: &str) -> Option<Target> {
    Target::ALL.into_iter().find(|t| t.name() == name)
}

/// The set of extra targets (`gettext_additional_targets`); `Default` is none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Targets {
    /// Index entries.
    pub index: bool,
    /// Literal and code blocks, parsed literals.
    pub literal_block: bool,
    /// Doctest blocks.
    pub doctest_block: bool,
    /// Raw blocks.
    pub raw: bool,
    /// Image alt text.
    pub image: bool,
}

impl Targets {
    /// Every target.
    pub const ALL: Targets =
        Targets { index: true, literal_block: true, doctest_block: true, raw: true, image: true };

    /// Returns the set holding exactly `targets`.
    pub fn of(targets: &[Target]) -> Targets {
        let mut set = Targets::default();
        for &t in targets {
            set.insert(t);
        }
        set
    }

    /// Adds `t` to the set.
    pub fn insert(&mut self, t: Target) {
        *self.flag(t) = true;
    }

    /// Tells whether the set holds `t`.
    pub fn contains(mut self, t: Target) -> bool {
        *self.flag(t)
    }

    fn flag(&mut self, t: Target) -> &mut bool {
        match t {
            Target::Index => &mut self.index,
            Target::LiteralBlock => &mut self.literal_block,
            Target::DoctestBlock => &mut self.doctest_block,
            Target::Raw => &mut self.raw,
            Target::Image => &mut self.image,
        }
    }
}

/// How a node carries a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unit {
    /// It carries none.
    #[default]
    None,
    /// Inline content: the source text with its markup; translations are re-parsed.
    Inline,
    /// The node's text, verbatim: code, doctest, raw, and math blocks.
    Literal,
    /// A parsed literal: inline content, only with the literal-block target.
    Parsed,
    /// An image's Alt attribute.
    Alt,
    /// Diagram alt text and, with the literal-block target, its source.
    Graphviz,
    /// Each part of each index entry.
    Index,
    /// A toctree's caption attribute.
    Caption,
    /// A toctree entry's explicit title.
    Entry,
    /// The values of a meta directive.
    Meta,
    /// A contents directive's title.
    Contents,
}

impl Unit {
    /// Tells whether the unit is inline content that translations re-parse.
    pub fn is_inline(self) -> bool {
        matches!(self, Unit::Inline | Unit::Parsed)
    }
}

/// Classifies node `id` of `doc` as Sphinx's `is_translatable` does, with the extra
/// `targets`. The match is exhaustive, so a new node kind does not compile until it says
/// whether it is translated.
// One line per group of kinds keeps the exhaustive match readable.
#[rustfmt::skip]
pub fn unit_of(doc: Document<'_>, id: NodeId, targets: Targets) -> Unit {
    use NodeKind as K;
    match doc.kind(id) {
        K::Title | K::Subtitle | K::Heading | K::Paragraph | K::Attribution | K::Term
        | K::Classifier | K::Line | K::Caption | K::Rubric => Unit::Inline,
        // Sphinx leaves the file-wide :orphan: marker (and its like) alone.
        K::FieldName if is_field_marker(doc, id) => Unit::None,
        K::FieldName => Unit::Inline,
        K::Preformatted if targets.literal_block => Unit::Parsed,
        K::CodeBlock => code_block_unit(doc, id, targets),
        K::DoctestBlock if targets.doctest_block => Unit::Literal,
        K::RawBlock if targets.raw => Unit::Literal,
        K::MathBlock => Unit::Literal,
        K::Image if targets.image && doc.attribute(id, AttributeKey::Alt).is_some() => Unit::Alt,
        K::Index if targets.index => Unit::Index,
        K::Toctree if doc.custom_attribute(id, "caption").is_some() => Unit::Caption,
        K::ToctreeEntry if doc.attribute(id, AttributeKey::Title).is_some() => Unit::Entry,
        K::Contents if doc.attribute(id, AttributeKey::Title).is_some() => Unit::Contents,
        K::Directive if doc.attribute(id, AttributeKey::Name) == Some("meta") => Unit::Meta,
        // The same kinds without the target or attribute that makes them translatable.
        K::Preformatted | K::DoctestBlock | K::RawBlock | K::Image | K::Index | K::Toctree
        | K::ToctreeEntry | K::Contents | K::Directive => Unit::None,
        // Containers, whose children are the units, and the parts Sphinx leaves
        // untranslated: options, signatures, grammar rules, and what a transform wrote.
        K::Document | K::Section | K::BlockQuote | K::List | K::ListItem | K::DefinitionList
        | K::DefinitionItem | K::Definition | K::FieldList | K::Field | K::FieldBody
        | K::OptionList | K::OptionItem | K::OptionGroup | K::Option | K::OptionArgument
        | K::OptionDescription | K::LineBlock | K::ThematicBreak | K::Table | K::TableHead
        | K::TableBody | K::Row | K::Cell | K::Figure | K::Legend | K::Admonition | K::Topic
        | K::Sidebar | K::Container | K::Compound | K::Footnote | K::Citation | K::Target
        | K::Comment | K::SubstitutionDefinition | K::Only | K::Glossary | K::VersionModified
        | K::Desc | K::DescSignature | K::DescContent | K::Hlist | K::ProductionList
        | K::Production => Unit::None,
        // Inline content belongs to the unit that holds it.
        K::Text | K::Emphasis | K::Strong | K::Code | K::Link | K::Reference
        | K::FootnoteReference | K::CitationReference | K::SubstitutionReference
        | K::InlineTarget | K::Math | K::RawInline | K::SoftBreak | K::HardBreak
        | K::Subscript | K::Superscript | K::TitleReference | K::Abbreviation | K::Role
        | K::Xref | K::DescName | K::DescAddname | K::DescAnnotation | K::DescParameterlist
        | K::DescParameter | K::DescOptional | K::DescReturns => Unit::None,
    }
}

/// A code block is a diagram with alt text (graphviz), translated with the image target,
/// or a literal, translated with the literal-block target.
fn code_block_unit(doc: Document<'_>, id: NodeId, targets: Targets) -> Unit {
    let diagram = doc.custom_attribute(id, "graphviz").is_some();
    let alt = doc.custom_attribute(id, "alt").is_some();
    if diagram && alt && targets.image {
        Unit::Graphviz
    } else if targets.literal_block {
        Unit::Literal
    } else {
        Unit::None
    }
}

/// Tells whether a field name is a file-wide marker Sphinx does not translate.
fn is_field_marker(doc: Document<'_>, id: NodeId) -> bool {
    let mut name = String::new();
    doc.plain_text(id, &mut name);
    matches!(name.as_str(), "orphan" | "nocomments" | "nosearch" | "tocdepth")
}

/// Returns the first child of `id` after the Generated children it starts with (words a
/// transform put in front, such as an info field's argument), or the node's end.
pub(crate) fn written_content(doc: Document<'_>, id: NodeId) -> NodeId {
    let end = doc.subtree_end(id);
    let mut c = id + 1;
    while c < end && doc.node(c).flags.has(NodeFlag::Generated) {
        c = doc.subtree_end(c);
    }
    c
}

/// Returns the source range of a node's inline content: from its first descendant with
/// source text to the end of its last, in the first one's source. A node whose content
/// was generated rather than written has none.
pub(crate) fn inline_span(doc: Document<'_>, id: NodeId) -> Option<SourceSpan> {
    let mut span: Option<SourceSpan> = None;
    for i in id.0 + 1..doc.subtree_end(id).0 {
        let n = doc.node(NodeId(i));
        if n.span.end <= n.span.start || n.flags.has(NodeFlag::Generated) {
            continue;
        }
        match &mut span {
            None => span = Some(n.span),
            Some(s) if s.source == n.span.source => {
                s.start = s.start.min(n.span.start);
                s.end = s.end.max(n.span.end);
            }
            Some(_) => {}
        }
    }
    span
}

/// Writes node `id`'s msgid into `out` (cleared first) as Sphinx makes it from the
/// node's source: line breaks and the indentation after them become one space, and the
/// ends are trimmed. Returns false, with `out` empty, when the node has no message.
pub fn write_message(doc: Document<'_>, id: NodeId, out: &mut String) -> bool {
    out.clear();
    let Some(span) = inline_span(doc, id) else {
        return false;
    };
    normalize_lines(doc.source_text(span), out);
    !out.is_empty()
}

/// Joins lines with single spaces, dropping each line's indentation and trailing white
/// space, as Sphinx's `rawsource.replace('\n', ' ').strip()` sees a paragraph after
/// Docutils has removed its indentation.
fn normalize_lines(text: &str, out: &mut String) {
    for line in text.split('\n') {
        let line = line.trim_matches([' ', '\t', '\r']);
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(line);
    }
}

/// Returns a literal node's msgid: its text without the final line break.
pub(crate) fn literal_message(doc: Document<'_>, id: NodeId) -> &str {
    let text = doc.node_text(id);
    text.strip_suffix('\n').unwrap_or(text)
}

/// One translatable piece of an index entry line, e.g. "b" of "single: a; b".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexPart<'a> {
    /// The piece's text.
    pub text: &'a str,
    /// Its byte offset in the line.
    pub start: usize,
}

/// The entry types whose name and colon start an index entry line.
const INDEX_TYPES: [&str; 12] = [
    "single",
    "pair",
    "triple",
    "see",
    "seealso",
    "module",
    "keyword",
    "operator",
    "object",
    "exception",
    "statement",
    "builtin",
];

/// Splits an index entry line into its translatable pieces, as Sphinx's
/// `split_index_msg` does: the entry type and the "!" main marker stay out of them.
/// Borrows `line`; the pieces come in order.
pub fn index_parts(line: &str) -> impl Iterator<Item = IndexPart<'_>> {
    let body = match line.find(':') {
        Some(colon) if colon > 0 && INDEX_TYPES.contains(&line[..colon].trim()) => colon + 1,
        _ => 0,
    };
    let mut at = body;
    line[body..].split(';').filter_map(move |piece| {
        let start = at;
        at += piece.len() + 1;
        let lead = piece.len() - piece.trim_start_matches([' ', '\t', '!']).len();
        let text = piece[lead..].trim();
        (!text.is_empty()).then_some(IndexPart { text, start: start + lead })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_parts_split_like_sphinx() {
        let line = "pair: loop; !statement";
        let parts: Vec<_> = index_parts(line).collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].text, "loop");
        assert_eq!(parts[1].text, "statement");
        assert_eq!(&line[parts[1].start..][..parts[1].text.len()], "statement");
        let plain: Vec<_> = index_parts("plain entry").collect();
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].text, "plain entry");
        assert_eq!(index_parts("single: a;; b").count(), 2);
    }

    #[test]
    fn targets_parse_their_names() {
        for t in Target::ALL {
            assert_eq!(parse_target(t.name()), Some(t));
        }
        assert_eq!(parse_target("figure"), None);
        let set = Targets::of(&[Target::Index, Target::Image]);
        assert!(set.contains(Target::Index) && set.contains(Target::Image));
        assert!(!set.contains(Target::Raw));
    }

    #[test]
    fn lines_join_with_one_space() {
        let mut out = String::new();
        normalize_lines("  first line  \n\t  second\r\n   third", &mut out);
        assert_eq!(out, "first line second third");
    }
}
