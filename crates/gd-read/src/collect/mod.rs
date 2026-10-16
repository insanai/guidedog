//! The symbols of one read document: what other documents can link to,
//! what it links to, its outline, and its words for the search index. Anchors come from
//! the table renderers build (`guidedoc::build_anchors`), so an id recorded here is the
//! id on the published page.

mod symbols;

use gd_env::{DocInfo, LabelKind, NumberedInfo, SectionInfo, SourceKind, TocEntry, ToctreeInfo};
use guidedoc::{
    AdmonitionKind, Anchors, AttributeKey, Document, NodeId, NodeKind, ROOT, RenderOptions,
    build_anchors,
};

use crate::search::SearchText;

/// Records the symbols of `doc` in `d` (a fresh `DocInfo` of the document) and returns
/// its words for the search index, as the search file's lines. A todo's `line` is left
/// as the byte offset of its start in the document's own source; the caller turns it
/// into a line once it holds the text.
pub(crate) fn collect(d: &mut DocInfo, doc: Document<'_>) -> String {
    let anchors = build_anchors(doc, &RenderOptions::default(), true);
    let mut c = Collector {
        rst: d.kind == SourceKind::Rst,
        d,
        doc,
        anchors,
        open: Vec::new(),
        descs: Vec::new(),
        onlys: Vec::new(),
        search: SearchText::default(),
    };
    for i in 0..doc.node_count() {
        c.visit(NodeId::from_index(i));
    }
    let words = c.search.lines(&c.d.sections);
    if c.d.title.is_empty()
        && let Some(first) = c.d.sections.first()
    {
        c.d.title = first.title.clone();
    }
    words
}

/// An object description the walk is inside: where it ends, the table of contents entry
/// of its last signature that has one (which nested objects go under), and its node.
#[derive(Debug, Clone, Copy)]
struct OpenDesc {
    end: NodeId,
    entry: Option<u32>,
    node: NodeId,
}

/// An `only` (or `ifconfig`) directive the walk is inside: where it ends, and the
/// expressions of it and those around it, joined with `and`.
#[derive(Debug, Clone)]
struct OpenOnly {
    end: NodeId,
    guard: String,
}

/// The walk over one document's nodes, in order, with the stacks of what encloses the
/// current node.
struct Collector<'a, 'd> {
    d: &'a mut DocInfo,
    doc: Document<'d>,
    anchors: Anchors,
    /// The document is reStructuredText (raw section titles are kept).
    rst: bool,
    /// Open sections: where each ends, and its index in `d.sections`.
    open: Vec<(NodeId, u32)>,
    descs: Vec<OpenDesc>,
    onlys: Vec<OpenOnly>,
    search: SearchText,
}

impl Collector<'_, '_> {
    fn visit(&mut self, id: NodeId) {
        self.close_before(id);
        let doc = self.doc;
        match doc.kind(id) {
            NodeKind::Title => {
                // Only the document's own title; admonitions and sidebars have titles too.
                if self.d.title.is_empty() && doc.parent_of(id) == Some(ROOT) {
                    self.d.title = self.plain(id);
                }
            }
            NodeKind::Section => self.add_section(id),
            NodeKind::Toctree => self.add_toctree(id),
            NodeKind::Target => self.add_label_from_target(id),
            NodeKind::Citation | NodeKind::InlineTarget => {
                // Sphinx's anonymous labels: every other explicit name but a footnote's.
                let name = doc.node_text(id);
                if !name.is_empty() {
                    self.add_label(name, id);
                }
            }
            NodeKind::Term => self.add_term(id),
            NodeKind::Desc => {
                let end = doc.subtree_end(id);
                self.descs.push(OpenDesc { end, entry: None, node: id });
            }
            NodeKind::Only => self.open_only(id),
            NodeKind::DescSignature => {
                self.add_object(id);
                self.add_toc_object(id);
            }
            NodeKind::Production => self.add_production(id),
            NodeKind::Index => {
                self.add_index(id);
                if doc.custom_attribute(id, "module").is_some() {
                    self.add_module(id);
                }
            }
            NodeKind::Figure => self.add_numbered(id, LabelKind::Figure),
            NodeKind::Table => self.add_numbered(id, LabelKind::Table),
            NodeKind::CodeBlock => self.add_code_block(id),
            NodeKind::MathBlock => self.add_math_block(id),
            NodeKind::Image => self.add_image(id),
            NodeKind::Xref => self.add_ref(id),
            NodeKind::FieldList => self.note_metadata(id),
            NodeKind::Directive => self.note_directive(id),
            NodeKind::Admonition => self.add_todo(id),
            NodeKind::Text | NodeKind::Code => {
                self.search.add(self.current_section(), doc.node_text(id));
            }
            _ => {}
        }
        self.add_named(id);
    }

    /// Leaves the sections, descriptions and `only` directives that end before `id`.
    fn close_before(&mut self, id: NodeId) {
        while self.open.last().is_some_and(|&(end, _)| end <= id) {
            self.open.pop();
        }
        while self.descs.last().is_some_and(|o| o.end <= id) {
            self.descs.pop();
        }
        while self.onlys.last().is_some_and(|o| o.end <= id) {
            self.onlys.pop();
        }
    }

