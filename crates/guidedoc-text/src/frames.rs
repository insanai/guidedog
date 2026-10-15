//! Framed and literal blocks: admonitions, topics, figures, notes, code, math, raw
//! content, tables of contents, directives, and the Sphinx layer's blocks.

use guidedoc::{
    AdmonitionKind, Arg, AttributeKey, ContentPolicy, Label, LinkKind, NodeFlag, NodeId, NodeKind,
    ROOT, admonition_label, arg_static, arg_text, label,
};

use crate::state::{Fill, INDENT, State, char_count, count_section};
use crate::{RAW_FORMAT, RAW_OMITTED, RAW_REFUSED};

impl State<'_, '_> {
    /// Writes "Note: text" when the admonition is one paragraph, and otherwise a
    /// "Note:" line with the content indented under it, as Sphinx does.
    pub fn render_admonition(&mut self, id: NodeId) {
        let doc = self.doc;
        let kind = AdmonitionKind::from_value(doc.node(id).value).unwrap_or_default();
        let title = doc.first_child(id).filter(|&f| doc.kind(f) == NodeKind::Title);
        let mut name = if kind == AdmonitionKind::Generic {
            match title {
                Some(t) => self.lay_out(t, "", Fill::Inlines),
                None => String::new(),
            }
        } else {
            admonition_label(self.options, kind).to_owned()
        };
        if name.is_empty() {
            name.push_str("Admonition");
        }
        name.push_str(": ");
        let body = doc.child_count(id) - usize::from(title.is_some());
        let last = doc.children(id).last();
        if let Some(last) = last.filter(|&l| body == 1 && doc.kind(l) == NodeKind::Paragraph) {
            self.push_frame(name.chars().count(), &name);
            self.render_node(last, crate::blocks::Context::Block);
            self.pop_frame();
            return;
        }
        self.block_start();
        self.line_begin();
        self.put(name.trim_end_matches(' '));
        self.line_end();
        self.block_end();
        self.push_frame(INDENT, "");
        self.render_blocks(id);
        self.pop_frame();
    }

    /// Writes a topic's or sidebar's title, then its blocks.
    pub fn render_topic(&mut self, id: NodeId) {
        if let Some(title) =
            self.doc.first_child(id).filter(|&t| self.doc.kind(t) == NodeKind::Title)
        {
            self.paragraph(title, "", Fill::FrameTitle);
        }
        self.render_blocks(id);
    }

    /// Writes a topic or sidebar title, and a sidebar's subtitle after " -- ".
    pub fn frame_title(&mut self, title: NodeId) {
        let doc = self.doc;
        self.render_inlines(title);
        let next = doc.subtree_end(title);
        if next.index() < doc.node_count() && doc.kind(next) == NodeKind::Subtitle {
            self.put(" -- ");
            self.render_inlines(next);
        }
    }

    pub fn rubric_text(&mut self, id: NodeId) {
        self.put("-[ ");
        self.render_inlines(id);
        self.put(" ]-");
    }

    /// Writes the image, the caption with its number, and the legend.
    pub fn render_figure(&mut self, id: NodeId) {
        let doc = self.doc;
        let mut lead = self.custom(id, "number-label").to_owned();
        if !lead.is_empty() {
            lead.push(' ');
        }
        for part in doc.children(id) {
            let kind = doc.kind(part);
            if kind == NodeKind::Image {
                self.paragraph(part, "", Fill::Image);
            } else if matches!(kind, NodeKind::Link | NodeKind::Reference) {
                if let Some(image) = doc.first_child(part) {
                    self.paragraph(image, "", Fill::Image);
                }
            } else if kind == NodeKind::Caption {
                self.paragraph(part, &lead, Fill::Inlines);
            } else if kind == NodeKind::Legend {
                self.render_blocks(part);
            }
        }
    }

    /// Writes a footnote or citation as "[label] text" with a hanging indent.
    pub fn render_note(&mut self, id: NodeId) {
        let mut marker = self.lay_out(id, "[", Fill::NoteLabel);
        marker.push_str("] ");
        self.push_frame(marker.chars().count(), &marker);
        self.render_blocks(id);
        self.pop_frame();
        self.recycle(marker);
    }

