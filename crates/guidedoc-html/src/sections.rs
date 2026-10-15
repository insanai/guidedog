//! Sections, headings, titles, section numbers, and tables of contents.

use guidedoc::{AttributeKey, Label, NodeId, NodeKind, contents_scope, label};

use crate::state::{Html, count_section};
use crate::walk::{Close, Visit};

/// The element and class of a title, by its parent's kind; others are `<p class="title">`.
const TITLE_TAGS: [(NodeKind, &str, &str); 6] = [
    (NodeKind::Document, "h1", "title"),
    (NodeKind::Admonition, "p", "admonition-title"),
    (NodeKind::Topic, "p", "topic-title"),
    (NodeKind::Sidebar, "p", "sidebar-title"),
    (NodeKind::Table, "caption", ""),
    (NodeKind::Directive, "p", "directive-title"),
];

fn title_tag(parent: NodeKind) -> (&'static str, &'static str) {
    TITLE_TAGS.iter().find(|t| t.0 == parent).map_or(("p", "title"), |t| (t.1, t.2))
}

/// Where a section title links back to: a table of contents itself, or the
/// section's entry in it.
#[derive(Debug, Clone, Copy)]
enum Backref {
    Top(NodeId),
    Entry(NodeId),
}

/// The open levels of a table of contents being written.
#[derive(Debug, Default)]
struct Toc {
    level: usize,
}

impl Html<'_, '_> {
    /// Counts the sections among the open ancestors of the current walk.
    fn section_depth(&self) -> usize {
        self.open_ancestors().filter(|&id| self.doc.kind(id) == NodeKind::Section).count()
    }

    fn heading_level(&self, id: NodeId) -> i64 {
        let mut level = i64::from(self.doc.node(id).value);
        if self.parent_kind(self.parent()) == NodeKind::Section {
            level = self.section_depth() as i64;
        }
        if self.has_title {
            level += 1;
        }
        level.clamp(1, 6)
    }

    pub fn enter_section(&mut self, id: NodeId) -> Visit {
        let depth = self.section_depth() + 1;
        count_section(&mut self.numbers.counters, depth, self.numbers.start);
        self.block("section", id, "")
    }

    pub fn enter_heading(&mut self, id: NodeId) -> Visit {
        let section = self.parent().filter(|&p| self.doc.kind(p) == NodeKind::Section);
        self.w.cr();
        self.w.put("<h");
        self.w.put_int(self.heading_level(id));
        if section.is_none() {
            self.write_id_attribute(id);
        }
        self.write_classes(id, "");
        self.w.put_byte(b'>');
        let Some(section) = section else {
            self.write_aliases(id);
            return Visit::Descend(Close::Heading);
        };
        self.toc_backref_open(section, id); // around the number too.
        let secnumber = self.custom(section, "secnumber");
        let d = self.section_depth();
        if !secnumber.is_empty() {
            self.w.put("<span class=\"section-number\">");
            self.w.esc(secnumber);
            let suffix = self.options.secnumber_suffix;
            self.w.esc(if suffix.is_empty() { ". " } else { suffix });
            self.w.put("</span>");
        } else if self.numbers.numbered(d) {
            let counters = self.numbers.counters;
            self.write_number(&counters[..d]);
        }
        Visit::Descend(Close::Heading)
    }

    pub fn leave_heading(&mut self, id: NodeId) {
        let section = self.parent().filter(|&p| self.doc.kind(p) == NodeKind::Section);
        if let Some(section) = section
            && self.toc_backref(section, id).is_some()
        {
            self.w.put("</a>");
        }
        self.headerlink(Some(section.unwrap_or(id)), label(self.options, Label::LinkHeading));
        self.w.put("</h");
        self.w.put_int(self.heading_level(id));
        self.w.put_byte(b'>');
        self.w.cr();
    }