    /// The innermost open section.
    fn current_section(&self) -> Option<u32> {
        self.open.last().map(|&(_, section)| section)
    }

    /// What the node being walked is shown under: the `only` expressions around it, or
    /// "" for always, as Sphinx's table of contents keeps its only nodes.
    fn guard(&self) -> String {
        self.onlys.last().map(|o| o.guard.clone()).unwrap_or_default()
    }

    /// A node's text without markup, on one line.
    fn plain(&self, id: NodeId) -> String {
        plain_text(self.doc, id)
    }

    /// A node's primary id; empty when it has none.
    fn anchor(&self, id: NodeId) -> String {
        self.anchors.anchor_of(id).unwrap_or("").to_owned()
    }

    /// Every id of a node, its anchor first: a figure read after `.. _fig1:` keeps its own
    /// id and takes the label's too, and :numref:`fig1` names it by either.
    fn anchor_list(&self, id: NodeId) -> Vec<String> {
        self.anchors.anchor_ids(id).map(str::to_owned).collect()
    }

    /// Whether a custom attribute is set to anything but "" or "0".
    fn flag(&self, id: NodeId, name: &str) -> bool {
        self.doc.custom_attribute(id, name).is_some_and(|v| !v.is_empty() && v != "0")
    }

    /// A custom attribute's value; empty when it is not set.
    fn custom(&self, id: NodeId, name: &str) -> String {
        self.doc.custom_attribute(id, name).unwrap_or("").to_owned()
    }

    /// The line a node starts on in its document's own source, or 0 when an included or
    /// generated file holds it; warnings about it name the line.
    fn source_line(&self, id: NodeId) -> u32 {
        let span = self.doc.node(id).span;
        let Some(entry) = self.doc.source_entry(span.source) else { return 0 };
        let text = self.doc.source_str(span.source);
        if entry.parent != span.source || text.is_empty() {
            return 0;
        }
        let (line, _) = guidedoc::line_column(text, span.start as usize);
        u32::try_from(line).unwrap_or(u32::MAX)
    }

    fn add_section(&mut self, id: NodeId) {
        let doc = self.doc;
        let heading = id + 1;
        let has_heading =
            heading.index() < doc.node_count() && doc.kind(heading) == NodeKind::Heading;
        let title = if has_heading { self.plain(heading) } else { String::new() };
        let raw_title = if self.rst && has_heading {
            doc.source_text(doc.node(heading).span).trim().to_owned()
        } else {
            String::new()
        };
        let info = SectionInfo {
            level: u32::try_from(doc.node(id).value).unwrap_or(0),
            title,
            anchor: self.section_anchor(id),
            parent: self.current_section(),
            slug: self.custom(id, "slug"),
            ids: self.section_ids(id),
            in_object: !self.descs.is_empty(),
            raw_title,
            only: self.guard(),
        };
        self.d.sections.push(info);
        let index = u32::try_from(self.d.sections.len() - 1).unwrap_or(u32::MAX);
        self.open.push((doc.subtree_end(id), index));
    }

    /// A section's id; a module declared first thing after its title gives the section
    /// its id, as Sphinx's MoveModuleTargets moves the module's target there.
    fn section_anchor(&self, id: NodeId) -> String {
        let doc = self.doc;
        let heading = id + 1;
        if heading.index() >= doc.node_count() || doc.kind(heading) != NodeKind::Heading {
            return self.anchor(id);
        }
        let next = doc.subtree_end(heading);
        if next >= doc.subtree_end(id) || doc.kind(next) != NodeKind::Index {
            return self.anchor(id);
        }
        let module = doc.custom_attribute(next, "module").is_some();
        let has_id = doc.attribute(next, AttributeKey::Id).is_some();
        if module && has_id { self.anchor(next) } else { self.anchor(id) }
    }

    /// Every id a section was read with, its anchor first (a module's, when
    /// `section_anchor` moves it there); the link pass pins them, so content an `only`
    /// drops cannot renumber them.
    fn section_ids(&self, id: NodeId) -> Vec<String> {
        let first = self.section_anchor(id);
        let rest = self.anchors.anchor_ids(id).filter(|a| *a != first).map(str::to_owned);
        let mut out: Vec<String> = Vec::new();
        if !first.is_empty() {
            out.push(first.clone());
        }
        out.extend(rest);
        out
    }

