//! The link pass: what one page knows while its document is rewritten for
//! one output, and the rule that decides each node.

use std::cell::OnceCell;

use gd_conf::Config;
use gd_env::{Asset, DocId, DocInfo, LabelKind, OutputFormat, Trace};
use gd_graphdog::Context;
use gd_host::Ledger;
use gd_report::Report;
use gd_resolve::BuildView;
use guidedoc::{
    AdmonitionKind, AttributeKey, Builder, Document, Fail, NodeId, NodeKind, PassOutcome, ROOT,
    SourceSpan, Workspace,
};

use crate::only::{Tags, evaluate_only};
use crate::rewrite::{Action, Rewriter, Rule, rewrite_document};
use crate::{discussion, extlinks, graph, image, nav, todolist, xref};

/// What a node of the document being linked is, counted in the order reading counted
/// them, to find its section, number, or toctree in the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Indexed {
    /// Nothing counted.
    None,
    /// The document's n-th section.
    Section(u32),
    /// The n-th numbered item (figure, table, captioned code, labelled equation).
    Numbered(u32),
    /// The n-th C++ declaration's signature.
    CppDecl(u32),
    /// The n-th toctree.
    Toctree(u32),
}

/// One page being linked: the build's view, the document, the output, and what the
/// pass gathers (assets, lookups, reports).
pub(crate) struct Page<'p> {
    /// What the build knows.
    pub view: BuildView<'p>,
    /// The document linked.
    pub id: DocId,
    /// What reading learned about it.
    pub info: &'p DocInfo,
    /// The output it is linked for.
    pub format: OutputFormat,
    /// PDF: the document's toctree depth below the book's root.
    pub depth: u32,
    /// A second workspace, for the documents a todolist copies from.
    pub spare: Option<&'p mut Workspace>,
    /// The worker's Graphviz context, started on the first graph.
    pub graphs: &'p OnceCell<Option<Context>>,
    /// The files and drawings the page holds of the build's budget.
    pub ledger: &'p Ledger<'p>,
    /// Images, downloads and graphs the page needs (in the book: the files it reads).
    pub assets: Vec<Asset>,
    /// What its references looked up.
    pub trace: Trace,
    /// What linking reported.
    pub reports: Vec<Report>,
    index: Vec<Indexed>,
}

/// A warning's fixed parts; `Page::warn` adds the document and the location.
pub(crate) struct Warning<'a> {
    pub code: &'a str,
    pub title: &'a str,
    pub category: &'a str,
    pub message: String,
    pub hint: &'a str,
}

impl<'p> Page<'p> {
    /// A page of document `id` to link for `format`.
    pub fn new(
        view: BuildView<'p>,
        id: DocId,
        format: OutputFormat,
        depth: u32,
        spare: &'p mut Workspace,
        graphs: &'p OnceCell<Option<Context>>,
        ledger: &'p Ledger<'p>,
    ) -> Page<'p> {
        Page {
            view,
            id,
            info: view.env.doc(id),
            format,
            depth,
            spare: Some(spare),
            graphs,
            ledger,
            assets: Vec::new(),
            trace: Trace::new(),
            reports: Vec::new(),
            index: Vec::new(),
        }
    }

    /// The link pass's body: index the document, then rewrite it into `b`.
    pub fn link(&mut self, doc: Document<'_>, b: &mut Builder<'_>) -> PassOutcome {
        self.index_nodes(doc);
        rewrite_document(doc, b, self)
    }

    /// The build's configuration.
    pub fn config(&self) -> &'p Config {
        self.view.config
    }

    /// The tags `only` expressions on this page see.
    pub fn tags(&self) -> Tags<'p> {
        Tags { kind: self.view.kind, format: self.format, tags: &self.view.options.tags }
    }

    /// What node `id` was counted as.
    pub fn indexed(&self, id: NodeId) -> Indexed {
        self.index.get(id.index()).copied().unwrap_or(Indexed::None)
    }

    /// Counts sections, numbered items, C++ declarations and toctrees in document
    /// order, as collecting counted them.
    fn index_nodes(&mut self, doc: Document<'_>) {
        self.index.clear();
        let (mut sections, mut numbered, mut decls, mut trees) = (0, 0, 0, 0);
        let next = |n: &mut u32| {
            *n += 1;
            *n - 1
        };
        for i in 0..doc.node_count() {
            let id = NodeId::from_index(i);
            let slot = match doc.kind(id) {
                NodeKind::DescSignature if doc.custom_attribute(id, "cpp:path").is_some() => {
                    Indexed::CppDecl(next(&mut decls))
                }
                NodeKind::Section => Indexed::Section(next(&mut sections)),
                NodeKind::Figure | NodeKind::Table => Indexed::Numbered(next(&mut numbered)),
                NodeKind::CodeBlock if doc.custom_attribute(id, "caption").is_some() => {
                    Indexed::Numbered(next(&mut numbered))
                }
                NodeKind::MathBlock if doc.attribute(id, AttributeKey::Id).is_some() => {
                    Indexed::Numbered(next(&mut numbered))
                }
                NodeKind::Toctree => Indexed::Toctree(next(&mut trees)),
                _ => Indexed::None,
            };
            self.index.push(slot);
        }
    }

    /// A section's number, or "".
    fn secnumber(&self, section: u32) -> &'p str {
        let numbers = self.view.tables.secnumbers.get(self.id.index());
        numbers.and_then(|n| n.get(section as usize)).map_or("", String::as_str)
    }

    /// A numbered item's number, or "".
    fn fignumber(&self, item: u32) -> &'p str {
        let numbers = self.view.tables.fignumbers.get(self.id.index());
        numbers.and_then(|n| n.get(item as usize)).map_or("", String::as_str)
    }

    /// Adds a warning about this document at `span` of `doc`.
    pub fn warn(&mut self, doc: Document<'_>, span: SourceSpan, w: Warning<'_>) {
        let mut report = Report {
            path: self.info.relative.clone(),
            category: w.category.to_string(),
            ..Report::warning(w.code, w.title, &w.message).hint(w.hint)
        };
        locate(doc, span, &mut report);
        self.reports.push(report);
    }
}

