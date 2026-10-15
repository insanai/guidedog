//! The walk: `node` is the one exhaustive match over `NodeKind`. Blocks write lines;
//! inline nodes append to the inline buffer of the block that collects them. Kinds
//! that only appear inside a parent the parent writes; met on their own, they are
//! written as the closest block.

use guidedoc::{NodeId, NodeKind};

use crate::state::Writer;

impl Writer<'_, '_> {
    /// Writes node `id` and its subtree. Every node kind is routed here; a new kind
    /// does not compile until it is.
    #[rustfmt::skip]
    pub fn node(&mut self, id: NodeId) {
        let kind = self.kind(id);
        let out_of_order = matches!(kind, NodeKind::Comment | NodeKind::Target | NodeKind::Topic);
        if out_of_order && self.done.contains(&id) {
            return;
        }
        if !self.enter(id) {
            return;
        }
        use NodeKind as K;
        match kind {
            K::Document => self.blocks(id),
            // Sections and titles.
            K::Section => self.section(id), K::Title => self.title(id),
            K::Subtitle => self.subtitle(id), K::Heading => self.heading(id),
            // Paragraph-like blocks.
            K::Paragraph => self.paragraph(id), K::Rubric => self.rubric(id),
            K::Caption | K::Attribution | K::Term | K::Classifier | K::FieldName | K::Line
            | K::Production => self.plain_block(id),
            K::Preformatted => self.parsed_literal(id),
            // Lists.
            K::List => self.list(id), K::DefinitionList => self.definition_list(id),
            K::FieldList => self.field_list(id), K::OptionList => self.option_list(id),
            K::LineBlock => self.line_block(id),
            K::ListItem | K::Definition | K::FieldBody | K::OptionDescription | K::Legend
            | K::Cell | K::DescContent | K::DefinitionItem | K::Field | K::OptionItem
            | K::TableHead | K::TableBody | K::Row => self.blocks(id),
            K::OptionGroup | K::Option | K::OptionArgument | K::ToctreeEntry
            | K::DescSignature => self.lost_part(id),
            // Literal content.
            K::CodeBlock => self.code_block(id), K::DoctestBlock => self.doctest_block(id),
            K::MathBlock => self.math_block(id), K::RawBlock => self.raw_block(id),
            K::ThematicBreak => self.transition(id), K::Comment => self.comment(id),
            // Quotes, tables, figures, and directives.
            K::BlockQuote => self.block_quote(id), K::Table => self.table(id),
            K::Figure => self.figure(id), K::Admonition => self.admonition(id),
            K::Topic => self.topic(id), K::Sidebar => self.sidebar(id),
            K::Container => self.container(id), K::Compound => self.compound(id),
            K::Contents => self.contents(id), K::Directive => self.directive(id),
            // Notes, targets, substitutions.
            K::Footnote | K::Citation => self.note(id), K::Target => self.target(id),
            K::SubstitutionDefinition => self.substitution_definition(id),
            // The Sphinx layer's blocks.
            K::Toctree => self.toctree(id), K::Only => self.only(id),
            K::Glossary => self.glossary(id), K::VersionModified => self.version_modified(id),
            K::Desc => self.desc(id), K::Hlist => self.hlist(id),
            K::ProductionList => self.production_list(id), K::Index => self.index(id),
            // Inline content (Image and Index may also be blocks).
            K::Image => self.image(id), K::Text => self.text_node(id),
            K::SoftBreak => self.inline.soft_break(), K::HardBreak => self.hard_break(id),
            K::Emphasis => self.emphasis(id, "*", "emphasis"),
            K::Strong => self.emphasis(id, "**", "strong"),
            K::Code => self.code(id), K::Link | K::Reference => self.link(id),
            K::FootnoteReference | K::CitationReference => self.note_reference(id),
            K::SubstitutionReference => self.substitution_reference(id),
            K::InlineTarget => self.inline_target(id), K::Math => self.math(id),
            K::RawInline => self.raw_inline(id),
            K::Subscript => self.simple_role(id, "sub"),
            K::Superscript => self.simple_role(id, "sup"),
            K::TitleReference => self.title_reference(id),
            K::Abbreviation => self.abbreviation(id), K::Role => self.role(id),
            K::Xref => self.xref(id),
            K::DescName | K::DescAddname | K::DescAnnotation | K::DescParameterlist
            | K::DescParameter | K::DescOptional | K::DescReturns => self.inline_children(id),
        }
        self.leave();
    }

    /// Writes the children of `id` as blocks, remembering each one's predecessor.
    pub fn blocks(&mut self, id: NodeId) {
        let saved = (self.prev_block, self.container_end);
        self.prev_block = None;
        self.container_end = self.doc.subtree_end(id).0;
        for child in self.doc.children(id) {
            self.node(child);
            self.prev_block = Some(child);
        }
        (self.prev_block, self.container_end) = saved;
    }

    /// Appends the inline children of `id` to the inline buffer.
    pub fn inline_children(&mut self, id: NodeId) {
        for child in self.doc.children(id) {
            self.node(child);
        }
    }

    /// A part that its parent writes, met on its own: its text, as a paragraph.
    fn lost_part(&mut self, id: NodeId) {
        self.lossy(id, "a part of a list or directive outside it", "its text as a paragraph");
        let mut text = String::new();
        self.doc.plain_text(id, &mut text);
        self.block_start();
        self.line(&text);
        self.block_end();
    }
}
