//! Dispatch: which part of the renderer writes each node kind. Both matches are
//! exhaustive, so a new kind does not compile until it is routed and written; a kind met
//! where its handler cannot write it fails the render (`unhandled`) rather than being
//! dropped.

use guidedoc::{AdmonitionKind, Node, NodeFlag, NodeId, NodeKind};

use crate::state::{Mark, State};

/// Where a node kind may stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// Nothing but its label.
    Silent,
    /// A block.
    Block,
    /// Written by its parent's handler, never met on its own.
    Part,
    /// Inline content.
    Inline,
}

/// Assigns every node kind its route.
pub(crate) fn route(k: NodeKind) -> Route {
    use NodeKind::*;
    match k {
        Target | Comment | SubstitutionDefinition | Toctree | Index => Route::Silent,
        Title | Subtitle | Section | Heading | Paragraph | BlockQuote | List | DefinitionList
        | FieldList | OptionList | LineBlock | CodeBlock | DoctestBlock | Preformatted
        | MathBlock | RawBlock | ThematicBreak | Table | Figure | Image | Admonition | Topic
        | Sidebar | Rubric | Container | Compound | Footnote | Citation | Contents | Directive
        | Only | Glossary | VersionModified | Desc | Hlist | ProductionList => Route::Block,
        Document | Attribution | ListItem | DefinitionItem | Term | Classifier | Definition
        | Field | FieldName | FieldBody | OptionItem | OptionGroup | Option | OptionArgument
        | OptionDescription | Line | TableHead | TableBody | Row | Cell | Caption | Legend
        | ToctreeEntry | DescSignature | DescContent | Production => Route::Part,
        Text
        | Emphasis
        | Strong
        | Code
        | Link
        | Reference
        | FootnoteReference
        | CitationReference
        | SubstitutionReference
        | InlineTarget
        | Math
        | RawInline
        | SoftBreak
        | HardBreak
        | Subscript
        | Superscript
        | TitleReference
        | Abbreviation
        | Role
        | Xref
        | DescName
        | DescAddname
        | DescAnnotation
        | DescParameterlist
        | DescParameter
        | DescOptional
        | DescReturns => Route::Inline,
    }
}

impl State<'_, '_> {
    /// Writes the block children of `parent`, one blank line apart. Children a parent
    /// writes itself (titles, subtitles, attributions) are skipped.
    pub(crate) fn render_blocks(&mut self, parent: NodeId) {
        let parent_kind = self.doc.kind(parent);
        let mut first = true;
        for child in self.doc.children(parent) {
            if !self.block_shown(parent_kind, child) {
                continue;
            }
            if !first {
                self.byte(b'\n');
            }
            first = false;
            self.render_block(child);
        }
    }

    /// Tells whether `render_blocks` writes `child` of a `parent_kind` node.
    fn block_shown(&self, parent_kind: NodeKind, child: NodeId) -> bool {
        use NodeKind::*;
        let kind = self.doc.kind(child);
        if matches!(kind, Attribution | Comment | SubstitutionDefinition | Toctree) {
            return false;
        }
        if kind == Title {
            parent_kind == Document
        } else if kind == Subtitle {
            parent_kind != Sidebar
        } else if kind == Target {
            // A label that names nothing after it keeps its ids, as a position.
            self.anchors.anchor_of(child).is_some()
        } else if kind == Directive {
            self.directive_renders(child)
        } else if kind == Footnote {
            !self.has_mark(child, Mark::Referenced) || self.quiet
        } else {
            true
        }
    }

    /// Writes one block followed by a line break.
    pub(crate) fn render_block(&mut self, id: NodeId) {
        if !self.enter(id) {
            return;
        }
        let n = *self.doc.node(id);
        self.map_node(id);
        // Sections, headings, footnotes, and citations place their own labels.
        let own_label = matches!(
            n.kind,
            NodeKind::Section | NodeKind::Heading | NodeKind::Footnote | NodeKind::Citation
        );
        if !own_label {
            self.emit_anchor(id);
        }
        if route(n.kind) == Route::Part {
            self.unhandled(id);
        } else {
            self.render_node(id, &n, true);
        }
        self.leave();
    }

    /// Writes the inline children of `parent`.
    pub(crate) fn render_inlines(&mut self, parent: NodeId) {
        for child in self.doc.children(parent) {
            self.render_inline(child);
        }
    }

    /// Writes one inline node. Calls such as `#emph[...]` are always followed by escaped
    /// text, so trailing text cannot extend the call.
    pub(crate) fn render_inline(&mut self, id: NodeId) {
        if !self.enter(id) {
            return;
        }
        let n = *self.doc.node(id);
        let inline = route(n.kind) == Route::Inline;
        if inline || matches!(n.kind, NodeKind::Image | NodeKind::Index) {
            self.render_node(id, &n, false);
        } else {
            // A block kind: validation keeps these out of inline content.
            self.unhandled(id);
        }
        self.leave();
    }

