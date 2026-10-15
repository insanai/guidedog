//! Block handlers: headings, quotes, framed blocks, code, equations, raw content,
//! tables of contents, and generic directives.

use guidedoc::{
    AdmonitionKind, AttributeKey, ContentPolicy, Node, NodeFlag, NodeId, NodeKind,
    admonition_label, admonition_name, arg_text,
};

use crate::escape::is_label_safe;
use crate::messages::{RAW_FOREIGN, RAW_FOREIGN_OMITTED, RAW_OMITTED, RAW_REFUSED};
use crate::state::{Mark, State};

impl State<'_, '_> {
    /// Writes a heading; a section's heading carries the section's label, so references
    /// to the section and to its title land on the same element.
    pub(crate) fn render_heading(&mut self, id: NodeId) {
        let level = self.doc.node(id).value.max(1);
        let section = self.section_of_heading(id);
        let secnumber = section.map_or("", |s| self.custom(s, "secnumber"));
        self.emit("#heading(level: ");
        self.int(i64::from(level));
        if !secnumber.is_empty() {
            // The host's number is the heading's numbering, so a template can set it
            // apart from the title and the outline lists it.
            self.emit(", numbering: gd-secnum(");
            self.emit_string(secnumber);
            self.byte(b')');
        }
        self.emit(")[");
        self.copied = true;
        self.render_inlines(id);
        self.copied = false;
        self.byte(b']');
        let target = self.label_target(id);
        if !self.emit_label(target) && target != id {
            self.emit_label(id);
        }
        self.byte(b'\n');
    }

    /// Returns the Section a Heading opens: the node just before it.
    fn section_of_heading(&self, heading: NodeId) -> Option<NodeId> {
        let before = heading.0.checked_sub(1).map(NodeId)?;
        (self.doc.kind(before) == NodeKind::Section).then_some(before)
    }

    /// Maps a node to the node whose label represents it in Typst: a section's heading
    /// stands for the section, and a heading stands for the targets inside it, which
    /// cannot carry labels of their own.
    pub(crate) fn label_target(&self, id: NodeId) -> NodeId {
        let mut target = id;
        if self.has_mark(id, Mark::InHeading)
            && let Some(heading) =
                (0..id.0).rev().map(NodeId).find(|&i| self.doc.kind(i) == NodeKind::Heading)
        {
            target = heading;
        }
        if self.doc.kind(target) == NodeKind::Heading {
            return self.section_of_heading(target).unwrap_or(target);
        }
        target
    }

    pub(crate) fn render_quote(&mut self, id: NodeId) {
        self.emit("#quote(block: true");
        if let Some(attribution) =
            self.doc.children(id).find(|&c| self.doc.kind(c) == NodeKind::Attribution)
        {
            self.wrap_inlines(", attribution: [", attribution, "]");
        }
        self.emit(")[\n");
        self.render_blocks(id);
        self.emit("]\n");
    }

    pub(crate) fn render_admonition(&mut self, id: NodeId, n: &Node) {
        let kind = Self::admonition_kind(n);
        self.emit("#gd-admonition(\"");
        self.emit(admonition_name(kind));
        self.emit("\", [");
        if kind == AdmonitionKind::Generic {
            self.emit_title_child(id);
        } else {
            self.emit_text(admonition_label(self.options, kind));
            let loud = matches!(
                kind,
                AdmonitionKind::Attention | AdmonitionKind::Caution | AdmonitionKind::Danger
            );
            if loud {
                self.emit("!");
            }
        }
        self.emit("])[\n");
        self.render_blocks(id);
        self.emit("]\n");
    }

    pub(crate) fn render_topic(&mut self, id: NodeId) {
        self.emit("#gd-topic([");
        self.emit_title_child(id);
        self.emit("])[\n");
        self.render_blocks(id);
        self.emit("]\n");
    }

    pub(crate) fn render_sidebar(&mut self, id: NodeId) {
        self.emit("#gd-sidebar([");
        self.emit_title_child(id);
        self.emit("], ");
        match self.doc.children(id).find(|&c| self.doc.kind(c) == NodeKind::Subtitle) {
            Some(subtitle) => self.wrap_inlines("[", subtitle, "]"),
            None => self.emit("none"),
        }
        self.emit(")[\n");
        self.render_blocks(id);
        self.emit("]\n");
    }

    /// Writes a literal block as a raw block. Numbered or emphasized lines use
    /// gd-numbered; a caption makes it a listing figure with the caption on top.
    pub(crate) fn render_code(&mut self, id: NodeId, n: &Node, language: &str) {
        let doc = self.doc;
        let lang = doc.attribute(id, AttributeKey::Language).unwrap_or(language);
        let code = doc.text(n.text);
        let caption = self.custom(id, "caption");
        let marked = self.custom(id, "emphasize-lines");
        let numbered = n.value > 0 || self.custom(id, "linenos") == "1";
        if caption.is_empty() {
            self.byte(b'#');
        } else {
            self.emit("#figure(kind: raw, numbering: none, supplement: none, ");
            self.emit("caption: figure.caption(position: top, [");
            self.emit_caption_number(id);
            match doc.code_caption(id) {
                Some(parsed) => self.render_inlines(parsed),
                None => self.emit_text(caption),
            }
            self.emit("]), ");
        }
        let lines = numbered || !marked.is_empty();
        if lines {
            self.emit("gd-numbered(");
            if numbered {
                self.int(i64::from(n.value.max(1)));
            } else {
                self.emit("none");
            }
            self.emit(", ");
        }
        self.emit("raw(block: true, ");
        if !lang.is_empty() {
            self.emit("lang: ");
            self.emit_string(lang);
            self.emit(", ");
        }
        self.emit_string(code.strip_suffix('\n').unwrap_or(code));
        self.byte(b')');
        if !marked.is_empty() {
            self.emit(", marked: ");
            self.emit_line_list(marked, code);
        }
        if lines {
            self.byte(b')');
        }
        if !caption.is_empty() {
            self.byte(b')');
        }
        self.byte(b'\n');
    }

