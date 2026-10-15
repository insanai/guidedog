//! The walk: a loop over the preorder node table with an explicit stack of open
//! ancestors, so a tree nested far deeper than any thread's stack renders. `enter` is
//! the one exhaustive match over `NodeKind`; it writes a node's start and returns how
//! to close it, which the walk keeps on its stack for `leave`.

use guidedoc::{Arg, CORE_DEPTH, CORE_STACK, NodeId, NodeKind, Status, arg_int};

use crate::blocks::ParagraphStyle;
use crate::state::Html;

/// What `enter` decided about a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Visit {
    /// `enter` wrote the whole subtree (or nothing of it): skip the children.
    Skip,
    /// Render the children, then close the node as this says.
    Descend(Close),
}

/// How `leave` closes a node whose children the walk rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Close {
    /// Nothing to write.
    Nothing,
    /// `</name>`.
    Tag(&'static str),
    /// `</name>` and the end of the line.
    Line(&'static str),
    /// `</name>` on a line of its own.
    Block(&'static str),
    /// Fixed text.
    Text(&'static str),
    /// A paragraph, as it was opened.
    Paragraph(ParagraphStyle),
    /// A term or classifier that ends its `<dt>`; with a glossary permalink.
    Dt { permalink: bool },
    /// A classifier; `last` when no classifier follows it.
    Classifier { last: bool },
    /// A figure caption.
    Caption,
    /// A heading, numbered and linked by its section.
    Heading,
    /// A title written as `<name>`.
    Title(&'static str),
    /// A table and its wrapper.
    Table,
    /// A link or reference, and the footnote references target-notes add after it.
    Link,
    /// A domain object's signature.
    Signature,
    /// The name of a field in an object's info fields.
    ObjectFieldName,
    /// A paragraph of a list item in an object's info fields.
    ObjectFieldParagraph,
}

impl Html<'_, '_> {
    /// Returns the ancestor `k` levels up (1: the parent) of the node being rendered.
    pub fn ancestor(&self, k: usize) -> Option<NodeId> {
        let used = self.stack.len() - self.base;
        if used >= k {
            return Some(self.stack[self.stack.len() - k].0);
        }
        if used + 1 == k { self.outer } else { None }
    }

    /// Returns the parent of the node being rendered.
    pub fn parent(&self) -> Option<NodeId> {
        self.ancestor(1)
    }

    /// Returns the open ancestors of the current walk, outermost first.
    pub fn open_ancestors(&self) -> impl DoubleEndedIterator<Item = NodeId> + '_ {
        self.stack[self.base..].iter().map(|&(id, _)| id)
    }

    /// Renders the sibling subtrees in [first, end); `outer` is their parent when it is
    /// not on the stack (a substitution's definition, an hlist column).
    pub fn walk(&mut self, first: NodeId, end: NodeId, outer: Option<NodeId>) {
        if self.guard.is_exhausted() {
            if first < end {
                let span = self.doc.node(first).span;
                let limit = i64::try_from(self.guard.limit()).unwrap_or(i64::MAX);
                self.w.fail(CORE_STACK, span, arg_int(limit), Arg::None);
            }
            return;
        }
        let saved = (self.base, self.outer);
        self.base = self.stack.len();
        self.outer = outer;
        self.walk_range(first, end);
        while self.stack.len() > self.base && self.w.status == Status::Ok {
            if let Some((id, close)) = self.stack.pop() {
                self.leave(id, close);
            }
        }
        self.stack.truncate(self.base);
        (self.base, self.outer) = saved;
    }

    fn walk_range(&mut self, first: NodeId, end: NodeId) {
        let limit = self.limits.max_depth.max(16) + 2;
        let mut i = first;
        while i < end && self.w.status == Status::Ok {
            while let Some(&(open, close)) = self.stack[self.base..].last() {
                if self.doc.subtree_end(open) > i {
                    break;
                }
                self.stack.pop();
                self.leave(open, close);
            }
            let Visit::Descend(close) = self.enter(i) else {
                i = self.doc.subtree_end(i);
                continue;
            };
            if self.stack.len() - self.base >= limit {
                let span = self.doc.node(i).span;
                let max = i64::try_from(limit).unwrap_or(i64::MAX);
                self.w.fail(CORE_DEPTH, span, arg_int(max), Arg::None);
                return;
            }
            self.stack.push((i, close));
            i = i + 1;
        }
    }

    /// Writes the start of node `id` and says whether to render its children. Every
    /// node kind is routed here; a new kind does not compile until it is.
    #[rustfmt::skip]
    fn enter(&mut self, id: NodeId) -> Visit {
        use NodeKind as K;
        match self.doc.kind(id) {
            K::Comment | K::SubstitutionDefinition | K::Toctree | K::ToctreeEntry => Visit::Skip,
            K::Document | K::DefinitionItem | K::Field | K::OptionItem | K::Only => {
                Visit::Descend(Close::Nothing)
            }
            // Blocks.
            K::Paragraph => self.enter_paragraph(id),
            K::BlockQuote => self.block("blockquote", id, ""),
            K::Attribution => self.enter_attribution(id), K::List => self.enter_list(id),
            K::ListItem => self.enter_list_item(id),
            K::DefinitionList => self.enter_definition_list(id), K::Term => self.enter_term(id),
            K::Classifier => self.enter_classifier(id),
            K::Definition | K::OptionDescription => self.enter_dd(id),
            K::FieldList => self.enter_field_list(id), K::FieldName => self.enter_field_name(id),
            K::FieldBody => self.enter_field_body(id), K::OptionGroup => self.line("dt", id, ""),
            K::OptionList => self.block("dl", id, "option-list"),
            K::Option => self.enter_option(id),
            K::OptionArgument => self.enter_option_argument(id),
            K::LineBlock => self.block("div", id, "line-block"), K::Line => self.enter_line(id),
            K::Preformatted => self.line("pre", id, "parsed-literal"),
            K::CodeBlock => self.enter_code(id), K::DoctestBlock => self.enter_doctest(id),
            K::MathBlock => self.enter_math_block(id), K::RawBlock => self.enter_raw(id, true),
            K::ThematicBreak => self.enter_thematic_break(id),
            // Figures, admonitions, and the other titled containers.
            K::Figure => self.enter_figure(id), K::Caption => self.enter_caption(id),
            K::Legend => self.block("div", id, "legend"),
            K::Admonition => self.enter_admonition(id),
            K::Topic => self.block("aside", id, "topic"),
            K::Sidebar => self.block("aside", id, "sidebar"),
            K::Rubric => self.line("p", id, "rubric"), K::Container => self.enter_container(id),
            K::Compound => self.block("div", id, "compound"), K::Target => self.enter_target(id),
            // Sections, headings, and titles.
            K::Section => self.enter_section(id), K::Heading => self.enter_heading(id),
            K::Title => self.enter_title(id), K::Subtitle => self.enter_subtitle(id),
            // Tables, notes, and directives.
            K::Table => self.enter_table(id), K::TableHead => self.block("thead", id, ""),
            K::TableBody => self.block("tbody", id, ""), K::Row => self.block("tr", id, ""),
            K::Cell => self.enter_cell(id), K::Footnote | K::Citation => self.enter_note(id),
            K::Contents => self.enter_contents(id), K::Directive => self.enter_directive(id),
            // The Sphinx layer's blocks.
            K::Index => self.enter_index(id), K::Glossary => self.enter_glossary(id),
            K::VersionModified => self.enter_version(id), K::Desc => self.enter_desc(id),
            K::DescSignature => self.enter_signature(id), K::DescContent => self.enter_dd(id),
            K::Hlist => self.enter_hlist(id), K::ProductionList => self.enter_production_list(id),
            K::Production => self.enter_production(id),
            // Inline content.
            K::Image => self.enter_image(id),
            K::Text => { self.w.esc(self.doc.node_text(id)); Visit::Skip }
            K::SoftBreak => { self.w.put_byte(b'\n'); Visit::Skip }
            K::HardBreak => { self.w.put("<br />"); self.w.cr(); Visit::Skip }
            K::Code => self.enter_code_span(id), K::Math => self.enter_math(id),
            K::RawInline => self.enter_raw(id, false),
            K::Link | K::Reference => self.enter_link(id),
            K::FootnoteReference | K::CitationReference => self.enter_note_reference(id),
            K::SubstitutionReference => self.enter_substitution(id),
            K::Emphasis => self.inline("em", id, ""), K::Strong => self.inline("strong", id, ""),
            K::Subscript => self.inline("sub", id, ""),
            K::Superscript => self.inline("sup", id, ""),
            K::TitleReference => self.inline("cite", id, ""),
            K::InlineTarget => self.inline("span", id, ""),
            K::Abbreviation => self.enter_abbreviation(id), K::Role => self.enter_role(id),
            K::Xref => self.enter_xref(id),
            K::DescName => self.inline("span", id, "sig-name descname"),
            K::DescAddname => self.inline("span", id, "sig-prename descclassname"),
            K::DescAnnotation => self.inline("em", id, "property"),
            K::DescParameterlist => self.enter_parameter_list(),
            K::DescParameter => self.enter_parameter(id), K::DescOptional => self.enter_optional(),
            K::DescReturns => self.enter_returns(id),
        }
    }

    /// Closes what `enter` opened for a node whose children the walk rendered.
    fn leave(&mut self, id: NodeId, close: Close) {
        match close {
            Close::Nothing => {}
            Close::Tag(name) => self.w.close(name),
            Close::Line(name) => self.line_close(name),
            Close::Block(name) => self.block_close(name),
            Close::Text(text) => self.w.put(text),
            Close::Paragraph(style) => self.leave_paragraph(style),
            Close::Dt { permalink } => self.leave_dt(id, permalink),
            Close::Classifier { last } => {
                self.w.close("span");
                if last {
                    self.line_close("dt");
                }
            }
            Close::Caption => self.leave_caption(),
            Close::Heading => self.leave_heading(id),
            Close::Title(name) => self.leave_title(name),
            Close::Table => self.leave_table(),
            Close::Link => self.leave_link(id),
            Close::Signature => self.leave_signature(id),
            Close::ObjectFieldName => {
                self.w.put("<span class=\"colon\">:</span></dt>");
                self.w.cr();
            }
            Close::ObjectFieldParagraph => self.leave_object_field_paragraph(),
        }
    }

    /// Opens a block element holding blocks.
    pub fn block(&mut self, name: &'static str, id: NodeId, class: &str) -> Visit {
        self.block_open(name, id, class);
        Visit::Descend(Close::Block(name))
    }

    /// Opens an element holding inline content on a line of its own.
    pub fn line(&mut self, name: &'static str, id: NodeId, class: &str) -> Visit {
        self.line_open(name, id, class);
        Visit::Descend(Close::Line(name))
    }

    /// Opens an inline element.
    pub fn inline(&mut self, name: &'static str, id: NodeId, class: &str) -> Visit {
        self.tag(name, id, class);
        Visit::Descend(Close::Tag(name))
    }

    /// Tells whether the next sibling of `id`, if any, has kind `k`.
    pub fn next_is(&self, id: NodeId, k: NodeKind) -> bool {
        let next = self.doc.subtree_end(id);
        match self.parent() {
            Some(p) if next < self.doc.subtree_end(p) => self.doc.kind(next) == k,
            _ => false,
        }
    }
}
