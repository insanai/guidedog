//! The dispatch over node kinds, and the plain blocks: titles, sections, paragraphs,
//! lists, definition, field, and option lists, and line blocks.

use guidedoc::{
    Arg, CORE_UNHANDLED_KIND, Label, ListInfo, ListStyle, NodeFlag, NodeId, NodeKind, arg_static,
    kind_name, label, split_label,
};

use crate::state::{Fill, INDENT, State, WIDTH, count_section};

/// Underlines titles by level, as Sphinx's text_sectionchars.
const SECTION_CHARS: [char; 7] = ['*', '=', '-', '~', '"', '+', '`'];

/// Where a node is met: among blocks, or in inline content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    Block,
    Inline,
}

/// The version notes and their labels.
const VERSION_LABELS: [(&str, Label); 4] = [
    ("versionadded", Label::VersionAdded),
    ("versionchanged", Label::VersionChanged),
    ("deprecated", Label::VersionDeprecated),
    ("versionremoved", Label::VersionRemoved),
];

impl State<'_, '_> {
    /// Writes the block children of `parent`. Children a parent writes itself (the
    /// titles of frames) are skipped here.
    pub fn render_blocks(&mut self, parent: NodeId) {
        let parent_kind = self.doc.kind(parent);
        for child in self.doc.children(parent) {
            let kind = self.doc.kind(child);
            let skip = (kind == NodeKind::Title && parent_kind != NodeKind::Document)
                || (kind == NodeKind::Subtitle && parent_kind == NodeKind::Sidebar);
            if !skip {
                self.render_node(child, Context::Block);
            }
        }
    }

    /// Writes node `id`. Every node kind is routed here; a new kind does not compile
    /// until it is. Parts of structures, which their parent writes, are reported as a
    /// renderer bug if met on their own.
    #[rustfmt::skip]
    pub fn render_node(&mut self, id: NodeId, context: Context) {
        use NodeKind as K;
        if !self.enter(id) {
            return;
        }
        match self.doc.kind(id) {
            K::Target | K::Comment | K::SubstitutionDefinition | K::Toctree | K::Index => {}
            K::Title => self.title_line(id, SECTION_CHARS[0], ""),
            K::Subtitle => self.title_line(id, SECTION_CHARS[1], ""),
            K::Section => self.render_section(id),
            K::Heading => self.render_heading(id),
            K::Paragraph => self.render_paragraph(id),
            K::BlockQuote => self.indented(id),
            K::Attribution => self.paragraph(id, "-- ", Fill::Inlines),
            K::List => self.render_list(id),
            K::DefinitionList => self.render_definitions(id),
            K::FieldList | K::OptionList => self.render_fields(id),
            K::LineBlock => self.render_line_block(id),
            K::CodeBlock | K::DoctestBlock => self.render_code_block(id),
            K::Preformatted => self.render_preformatted(id),
            K::MathBlock => self.render_math(id),
            K::RawBlock => self.render_raw_block(id),
            K::ThematicBreak => self.render_rule(),
            K::Table => self.render_table(id),
            K::Image if context == Context::Block => self.paragraph(id, "", Fill::Image),
            K::Image => self.image_text(id),
            K::Figure => self.render_figure(id),
            K::Admonition => self.render_admonition(id),
            K::Topic | K::Sidebar => self.render_topic(id),
            K::Rubric => self.paragraph(id, "", Fill::Rubric),
            K::Container | K::Compound | K::Legend | K::Only | K::Glossary | K::DescContent => {
                self.render_blocks(id);
            }
            K::Footnote | K::Citation => self.render_note(id),
            K::Contents => self.render_contents(id),
            K::Directive => self.render_directive(id),
            K::VersionModified => self.render_version(id),
            K::Desc => self.render_desc(id),
            K::Hlist => {
                if let Some(list) = self.doc.first_child(id) {
                    self.render_node(list, Context::Block);
                }
            }
            K::ProductionList => self.render_productions(id),
            K::Caption => self.paragraph(id, "", Fill::Inlines),
            K::Document | K::ListItem | K::DefinitionItem | K::Term | K::Classifier
            | K::Definition | K::Field | K::FieldName | K::FieldBody | K::OptionItem
            | K::OptionGroup | K::Option | K::OptionArgument | K::OptionDescription | K::Line
            | K::TableHead | K::TableBody | K::Row | K::Cell | K::ToctreeEntry
            | K::DescSignature | K::Production => self.unhandled(id),
            // Inline content.
            K::Text | K::Math => self.write_text(self.doc.node_text(id)),
            K::Emphasis | K::TitleReference => self.wrap("*", id, "*"),
            K::Strong => self.wrap("**", id, "**"),
            K::Subscript => self.wrap("_", id, ""),
            K::Superscript => self.wrap("^", id, ""),
            K::Code => self.render_code(id),
            K::RawInline => self.render_raw_inline(id),
            K::Link | K::Reference => self.render_link(id),
            K::FootnoteReference | K::CitationReference => self.render_note_reference(id),
            K::SubstitutionReference => self.render_substitution(id),
            K::SoftBreak => self.put_char(' '),
            K::HardBreak => self.put_char('\n'),
            K::Abbreviation => self.render_abbreviation(id),
            K::Role => self.render_role(id),
            K::DescParameterlist => self.parameters_in("(", id, ")"),
            K::DescOptional => self.parameters_in("[", id, "]"),
            K::DescReturns => self.wrap(" -> ", id, ""),
            K::InlineTarget | K::Xref | K::DescName | K::DescAddname | K::DescAnnotation
            | K::DescParameter => self.render_inlines(id),
        }
        self.leave();
    }