    pub fn enter_title(&mut self, id: NodeId) -> Visit {
        let parent = self.parent();
        let (name, class) = title_tag(self.parent_kind(parent));
        self.line_open(name, id, class);
        if self.parent_kind(parent) == NodeKind::Table {
            self.caption_open(parent);
        }
        Visit::Descend(Close::Title(name))
    }

    pub fn leave_title(&mut self, name: &str) {
        let parent = self.parent();
        let table = self.parent_kind(parent) == NodeKind::Table;
        if table {
            self.caption_close(parent, label(self.options, Label::LinkTable));
        }
        self.line_close(name);
        if let Some(table) = parent.filter(|_| table) {
            self.write_colgroup(table);
        }
    }

    pub fn enter_subtitle(&mut self, id: NodeId) -> Visit {
        let sidebar = self.parent_kind(self.parent()) == NodeKind::Sidebar;
        self.line("p", id, if sidebar { "sidebar-subtitle" } else { "subtitle" })
    }

    /// Writes a section number such as "Part 3.1" in its span.
    fn write_number(&mut self, counters: &[i64]) {
        self.w.put("<span class=\"section-number\">");
        self.w.esc(self.numbers.prefix);
        for (i, &c) in counters.iter().enumerate() {
            if i > 0 {
                self.w.put_byte(b'.');
            }
            self.w.put_int(c);
        }
        self.w.esc(self.numbers.suffix);
        self.w.put("</span> ");
    }

    /// Writes the escaped text of a subtree, as titles and tables of contents use it,
    /// where nested markup would be invalid.
    pub fn write_plain(&mut self, id: NodeId) {
        let doc = self.doc;
        for i in id.0 + 1..doc.subtree_end(id).0 {
            let kind = doc.kind(NodeId(i));
            if matches!(kind, NodeKind::Text | NodeKind::Code | NodeKind::Math) {
                self.w.esc(doc.node_text(NodeId(i)));
            } else if matches!(kind, NodeKind::SoftBreak | NodeKind::HardBreak) {
                self.w.put_byte(b' ');
            }
        }
    }

    pub fn enter_contents(&mut self, id: NodeId) -> Visit {
        let doc = self.doc;
        let (first, end, base) = contents_scope(doc, id);
        self.block_open("nav", id, "contents");
        let title = doc
            .attribute(id, AttributeKey::Title)
            .unwrap_or_else(|| label(self.options, Label::Contents));
        if !title.is_empty() {
            self.w.put("<p class=\"contents-title\">");
            self.w.esc(title);
            self.w.put("</p>");
            self.w.cr();
        }
        let limit = usize::try_from(doc.node(id).value).unwrap_or(0);
        if doc.nodes().iter().any(|n| n.kind == NodeKind::Section) {
            self.toc_sections(id, (first, end), base, limit);
        } else {
            self.toc_headings(first, end, limit);
        }
        self.block_close("nav");
        Visit::Skip
    }

    fn toc_sections(
        &mut self,
        contents: NodeId,
        scope: (NodeId, NodeId),
        base: usize,
        limit: usize,
    ) {
        let doc = self.doc;
        let mut counters = [0i64; 16];
        let mut ends: Vec<NodeId> = Vec::new();
        let mut toc = Toc::default();
        for i in 1..doc.node_count() {
            let id = NodeId::from_index(i);
            while ends.last().is_some_and(|&end| end <= id) {
                ends.pop();
            }
            if doc.kind(id) != NodeKind::Section {
                continue;
            }
            ends.push(doc.subtree_end(id));
            let depth = ends.len();
            count_section(&mut counters, depth, self.numbers.start);
            let d = depth.saturating_sub(base);
            if id < scope.0 || id >= scope.1 || (limit > 0 && d > limit) {
                continue;
            }
            let title = id + 1;
            if title >= doc.subtree_end(id) || doc.kind(title) != NodeKind::Heading {
                continue;
            }
            let shown = if self.numbers.numbered(depth) { depth.min(counters.len()) } else { 0 };
            self.toc_entry(&mut toc, d, (id, title), &counters[..shown], Some(contents));
        }
        self.toc_finish(&mut toc);
    }