    /// Writes node `id` with its kind's handler; `block` tells whether it stands as a
    /// block (an image becomes a block image or an inline box).
    fn render_node(&mut self, id: NodeId, n: &Node, block: bool) {
        use NodeKind::*;
        match n.kind {
            Target | Comment | SubstitutionDefinition | Toctree => {}
            Index => self.emit_anchor(id),
            Title => self.wrap_inlines("#gd-title[", id, "]\n"),
            Subtitle => self.wrap_inlines("#gd-subtitle[", id, "]\n"),
            Section | Container | Compound | Only | Glossary => self.render_blocks(id),
            Heading => self.render_heading(id),
            Paragraph => self.render_paragraph(id),
            BlockQuote => self.render_quote(id),
            List => self.render_list(id),
            DefinitionList => self.render_definitions(id),
            FieldList => self.render_fields(id),
            OptionList => self.render_options(id),
            LineBlock => {
                self.render_line_block(id);
                self.byte(b'\n');
            }
            CodeBlock => self.render_code(id, n, ""),
            DoctestBlock => self.render_code(id, n, "pycon"),
            Preformatted => self.render_preformatted(id),
            MathBlock => self.render_equation(id),
            RawBlock => self.render_raw(id, true),
            ThematicBreak => self.emit("#gd-rule()\n"),
            Table => self.render_table(id),
            Figure => self.render_figure(id),
            Image => self.render_image(id, block),
            Admonition => self.render_admonition(id, n),
            Topic => self.render_topic(id),
            Sidebar => self.render_sidebar(id),
            Rubric => self.wrap_inlines("#gd-rubric[", id, "]\n"),
            Footnote => self.render_footnote_block(id),
            Citation => self.render_citation(id),
            Contents => self.render_contents(id, n),
            Directive => self.render_directive(id),
            VersionModified => self.render_version(id),
            Desc => self.render_desc(id),
            Hlist => self.render_hlist(id),
            ProductionList => self.render_productions(id),
            Document | Attribution | ListItem | DefinitionItem | Term | Classifier
            | Definition | Field | FieldName | FieldBody | OptionItem | OptionGroup | Option
            | OptionArgument | OptionDescription | Line | TableHead | TableBody | Row | Cell
            | Caption | Legend | ToctreeEntry | DescSignature | DescContent | Production => {
                self.unhandled(id)
            }
            Text => self.emit_text(self.doc.text(n.text)),
            Emphasis | TitleReference | DescAnnotation | DescParameter => {
                self.wrap_inlines("#emph[", id, "]")
            }
            Strong | DescName => self.wrap_inlines("#strong[", id, "]"),
            Subscript => self.wrap_inlines("#sub[", id, "]"),
            Superscript => self.wrap_inlines("#super[", id, "]"),
            Code => self.emit_code(id, n),
            Link | Reference => self.render_link(id),
            FootnoteReference => self.render_footnote_reference(id),
            CitationReference => self.render_citation_reference(id),
            SubstitutionReference => self.render_substitution(id),
            InlineTarget => self.render_inline_target(id),
            Math => self.render_math(id, false),
            RawInline => self.render_raw(id, false),
            SoftBreak => self.render_break(false),
            HardBreak => self.render_break(true),
            Abbreviation | Xref | DescAddname => self.render_inlines(id),
            Role => self.render_role(id),
            DescParameterlist => self.render_parameter_list(id, "\\(", "\\)"),
            DescOptional => self.render_parameter_list(id, "\\[", "\\]"),
            DescReturns => self.wrap_inlines(" → ", id, ""),
        }
    }

    /// Writes an image: as a block, or in an inline box.
    fn render_image(&mut self, id: NodeId, block: bool) {
        if block {
            self.byte(b'#');
            self.emit_image(id, true);
            self.byte(b'\n');
        } else {
            self.emit("#box(");
            self.emit_image(id, false);
            self.byte(b')');
        }
    }

    fn render_inline_target(&mut self, id: NodeId) {
        self.emit_anchor(id);
        self.render_inlines(id);
    }

    /// Writes `open`, the inline children of `id`, and `close`.
    pub(crate) fn wrap_inlines(&mut self, open: &str, id: NodeId, close: &str) {
        self.emit(open);
        self.render_inlines(id);
        self.emit(close);
    }

    /// Writes `open`, `text` as markup, and `close`.
    pub(crate) fn wrap_text(&mut self, open: &str, text: &str, close: &str) {
        self.emit(open);
        self.emit_text(text);
        self.emit(close);
    }

    /// Writes the inlines of `id`'s first child when it is a Title.
    pub(crate) fn emit_title_child(&mut self, id: NodeId) {
        if let Some(child) = self.doc.first_child(id)
            && self.doc.kind(child) == NodeKind::Title
        {
            self.render_inlines(child);
        }
    }

    /// Returns the admonition kind of `n`, generic for an unknown value.
    pub(crate) fn admonition_kind(n: &Node) -> AdmonitionKind {
        AdmonitionKind::from_value(n.value).unwrap_or(AdmonitionKind::Generic)
    }

    /// Tells whether node `id` has flag `f`.
    pub(crate) fn has_flag(&self, id: NodeId, f: NodeFlag) -> bool {
        self.doc.node(id).flags.has(f)
    }
}