    /// Reports a part of a structure met on its own: a renderer bug.
    fn unhandled(&mut self, id: NodeId) {
        let kind = arg_static(kind_name(self.doc.kind(id)));
        self.fail(CORE_UNHANDLED_KIND, id, kind, Arg::None);
    }

    fn render_rule(&mut self) {
        self.block_start();
        self.line_begin();
        let width = self.width.saturating_sub(self.indentation()).clamp(1, WIDTH);
        self.put_repeat('=', width);
        self.line_end();
        self.block_end();
    }

    fn render_section(&mut self, id: NodeId) {
        self.sections += 1;
        count_section(&mut self.numbers.counters, self.sections, self.numbers.start);
        self.render_blocks(id);
        self.sections -= 1;
    }

    /// Writes a paragraph; the first paragraph of a version note starts with "Added in
    /// version X: ".
    fn render_paragraph(&mut self, id: NodeId) {
        let mut lead = String::new();
        let before = id - 1;
        if id.0 > 0 && self.doc.kind(before) == NodeKind::VersionModified {
            self.version_label(before, &mut lead, true);
        }
        self.paragraph(id, &lead, Fill::Inlines);
    }

    fn version_label(&self, id: NodeId, out: &mut String, explained: bool) {
        let kind = self.custom(id, "type");
        let l = VERSION_LABELS.iter().find(|v| v.0 == kind).map_or(Label::VersionChanged, |v| v.1);
        let (before, after) = split_label(label(self.options, l));
        out.push_str(before);
        out.push_str(self.custom(id, "version"));
        out.push_str(after);
        out.push_str(if explained { ": " } else { "." });
    }

    pub fn render_version(&mut self, id: NodeId) {
        let explained =
            self.doc.first_child(id).is_some_and(|f| self.doc.kind(f) == NodeKind::Paragraph);
        if !explained {
            let mut lead = String::new();
            self.version_label(id, &mut lead, false);
            self.block_start();
            self.write_wrapped(&lead);
            self.block_end();
        }
        self.render_blocks(id);
    }

    /// Writes a title underlined with `c` over its whole width.
    fn title_line(&mut self, id: NodeId, c: char, lead: &str) {
        let text = self.lay_out(id, lead, Fill::Inlines);
        self.block_start();
        self.line_begin();
        let line = text.replace('\n', " ");
        self.put(&line);
        self.line_end();
        self.line_begin();
        self.put_repeat(c, line.chars().count().max(1));
        self.line_end();
        self.block_end();
        self.recycle(text);
    }

    /// Writes a section title, numbered by the host or by sectnum.
    fn render_heading(&mut self, id: NodeId) {
        let doc = self.doc;
        let section = (id.0 > 0).then(|| id - 1).filter(|&p| doc.kind(p) == NodeKind::Section);
        let mut level = match section {
            Some(_) => self.sections,
            None => usize::try_from(doc.node(id).value).unwrap_or(0),
        };
        if self.has_title {
            level += 1;
        }
        let c = SECTION_CHARS[level.saturating_sub(1).min(SECTION_CHARS.len() - 1)];
        let Some(section) = section else {
            self.title_line(id, c, "");
            return;
        };
        let mut lead = String::new();
        let secnumber = self.custom(section, "secnumber");
        if !secnumber.is_empty() {
            lead.push_str(secnumber);
            let suffix = self.options.secnumber_suffix;
            lead.push_str(if suffix.is_empty() { ". " } else { suffix });
        } else if self.numbers.numbered(self.sections) {
            self.numbers.write(&mut lead, &self.numbers.counters[..self.sections]);
        }
        self.title_line(id, c, &lead);
    }

    /// Writes a block quote's blocks three spaces in.
    fn indented(&mut self, id: NodeId) {
        self.push_frame(INDENT, "");
        self.render_blocks(id);
        self.pop_frame();
    }