    fn add_toctree(&mut self, id: NodeId) {
        let doc = self.doc;
        let entries = doc
            .children(id)
            .map(|e| TocEntry {
                target: doc.node_text(e).to_owned(),
                title: doc.attribute(e, AttributeKey::Title).unwrap_or("").to_owned(),
            })
            .collect();
        let numbered = doc.custom_attribute(id, "numbered").and_then(|n| n.parse().ok());
        let t = ToctreeInfo {
            section: self.current_section(),
            position: count(self.d.sections.len()),
            line: self.source_line(id),
            maxdepth: u32::try_from(doc.node(id).value).unwrap_or(0),
            caption: self.custom(id, "caption"),
            hidden: self.flag(id, "hidden"),
            includehidden: self.flag(id, "includehidden"),
            titlesonly: self.flag(id, "titlesonly"),
            glob: self.flag(id, "glob"),
            reversed: self.flag(id, "reversed"),
            numbered: numbered.unwrap_or(0),
            entries,
            only: self.guard(),
        };
        self.d.toctrees.push(t);
    }

    fn open_only(&mut self, id: NodeId) {
        let doc = self.doc;
        let expression = doc
            .custom_attribute(id, "expression")
            .or_else(|| doc.custom_attribute(id, "ifconfig"))
            .unwrap_or("");
        let outer = self.guard();
        let guard = if outer.is_empty() {
            format!("({expression})")
        } else {
            format!("{outer} and ({expression})")
        };
        self.onlys.push(OpenOnly { end: doc.subtree_end(id), guard });
    }

    fn add_numbered(&mut self, id: NodeId, kind: LabelKind) {
        let info = NumberedInfo {
            kind,
            anchor: self.anchor(id),
            ids: self.anchor_list(id),
            section: self.current_section(),
            toctrees: count(self.d.toctrees.len()),
        };
        self.d.numbered.push(info);
    }

    /// A code block with a caption is numbered: a graph as a figure, else as code.
    fn add_code_block(&mut self, id: NodeId) {
        if self.doc.custom_attribute(id, "caption").is_some() {
            let kind = if self.flag(id, "graphviz") { LabelKind::Figure } else { LabelKind::Code };
            self.add_numbered(id, kind);
        }
    }

    fn add_math_block(&mut self, id: NodeId) {
        if self.doc.attribute(id, AttributeKey::Id).is_some() {
            self.add_numbered(id, LabelKind::Equation);
        }
        // The math domain's equation labels, which :eq: finds and :ref: does not.
        if let Some(label) = self.doc.custom_attribute(id, "label")
            && !label.is_empty()
        {
            self.add_label(label, id);
        }
    }

    fn note_directive(&mut self, id: NodeId) {
        match self.doc.attribute(id, AttributeKey::Name) {
            Some("todolist") => self.d.todolist = true,
            Some("meta") => self.add_meta(id),
            Some("postlist") => {
                if let Err(why) = gd_env::PostlistSpec::from_node(self.doc, id) {
                    self.d.failed = true;
                    self.d.reports.push(gd_report::Report {
                        path: self.d.relative.clone(),
                        ..gd_report::Report::error(
                            "page.postlist",
                            "INVALID POSTLIST",
                            &why.to_string(),
                        )
                        .hint("Use a non-negative item count and sort by date or -date.")
                    });
                }
            }
            _ => {}
        }
    }

    /// A todo; its `line` is the byte offset of its start until the caller has the text.
    fn add_todo(&mut self, id: NodeId) {
        let node = self.doc.node(id);
        if AdmonitionKind::from_value(node.value) == Some(AdmonitionKind::Todo) {
            let anchor = self.anchor(id);
            self.d.todos.push(gd_env::TodoInfo { node: id.0, anchor, line: node.span.start });
        }
    }

    /// Sphinx's file-wide metadata: a field list at the very start of a document
    /// (`:orphan:`, `:tocdepth:`).
    fn note_metadata(&mut self, id: NodeId) {
        let doc = self.doc;
        if doc.parent_of(id) != Some(ROOT) {
            return;
        }
        if doc.first_child(ROOT) == Some(id) {
            match gd_env::page_metadata(doc) {
                Ok(page) => self.d.page = page,
                Err(error) => {
                    self.d.failed = true;
                    let mut report = gd_report::Report {
                        path: self.d.relative.clone(),
                        ..gd_report::Report::error(
                            "page.metadata",
                            "INVALID PAGE METADATA",
                            &format!("Invalid publication field: {error}."),
                        )
                        .hint("Correct the leading metadata field and build again.")
                    };
                    let span = error.span;
                    gd_report::set_span(
                        &mut report,
                        doc.source_str(span.source),
                        span.start as usize,
                        span.end as usize,
                    );
                    self.d.reports.push(report);
                }
            }
            self.d.orphan |= !self.d.page.date.is_empty();
        }
        for field in doc.children(id) {
            let name = self.plain(field + 1).to_lowercase();
            let body = doc.subtree_end(field + 1);
            let value = self.plain(body);
            if name == "orphan" {
                self.d.orphan = true;
            }
            if name == "tocdepth" {
                self.d.tocdepth = value.parse().unwrap_or(0);
            }
        }
    }
}

/// A node's text without markup, line breaks as spaces, trimmed.
fn plain_text(doc: Document<'_>, id: NodeId) -> String {
    let mut text = String::new();
    doc.plain_text(id, &mut text);
    text.replace('\n', " ").trim().to_owned()
}

/// A length as the `u32` counts the environment keeps.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