    /// Writes a literal block indented three spaces; a caption comes first.
    pub fn render_code_block(&mut self, id: NodeId) {
        let caption = self.custom(id, "caption");
        if !caption.is_empty() {
            self.block_start();
            self.line_begin();
            let number = self.custom(id, "number-label");
            if !number.is_empty() {
                self.put(number);
                self.put_char(' ');
            }
            match self.doc.code_caption(id) {
                Some(parsed) => self.render_inlines(parsed),
                None => self.put(caption),
            }
            self.line_end();
            self.block_end();
        }
        self.block_start();
        self.push_frame(INDENT, "");
        self.write_lines(self.doc.node_text(id));
        self.pop_frame();
        self.block_end();
    }

    pub fn render_preformatted(&mut self, id: NodeId) {
        let text = self.lay_out(id, "", Fill::Inlines);
        self.block_start();
        self.push_frame(INDENT, "");
        self.write_lines(&text);
        self.pop_frame();
        self.block_end();
        self.recycle(text);
    }

    /// Writes display math as its source, with the equation number after it.
    pub fn render_math(&mut self, id: NodeId) {
        self.block_start();
        self.push_frame(INDENT, "");
        self.write_lines(self.doc.node_text(id));
        let number = self.custom(id, "number");
        if !number.is_empty() {
            self.line_begin();
            self.put_char('(');
            self.put(number);
            self.put_char(')');
            self.line_end();
        }
        self.pop_frame();
        self.block_end();
    }

    /// Applies the raw policy: only raw text can appear in text output.
    pub fn raw_allowed(&mut self, id: NodeId) -> bool {
        let format = self.doc.attribute(id, AttributeKey::Format).unwrap_or("");
        if !format.eq_ignore_ascii_case("text") {
            let name = arg_text(if format.is_empty() { "untyped" } else { format });
            let m =
                if self.policy.raw == ContentPolicy::Refuse { RAW_REFUSED } else { RAW_FORMAT };
            self.fail(m, id, name, Arg::None);
            return false;
        }
        match self.policy.raw {
            ContentPolicy::Allow => return true,
            ContentPolicy::Omit => self.fail(RAW_OMITTED, id, arg_static("text"), Arg::None),
            ContentPolicy::Refuse => self.fail(RAW_REFUSED, id, arg_static("text"), Arg::None),
        }
        false
    }

    pub fn render_raw_block(&mut self, id: NodeId) {
        if !self.raw_allowed(id) {
            return;
        }
        self.block_start();
        self.write_lines(self.doc.node_text(id));
        self.block_end();
    }

    /// Lists the section titles, nested by level, as a bulleted outline.
    pub fn render_contents(&mut self, id: NodeId) {
        let doc = self.doc;
        let title = doc
            .attribute(id, AttributeKey::Title)
            .unwrap_or_else(|| label(self.options, Label::Contents));
        if !title.is_empty() {
            self.block_start();
            self.write_wrapped(title);
            self.block_end();
        }
        let (first, end, base) = contents_scope(self, id);
        let limit = usize::try_from(doc.node(id).value).unwrap_or(0);
        let mut counters = [0i64; 16];
        let mut ends: Vec<NodeId> = Vec::new();
        self.block_start();
        for i in 1..doc.node_count() {
            let section = NodeId::from_index(i);
            while ends.last().is_some_and(|&e| e <= section) {
                ends.pop();
            }
            if doc.kind(section) != NodeKind::Section {
                continue;
            }
            ends.push(doc.subtree_end(section));
            let depth = ends.len();
            count_section(&mut counters, depth, self.numbers.start);
            let d = depth.saturating_sub(base);
            if section < first || section >= end || (limit > 0 && d > limit) {
                continue;
            }
            if section + 1 >= end || doc.kind(section + 1) != NodeKind::Heading {
                continue;
            }
            self.contents_entry(section, d, &counters[..depth.min(counters.len())]);
        }
        self.block_end();
    }

    fn contents_entry(&mut self, section: NodeId, d: usize, counters: &[i64]) {
        self.line_begin();
        for _ in 1..d {
            self.put("  ");
        }
        self.put("* ");
        let number = self.custom(section, "secnumber");
        if !number.is_empty() {
            self.put(number);
            self.put(". ");
        } else if self.numbers.numbered(counters.len()) {
            let mut lead = String::new();
            self.numbers.write(&mut lead, counters);
            self.put(&lead);
        }
        let text = self.lay_out(section + 1, "", Fill::Inlines);
        self.put(&text.replace('\n', " "));
        self.line_end();
        self.recycle(text);
    }