    /// Writes "* " or "1. " before each item, and indents the item under it. Items of
    /// a tight list follow each other without blank lines.
    fn render_list(&mut self, id: NodeId) {
        let info = self.doc.list_info(id);
        let tight = self.doc.node(id).flags.has(NodeFlag::Tight);
        let mut marker = String::new();
        for (number, item) in (i64::from(info.start)..).zip(self.doc.children(id)) {
            marker.clear();
            write_marker(&mut marker, info, number);
            self.push_frame(marker.chars().count(), &marker);
            self.render_blocks(item);
            self.pop_frame();
            if tight {
                self.blank = false;
            }
        }
        self.block_end();
    }

    /// Writes each term on its own line, classifiers after " : ", and the definition
    /// indented under it.
    fn render_definitions(&mut self, id: NodeId) {
        let doc = self.doc;
        for item in doc.children(id) {
            for part in doc.children(item) {
                if doc.kind(part) == NodeKind::Term {
                    self.paragraph(part, "", Fill::Term);
                    self.blank = false;
                } else if doc.kind(part) == NodeKind::Definition {
                    self.push_frame(INDENT, "");
                    self.render_blocks(part);
                    self.pop_frame();
                    self.block_end();
                }
            }
        }
    }

    /// Writes a term and the classifiers that follow it.
    pub fn term_text(&mut self, term: NodeId) {
        self.render_inlines(term);
        let doc = self.doc;
        let mut next = doc.subtree_end(term);
        while next.index() < doc.node_count() && doc.kind(next) == NodeKind::Classifier {
            self.put(" : ");
            self.render_inlines(next);
            next = doc.subtree_end(next);
        }
    }

    /// Writes field and option lists: the name or options on a line, and the body
    /// indented under it.
    fn render_fields(&mut self, id: NodeId) {
        let doc = self.doc;
        for item in doc.children(id) {
            for part in doc.children(item) {
                let kind = doc.kind(part);
                if kind == NodeKind::FieldName {
                    self.paragraph(part, "", Fill::FieldName);
                } else if kind == NodeKind::OptionGroup {
                    self.paragraph(part, "", Fill::OptionGroup);
                } else if matches!(kind, NodeKind::FieldBody | NodeKind::OptionDescription) {
                    self.blank = false;
                    self.push_frame(INDENT, "");
                    self.render_blocks(part);
                    self.pop_frame();
                    self.block_end();
                }
            }
        }
    }

    pub fn field_name(&mut self, id: NodeId) {
        self.render_inlines(id);
        self.put_char(':');
    }

    pub fn option_group(&mut self, group: NodeId) {
        let doc = self.doc;
        for (i, option) in doc.children(group).enumerate() {
            if i > 0 {
                self.put(", ");
            }
            self.put(doc.node_text(option));
            for argument in doc.children(option) {
                self.put_char(if doc.node(argument).value == i32::from(b'=') { '=' } else { ' ' });
                self.put(doc.node_text(argument));
            }
        }
    }

    /// Writes each line as it stands; nested line blocks are indented.
    fn render_line_block(&mut self, id: NodeId) {
        self.block_start();
        for child in self.doc.children(id) {
            if self.doc.kind(child) == NodeKind::LineBlock {
                self.push_frame(INDENT, "");
                self.render_line_block(child);
                self.pop_frame();
                self.blank = false;
                continue;
            }
            let text = self.lay_out(child, "", Fill::Inlines);
            if text.is_empty() {
                self.line_end();
            } else {
                self.write_wrapped(&text);
            }
            self.recycle(text);
        }
        self.block_end();
    }
}

/// Writes a list marker such as "* ", "3. ", "c) ", or "(iv) ".
fn write_marker(out: &mut String, info: ListInfo, number: i64) {
    if info.style == ListStyle::Bullet {
        out.push_str("* ");
        return;
    }
    if info.delimiter == b'(' {
        out.push('(');
    }
    match info.style {
        ListStyle::Bullet => {}
        ListStyle::Arabic => out.push_str(&number.to_string()),
        ListStyle::LowerAlpha | ListStyle::UpperAlpha => {
            let base = if info.style == ListStyle::LowerAlpha { b'a' } else { b'A' };
            let offset = u8::try_from((number - 1).max(0) % 26).unwrap_or(0);
            out.push(char::from(base + offset));
        }
        ListStyle::LowerRoman => write_roman(out, number, false),
        ListStyle::UpperRoman => write_roman(out, number, true),
    }
    let closing = matches!(info.delimiter, b')' | b'(');
    out.push_str(if closing { ") " } else { ". " });
}

fn write_roman(out: &mut String, number: i64, upper: bool) {
    const VALUES: [(i64, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut n = number.clamp(1, 3999);
    for (value, numeral) in VALUES {
        while n >= value {
            if upper {
                out.push_str(&numeral.to_ascii_uppercase());
            } else {
                out.push_str(numeral);
            }
            n -= value;
        }
    }
}