    /// Writes display math; a numbered equation shows the host's number.
    pub(crate) fn render_equation(&mut self, id: NodeId) {
        let number = self.custom(id, "number");
        if number.is_empty() {
            self.render_math(id, true);
            return;
        }
        self.emit("#gd-equation(");
        self.emit_string(number);
        self.emit(")[\n");
        self.render_math(id, true);
        self.emit("]\n");
    }

    pub(crate) fn render_preformatted(&mut self, id: NodeId) {
        self.emit("#gd-pre[");
        let saved = self.pre;
        self.pre = true;
        self.column = 0;
        self.render_inlines(id);
        self.pre = saved;
        self.emit("]\n");
    }

    /// Writes a line break; in preformatted text a soft break is kept too.
    pub(crate) fn render_break(&mut self, hard: bool) {
        if hard || self.pre {
            self.emit("\\\n");
            self.column = 0;
        } else {
            self.byte(b'\n');
        }
    }

    /// Writes raw Typst only under the allow policy; raw content for any other format
    /// is never written.
    pub(crate) fn render_raw(&mut self, id: NodeId, block: bool) {
        let doc = self.doc;
        let n = doc.node(id);
        let format = doc.attribute(id, AttributeKey::Format).unwrap_or("");
        let policy = self.ctx.policy.raw;
        if !format.eq_ignore_ascii_case("typst") {
            let m =
                if policy == ContentPolicy::Refuse { RAW_FOREIGN } else { RAW_FOREIGN_OMITTED };
            self.fail(m, n.span, arg_text(format));
            return;
        }
        match policy {
            ContentPolicy::Refuse => self.fail(RAW_REFUSED, n.span, guidedoc::Arg::None),
            ContentPolicy::Omit => self.fail(RAW_OMITTED, n.span, guidedoc::Arg::None),
            ContentPolicy::Allow => {
                let text = doc.text(n.text);
                self.emit(text);
                if block && !text.ends_with('\n') {
                    self.byte(b'\n');
                }
            }
        }
    }

    /// Writes an outline. A local table of contents lists the headings between the
    /// enclosing section's heading and the next section that is not nested in it, and
    /// counts its depth from the enclosing section's level.
    pub(crate) fn render_contents(&mut self, id: NodeId, n: &Node) {
        self.emit("#outline(title: ");
        let section = if n.flags.has(NodeFlag::Local) { self.enclosing_section(id) } else { None };
        match (self.doc.attribute(id, AttributeKey::Title), section) {
            (Some(title), _) => self.wrap_text("[", title, "]"),
            (None, Some(_)) => self.emit("none"),
            (None, None) => self.emit("auto"),
        }
        let mut base = 0;
        if let Some(section) = section
            && !self.quiet
        {
            base = self.doc.node(section).value;
            self.emit_local_target(section);
        }
        if n.value > 0 {
            self.emit(", depth: ");
            self.int(i64::from(base) + i64::from(n.value));
        }
        self.emit(")\n");
    }

    fn enclosing_section(&self, id: NodeId) -> Option<NodeId> {
        std::iter::successors(self.doc.parent_of(id), |&p| self.doc.parent_of(p))
            .find(|&p| self.doc.kind(p) == NodeKind::Section)
    }

    fn emit_local_target(&mut self, section: NodeId) {
        let doc = self.doc;
        let Some(name) = self.anchors.anchor_of(section).filter(|n| is_label_safe(n)) else {
            return;
        };
        let out = &mut self.ctx.out;
        out.emit(", target: selector(heading).after(<");
        out.emit(name);
        out.emit(">, inclusive: false)");
        let next = (doc.subtree_end(section).0..doc.node_count() as u32)
            .map(NodeId)
            .find(|&i| doc.kind(i) == NodeKind::Section);
        if let Some(next) =
            next.and_then(|i| self.anchors.anchor_of(i)).filter(|n| is_label_safe(n))
        {
            out.emit(".before(<");
            out.emit(next);
            out.emit(">, inclusive: false)");
        }
    }

    /// Tells whether a directive has content where it stands.
    pub(crate) fn directive_renders(&self, id: NodeId) -> bool {
        let name = self.name_of(id);
        if matches!(name, "title" | "meta" | "sectnum" | "section-numbering" | "header" | "footer")
        {
            return false;
        }
        if self.author_label(name).is_some() {
            return self.custom(id, "show") == "1";
        }
        true
    }

    /// Writes generic directives. Document-level ones (title, meta, sectnum, header,
    /// footer) were applied in the settings.
    pub(crate) fn render_directive(&mut self, id: NodeId) {
        if !self.directive_renders(id) {
            return;
        }
        let name = self.name_of(id);
        if name == "target-notes" {
            self.render_target_notes();
            return;
        }
        if let Some(label) = self.author_label(name) {
            self.render_author(id, label);
            return;
        }
        if let Some(child) = self.doc.first_child(id)
            && self.doc.kind(child) == NodeKind::Title
        {
            self.wrap_inlines("#gd-rubric[", child, "]\n");
            if self.doc.subtree_end(child) < self.doc.subtree_end(id) {
                self.byte(b'\n');
            }
        }
        self.render_blocks(id);
    }
}