/// Points a report at `span`, in the file it was read from (an included file's own),
/// with that line shown.
pub(crate) fn locate(doc: Document<'_>, span: SourceSpan, report: &mut Report) {
    let text = doc.source_str(span.source);
    if text.is_empty() || span.start as usize > text.len() {
        return;
    }
    if let Some(entry) = doc.source_entry(span.source)
        && entry.parent != span.source
        && !entry.name.is_empty()
    {
        report.path = gd_host::display_path(&entry.name);
    }
    gd_report::set_span(report, text, span.start as usize, span.end as usize);
}

/// Whether a Custom attribute is set: present, not empty, not "0".
pub(crate) fn flag(doc: Document<'_>, id: NodeId, name: &str) -> bool {
    doc.custom_attribute(id, name).is_some_and(|v| !v.is_empty() && v != "0")
}

impl Rule for Page<'_> {
    fn visits(&self, doc: Document<'_>, id: NodeId) -> bool {
        match doc.kind(id) {
            NodeKind::Link | NodeKind::Reference => self.config().extlinks_detect_hardcoded_links,
            NodeKind::FieldName
            | NodeKind::FieldBody
            | NodeKind::Toctree
            | NodeKind::Only
            | NodeKind::Xref
            | NodeKind::CodeBlock
            | NodeKind::Image
            | NodeKind::Title
            | NodeKind::Subtitle
            | NodeKind::Heading
            | NodeKind::Admonition
            | NodeKind::Directive
            | NodeKind::Section
            | NodeKind::Figure
            | NodeKind::Table
            | NodeKind::MathBlock
            | NodeKind::DescSignature => true,
            _ => false,
        }
    }

    fn decide(&mut self, doc: Document<'_>, id: NodeId) -> Action {
        let format = self.format;
        match doc.kind(id) {
            NodeKind::FieldName | NodeKind::FieldBody => discussion::decide(doc, id),
            NodeKind::Toctree => {
                let hidden = doc.custom_attribute(id, "hidden") == Some("1");
                if format != OutputFormat::Html || hidden { Action::Drop } else { Action::Replace }
            }
            NodeKind::Only => {
                let expression = doc
                    .custom_attribute(id, "expression")
                    .or_else(|| doc.custom_attribute(id, "ifconfig"))
                    .unwrap_or("");
                if evaluate_only(expression, &self.tags()) { Action::Unwrap } else { Action::Drop }
            }
            NodeKind::Xref => Action::Replace,
            NodeKind::Link | NodeKind::Reference => {
                extlinks::check_hardcoded_link(self, doc, id);
                Action::Copy
            }
            NodeKind::CodeBlock
                if format != OutputFormat::Text && graph::is_graph(self, doc, id) =>
            {
                Action::Replace
            }
            NodeKind::Image if format != OutputFormat::Text => Action::Replace,
            NodeKind::Title => self.decide_title(doc, id),
            NodeKind::Subtitle if format == OutputFormat::Pdf => Action::Drop,
            NodeKind::Heading if self.is_book_title(doc, id) => Action::Drop,
            NodeKind::Admonition => {
                let todo = doc.node(id).value == AdmonitionKind::Todo as i32;
                if todo && !self.config().todo_include_todos { Action::Drop } else { Action::Copy }
            }
            NodeKind::Directive if doc.attribute(id, AttributeKey::Name) == Some("todolist") => {
                if self.config().todo_include_todos {
                    Action::Replace
                } else {
                    Action::Drop
                }
            }
            _ => Action::Copy,
        }
    }

    fn replace(&mut self, rw: &mut Rewriter<'_, '_, '_>, id: NodeId) -> Result<(), Fail> {
        match rw.doc.kind(id) {
            NodeKind::FieldName | NodeKind::FieldBody => discussion::replace_field(self, rw, id),
            NodeKind::Toctree => nav::replace_toctree(self, rw, id),
            NodeKind::Xref => xref::replace_xref(self, rw, id),
            NodeKind::Image => image::replace_image(self, rw, id),
            NodeKind::Title => self.replace_title(rw, id),
            NodeKind::CodeBlock => graph::replace_graph(self, rw, id),
            NodeKind::Directive => todolist::replace_todolist(self, rw, id),
            // decide replaces nothing else; keep the node should that change.
            _ => rw.copy_node(self, id),
        }
    }

    fn decorate(
        &mut self,
        rw: &mut Rewriter<'_, '_, '_>,
        source: NodeId,
        copy: NodeId,
    ) -> Result<(), Fail> {
        match self.indexed(source) {
            Indexed::Section(s) => self.decorate_section(rw, s, copy)?,
            Indexed::CppDecl(n) => {
                if self.view.tables.cpp.redeclared.contains(&(self.id, n)) {
                    rw.add_custom(copy, "nopermalink", "1")?;
                }
            }
            Indexed::Numbered(n) => self.decorate_number(rw, n, copy)?,
            Indexed::Toctree(_) | Indexed::None => {}
        }
        let doc = rw.doc;
        match doc.kind(source) {
            NodeKind::Section | NodeKind::Heading if self.format == OutputFormat::Pdf => {
                // A document's title section is level 1; in the book it is a chapter at
                // the document's depth below the root.
                let depth = i32::try_from(self.depth.max(1)).unwrap_or(i32::MAX);
                let level = doc.node(source).value.saturating_add(depth - 1).max(1);
                rw.b.set_value(copy, level);
            }
            NodeKind::Directive if self.config().show_authors => {
                let name = doc.attribute(source, AttributeKey::Name).unwrap_or("");
                if matches!(name, "sectionauthor" | "moduleauthor" | "codeauthor") {
                    rw.add_custom(copy, "show", "1")?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Page<'_> {
    /// Only the document title becomes a chapter in the book; admonition titles stay.
    fn decide_title(&self, doc: Document<'_>, id: NodeId) -> Action {
        if self.format != OutputFormat::Pdf || doc.parent_of(id) != Some(ROOT) {
            return Action::Copy;
        }
        if self.depth == 0 { Action::Drop } else { Action::Replace }
    }

    /// The root document's title is the book's title, not a chapter.
    fn is_book_title(&self, doc: Document<'_>, id: NodeId) -> bool {
        if self.format != OutputFormat::Pdf || self.depth != 0 {
            return false;
        }
        doc.parent_of(id).is_some_and(|parent| {
            doc.kind(parent) == NodeKind::Section && self.indexed(parent) == Indexed::Section(0)
        })
    }

    /// Makes a document's title a chapter heading at its depth in the book.
    fn replace_title(&mut self, rw: &mut Rewriter<'_, '_, '_>, id: NodeId) -> Result<(), Fail> {
        let heading = rw.open(NodeKind::Heading, rw.doc.node(id).span)?;
        rw.moved_to(id, heading);
        rw.b.set_value(heading, i32::try_from(self.depth).unwrap_or(i32::MAX));
        rw.rewrite_children(self, id)?;
        rw.close(heading)
    }

    /// A section's number, and the ids it was read with, which other pages link to:
    /// Sphinx gives ids before `only` drops content, so a dropped section still takes
    /// its "idN". The "pinned-ids" mark tells the anchor table these are all of them.
    fn decorate_section(
        &mut self,
        rw: &mut Rewriter<'_, '_, '_>,
        section: u32,
        copy: NodeId,
    ) -> Result<(), Fail> {
        let number = self.secnumber(section);
        if !number.is_empty() {
            rw.add_custom(copy, "secnumber", number)?;
        }
        let ids = self.info.sections.get(section as usize).map_or(&[][..], |s| &s.ids[..]);
        if ids.is_empty() {
            return Ok(());
        }
        for id in ids {
            rw.b.add_attribute_string(copy, AttributeKey::Id, id)?;
        }
        rw.add_custom(copy, "pinned-ids", "1")
    }

    /// A numbered item's number, and its caption's label ("Fig. 3").
    fn decorate_number(
        &mut self,
        rw: &mut Rewriter<'_, '_, '_>,
        item: u32,
        copy: NodeId,
    ) -> Result<(), Fail> {
        let number = self.fignumber(item);
        if number.is_empty() {
            return Ok(());
        }
        rw.add_custom(copy, "number", number)?;
        let kind = self.info.numbered.get(item as usize).map_or(LabelKind::Other, |n| n.kind);
        if kind != LabelKind::Equation {
            rw.add_custom(copy, "number-label", &number_label(&self.view, kind, number))?;
        }
        Ok(())
    }
}

/// Formats a number with `numfig_format`, as "Fig. %s" does.
fn number_label(view: &BuildView<'_>, kind: LabelKind, number: &str) -> String {
    view.number_template(kind).replace("%s", number)
}
