//! Kind tables: block or inline class, payload table, content model, legal parent-child
//! pairs, and static names. Matches are exhaustive, so a new kind does not compile until
//! every table says what it does.

use crate::types::{AdmonitionKind, Content, NodeKind};

/// Whether nodes of a kind are blocks, inlines, or either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KindClass {
    Block,
    Inline,
    Either,
}

/// The table a node's payload row indexes, which its kind decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Payload {
    None,
    List,
    Cell,
    Link,
}

/// Tells whether nodes of kind `k` are blocks, inlines, or either.
pub(crate) fn kind_class(k: NodeKind) -> KindClass {
    if k == NodeKind::Image || k == NodeKind::Index {
        return KindClass::Either;
    }
    if k >= NodeKind::Text {
        return KindClass::Inline;
    }
    KindClass::Block
}

/// Returns the table a kind's payload row indexes. The match is exhaustive, so a new
/// kind does not compile until it says which table it uses.
#[rustfmt::skip] // Kinds listed one per line would pass the function-length limit.
pub(crate) fn payload_of(k: NodeKind) -> Payload {
    use NodeKind as K;
    match k {
        K::List => Payload::List,
        K::Cell => Payload::Cell,
        K::Link | K::Reference | K::Image | K::FootnoteReference | K::CitationReference
        | K::SubstitutionReference | K::Target | K::InlineTarget | K::TitleReference
        | K::Xref => Payload::Link,
        K::Document | K::Title | K::Subtitle | K::Section | K::Heading | K::Paragraph
        | K::BlockQuote | K::Attribution | K::ListItem | K::DefinitionList
        | K::DefinitionItem | K::Term | K::Classifier | K::Definition | K::FieldList
        | K::Field | K::FieldName | K::FieldBody | K::OptionList | K::OptionItem
        | K::OptionGroup | K::Option | K::OptionArgument | K::OptionDescription
        | K::LineBlock | K::Line | K::CodeBlock | K::Preformatted | K::DoctestBlock
        | K::MathBlock | K::RawBlock | K::ThematicBreak | K::Table | K::TableHead
        | K::TableBody | K::Row | K::Figure | K::Caption | K::Legend | K::Admonition
        | K::Topic | K::Sidebar | K::Rubric | K::Container | K::Compound | K::Footnote
        | K::Citation | K::Comment | K::SubstitutionDefinition | K::Contents | K::Directive
        | K::Toctree | K::ToctreeEntry | K::Only | K::Glossary | K::VersionModified
        | K::Desc | K::DescSignature | K::DescContent | K::Hlist | K::ProductionList
        | K::Production | K::Index | K::Text | K::Emphasis | K::Strong | K::Code | K::Math
        | K::RawInline | K::SoftBreak | K::HardBreak | K::Subscript | K::Superscript
        | K::Abbreviation | K::Role | K::DescName | K::DescAddname | K::DescAnnotation
        | K::DescParameterlist | K::DescParameter | K::DescOptional | K::DescReturns => {
            Payload::None
        }
    }
}