    /// Writes the directives that carry content.
    pub fn render_directive(&mut self, id: NodeId) {
        let name = self.directive_name(id);
        match name {
            "sectnum" | "section-numbering" | "meta" | "title" => return,
            "target-notes" => return self.render_target_notes(),
            "sectionauthor" | "moduleauthor" | "codeauthor" => {
                return self.render_author(id, name);
            }
            _ => {}
        }
        if let Some(title) =
            self.doc.first_child(id).filter(|&t| self.doc.kind(t) == NodeKind::Title)
        {
            self.paragraph(title, "", Fill::Rubric);
        }
        self.render_blocks(id);
    }

    fn render_author(&mut self, id: NodeId, name: &str) {
        if self.custom(id, "show") != "1" {
            return;
        }
        let l = match name {
            "moduleauthor" => Label::ModuleAuthor,
            "codeauthor" => Label::CodeAuthor,
            _ => Label::SectionAuthor,
        };
        self.block_start();
        self.line_begin();
        self.put(label(self.options, l));
        self.put(self.custom(id, "author"));
        self.line_end();
        self.block_end();
    }

    /// Lists every named external target with its URL.
    fn render_target_notes(&mut self) {
        let doc = self.doc;
        self.block_start();
        for i in 0..doc.node_count() {
            let t = NodeId::from_index(i);
            if doc.kind(t) != NodeKind::Target {
                continue;
            }
            let Some(l) = doc.link_info(t) else {
                continue;
            };
            if l.kind != LinkKind::External || l.destination.is_empty() {
                continue;
            }
            self.line_begin();
            self.put("* ");
            let name = doc.node_text(t);
            if !name.is_empty() {
                self.put(name);
                self.put(": ");
            }
            self.put(doc.text(l.destination));
            self.line_end();
        }
        self.block_end();
    }

    /// Writes each signature on its own line and the description under it.
    pub fn render_desc(&mut self, id: NodeId) {
        for child in self.doc.children(id) {
            let kind = self.doc.kind(child);
            if kind == NodeKind::DescSignature {
                self.paragraph(child, "", Fill::Inlines);
                self.blank = false;
            } else if kind == NodeKind::DescContent {
                self.blank = true;
                self.push_frame(INDENT, "");
                self.render_blocks(child);
                self.pop_frame();
            }
        }
        self.block_end();
    }

    /// Writes a grammar: tokens padded to the widest, then " ::= " and the definition;
    /// continuation lines are indented to the definitions.
    pub fn render_productions(&mut self, id: NodeId) {
        let doc = self.doc;
        let width = doc.children(id).map(|p| char_count(doc.node_text(p))).max().unwrap_or(0);
        self.block_start();
        self.push_frame(INDENT, "");
        for p in doc.children(id) {
            let token = doc.node_text(p);
            self.line_begin();
            self.put(token);
            self.put_repeat(' ', width - char_count(token));
            self.put(if token.is_empty() { "     " } else { " ::= " });
            let text = self.lay_out(p, "", Fill::Inlines);
            self.put(&text.replace('\n', " "));
            self.line_end();
            self.recycle(text);
        }
        self.pop_frame();
        self.block_end();
    }
}

/// The nodes a table of contents lists and the depth of the sections it starts in: the
/// whole document, or, for a local one, the section holding it.
fn contents_scope(s: &State<'_, '_>, id: NodeId) -> (NodeId, NodeId, usize) {
    let doc = s.doc;
    let (mut first, mut end) = (ROOT + 1, doc.subtree_end(ROOT));
    let mut base = 0;
    if doc.node(id).flags.has(NodeFlag::Local) {
        let mut p = doc.parent_of(id);
        while let Some(parent) = p {
            if doc.kind(parent) == NodeKind::Section {
                if first == ROOT + 1 {
                    (first, end) = (parent + 1, doc.subtree_end(parent));
                }
                base += 1;
            }
            p = doc.parent_of(parent);
        }
    }
    (first, end, base)
}