    fn toc_headings(&mut self, first: NodeId, end: NodeId, limit: usize) {
        let doc = self.doc;
        let headings = (first.0..end.0).map(NodeId).filter(|&i| doc.kind(i) == NodeKind::Heading);
        let top = headings.clone().map(|i| doc.node(i).value).min().unwrap_or(7);
        let mut toc = Toc::default();
        for i in headings {
            let d = usize::try_from(doc.node(i).value - top + 1).unwrap_or(1);
            if limit > 0 && d > limit {
                continue;
            }
            self.toc_entry(&mut toc, d, (i, i), &[], None);
        }
        self.toc_finish(&mut toc);
    }

    fn toc_entry(
        &mut self,
        toc: &mut Toc,
        depth: usize,
        (target, title): (NodeId, NodeId),
        number: &[i64],
        contents: Option<NodeId>,
    ) {
        let d = depth.min(toc.level + 1);
        if d > toc.level {
            self.w.cr();
            self.w.put("<ul>");
            self.w.cr();
            toc.level = d;
        } else {
            self.w.put("</li>");
            self.w.cr();
            while toc.level > d {
                self.w.put("</ul>\n</li>");
                self.w.cr();
                toc.level -= 1;
            }
        }
        self.w.put("<li><a");
        // A table of contents' entry has the id Docutils gives it.
        if let Some(entry) = contents.and_then(|c| self.anchors.extra_anchor(c, target)) {
            self.w.put(" id=\"");
            self.w.esc(entry);
            self.w.put_byte(b'"');
        }
        if let Some(a) = self.anchors.anchor_of(target) {
            self.w.put(" href=\"#");
            self.w.esc(a);
            self.w.put_byte(b'"');
        }
        self.w.put_byte(b'>');
        if !number.is_empty() {
            self.write_number(number);
        }
        self.write_plain(title);
        self.w.put("</a>");
    }

    fn toc_finish(&mut self, toc: &mut Toc) {
        if toc.level == 0 {
            return;
        }
        self.w.put("</li>");
        self.w.cr();
        while toc.level > 1 {
            self.w.put("</ul>\n</li>");
            self.w.cr();
            toc.level -= 1;
        }
        self.w.put("</ul>");
        self.w.cr();
    }

    /// Where a section's title links back to, as Docutils' Contents transform sets its
    /// refid: the last table of contents listing the section, with backlinks "entry"
    /// (the default) its entry, with "top" the table itself. A title holding a
    /// reference gets none.
    fn toc_backref(&self, section: NodeId, title: NodeId) -> Option<Backref> {
        let doc = self.doc;
        let (first, last) = self.contents.filter(|_| !self.w.reference)?;
        let linked = (title.0 + 1..doc.subtree_end(title).0)
            .any(|i| matches!(doc.kind(NodeId(i)), NodeKind::Link | NodeKind::Reference));
        if linked {
            return None;
        }
        let tables = (first.0..=last.0).rev().map(NodeId);
        for c in tables.filter(|&c| doc.kind(c) == NodeKind::Contents) {
            if self.anchors.extra_anchor(c, section).is_none() {
                continue;
            }
            return match self.custom(c, "backlinks") {
                "none" => None,
                "top" => Some(Backref::Top(c)),
                _ => Some(Backref::Entry(c)),
            };
        }
        None
    }

    fn toc_backref_open(&mut self, section: NodeId, title: NodeId) {
        let target = match self.toc_backref(section, title) {
            None => return,
            Some(Backref::Top(c)) => self.anchors.anchor_of(c),
            Some(Backref::Entry(c)) => self.anchors.extra_anchor(c, section),
        };
        self.w.put("<a class=\"toc-backref\" href=\"#");
        self.w.esc(target.unwrap_or(""));
        self.w.put("\" role=\"doc-backlink\">");
    }
}