/// Tells what children nodes of kind `k` may have; see `child_allowed`. Exhaustive, as
/// `payload_of` is.
#[rustfmt::skip] // Kinds listed one per line would pass the function-length limit.
pub fn content_model(k: NodeKind) -> Content {
    use NodeKind as K;
    match k {
        K::Document | K::Section | K::BlockQuote | K::ListItem | K::Definition
        | K::FieldBody | K::OptionDescription | K::Cell | K::Legend | K::Admonition
        | K::Topic | K::Sidebar | K::Container | K::Compound | K::Footnote | K::Citation
        | K::Directive | K::Only | K::VersionModified | K::DescContent => Content::Blocks,
        K::Title | K::Subtitle | K::Heading | K::Paragraph | K::Attribution | K::Term
        | K::Classifier | K::FieldName | K::Line | K::Caption | K::Rubric
        | K::SubstitutionDefinition | K::Preformatted | K::Emphasis | K::Strong | K::Link
        | K::Reference | K::Image | K::InlineTarget | K::Subscript | K::Superscript
        | K::TitleReference | K::Abbreviation | K::Role | K::DescSignature | K::Production
        | K::Xref | K::DescName | K::DescAddname | K::DescAnnotation | K::DescParameterlist
        | K::DescParameter | K::DescOptional | K::DescReturns => Content::Inlines,
        K::List | K::DefinitionList | K::DefinitionItem | K::FieldList | K::Field
        | K::OptionList | K::OptionItem | K::OptionGroup | K::Option | K::LineBlock
        | K::Table | K::TableHead | K::TableBody | K::Row | K::Figure | K::Toctree
        | K::Glossary | K::Desc | K::Hlist | K::ProductionList | K::CodeBlock => {
            Content::Structure
        }
        K::OptionArgument | K::DoctestBlock | K::MathBlock | K::RawBlock | K::ThematicBreak
        | K::Target | K::Comment | K::Contents | K::ToctreeEntry | K::Index | K::Text
        | K::Code | K::FootnoteReference | K::CitationReference | K::SubstitutionReference
        | K::Math | K::RawInline | K::SoftBreak | K::HardBreak => Content::Nothing,
    }
}

/// States the legal parent-child pairs. Builders and validation share it.
pub fn child_allowed(parent: NodeKind, child: NodeKind) -> bool {
    match content_model(parent) {
        Content::Nothing => false,
        Content::Inlines => kind_class(child) != KindClass::Block,
        Content::Blocks => block_child_allowed(parent, child),
        Content::Structure => structure_child_allowed(parent, child),
    }
}

fn block_child_allowed(parent: NodeKind, child: NodeKind) -> bool {
    use NodeKind as K;
    if kind_class(child) == KindClass::Inline {
        return false;
    }
    match child {
        K::Section => matches!(parent, K::Document | K::Section | K::Only | K::DescContent),
        K::Title => {
            matches!(parent, K::Document | K::Admonition | K::Topic | K::Sidebar | K::Directive)
        }
        K::Subtitle => matches!(parent, K::Document | K::Sidebar),
        K::Attribution => parent == K::BlockQuote,
        K::Caption
        | K::Legend
        | K::Row
        | K::Cell
        | K::TableHead
        | K::TableBody
        | K::ListItem
        | K::DefinitionItem
        | K::Term
        | K::Classifier
        | K::Definition
        | K::Field
        | K::FieldName
        | K::FieldBody
        | K::OptionItem
        | K::OptionGroup
        | K::Option
        | K::OptionArgument
        | K::OptionDescription
        | K::Line
        | K::ToctreeEntry
        | K::DescSignature
        | K::DescContent
        | K::Production => false,
        _ => true,
    }
}

fn structure_child_allowed(parent: NodeKind, child: NodeKind) -> bool {
    use NodeKind as K;
    match parent {
        K::List => child == K::ListItem,
        K::DefinitionList => child == K::DefinitionItem,
        K::DefinitionItem => matches!(child, K::Term | K::Classifier | K::Definition),
        K::FieldList => child == K::Field,
        K::Field => matches!(child, K::FieldName | K::FieldBody),
        K::OptionList => child == K::OptionItem,
        K::OptionItem => matches!(child, K::OptionGroup | K::OptionDescription),
        K::OptionGroup => child == K::Option,
        K::Option => child == K::OptionArgument,
        K::LineBlock => matches!(child, K::Line | K::LineBlock),
        K::Table => matches!(child, K::TableHead | K::TableBody | K::Title),
        K::TableHead | K::TableBody => child == K::Row,
        K::Row => child == K::Cell,
        K::Figure => matches!(child, K::Image | K::Caption | K::Legend | K::Reference | K::Link),
        K::CodeBlock => child == K::Caption,
        K::Toctree => child == K::ToctreeEntry,
        K::Glossary => child == K::DefinitionList,
        K::Desc => matches!(child, K::DescSignature | K::DescContent | K::Index),
        K::Hlist => child == K::List,
        K::ProductionList => child == K::Production,
        _ => false,
    }
}

/// Returns the static name of a node kind, as dumps and diagnostics write it.
pub fn kind_name(k: NodeKind) -> &'static str {
    KIND_NAMES[k as usize]
}

/// Returns the static class name of an admonition kind ("note", "tip").
pub fn admonition_name(k: AdmonitionKind) -> &'static str {
    match k {
        AdmonitionKind::Generic => "admonition",
        AdmonitionKind::Attention => "attention",
        AdmonitionKind::Caution => "caution",
        AdmonitionKind::Danger => "danger",
        AdmonitionKind::Error => "error",
        AdmonitionKind::Hint => "hint",
        AdmonitionKind::Important => "important",
        AdmonitionKind::Note => "note",
        AdmonitionKind::Tip => "tip",
        AdmonitionKind::Warning => "warning",
        AdmonitionKind::Seealso => "seealso",
        AdmonitionKind::Todo => "todo",
    }
}

/// Names in `NodeKind` declaration order.
const KIND_NAMES: [&str; NodeKind::ALL.len()] = [
    "document",
    "title",
    "subtitle",
    "section",
    "heading",
    "paragraph",
    "block-quote",
    "attribution",
    "list",
    "list-item",
    "definition-list",
    "definition-item",
    "term",
    "classifier",
    "definition",
    "field-list",
    "field",
    "field-name",
    "field-body",
    "option-list",
    "option-item",
    "option-group",
    "option",
    "option-argument",
    "option-description",
    "line-block",
    "line",
    "code-block",
    "preformatted",
    "doctest-block",
    "math-block",
    "raw-block",
    "thematic-break",
    "table",
    "table-head",
    "table-body",
    "row",
    "cell",
    "figure",
    "caption",
    "legend",
    "admonition",
    "topic",
    "sidebar",
    "rubric",
    "container",
    "compound",
    "footnote",
    "citation",
    "target",
    "comment",
    "substitution-definition",
    "contents",
    "directive",
    "toctree",
    "toctree-entry",
    "only",
    "glossary",
    "version-modified",
    "desc",
    "desc-signature",
    "desc-content",
    "hlist",
    "production-list",
    "production",
    "index",
    "text",
    "emphasis",
    "strong",
    "code",
    "link",
    "reference",
    "image",
    "footnote-reference",
    "citation-reference",
    "substitution-reference",
    "inline-target",
    "math",
    "raw-inline",
    "soft-break",
    "hard-break",
    "subscript",
    "superscript",
    "title-reference",
    "abbreviation",
    "role",
    "xref",
    "desc-name",
    "desc-addname",
    "desc-annotation",
    "desc-parameterlist",
    "desc-parameter",
    "desc-optional",
    "desc-returns",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_name_in_order() {
        assert_eq!(NodeKind::ALL.len(), KIND_NAMES.len());
        for (i, k) in NodeKind::ALL.iter().enumerate() {
            assert_eq!(*k as usize, i);
        }
        assert_eq!(kind_name(NodeKind::BlockQuote), "block-quote");
        assert_eq!(kind_name(NodeKind::DescReturns), "desc-returns");
        assert_eq!(kind_name(NodeKind::Toctree), "toctree");
        assert_eq!(kind_name(NodeKind::Text), "text");
    }

    #[test]
    fn content_rules() {
        assert!(child_allowed(NodeKind::Document, NodeKind::Section));
        assert!(!child_allowed(NodeKind::Paragraph, NodeKind::Paragraph));
        assert!(child_allowed(NodeKind::Paragraph, NodeKind::Image));
        assert!(child_allowed(NodeKind::List, NodeKind::ListItem));
        assert!(!child_allowed(NodeKind::Text, NodeKind::Text));
    }
}
