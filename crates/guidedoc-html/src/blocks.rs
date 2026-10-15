//! Block elements: paragraphs, lists, definition and option lists, line blocks, display
//! math, raw blocks, figures, admonitions, and directives.

use guidedoc::{
    AdmonitionKind, ContentPolicy, Label, ListStyle, NodeFlag, NodeId, NodeKind, admonition_label,
    arg_static, label,
};

use crate::state::Html;
use crate::walk::{Close, Visit};
use crate::{RAW_FORMAT, RAW_OMITTED, RAW_REFUSED};

/// The classes of each admonition kind, as Docutils and Sphinx write them.
fn admonition_class(kind: AdmonitionKind) -> &'static str {
    match kind {
        AdmonitionKind::Generic => "admonition",
        AdmonitionKind::Attention => "admonition attention",
        AdmonitionKind::Caution => "admonition caution",
        AdmonitionKind::Danger => "admonition danger",
        AdmonitionKind::Error => "admonition error",
        AdmonitionKind::Hint => "admonition hint",
        AdmonitionKind::Important => "admonition important",
        AdmonitionKind::Note => "admonition note",
        AdmonitionKind::Tip => "admonition tip",
        AdmonitionKind::Warning => "admonition warning",
        AdmonitionKind::Seealso => "admonition seealso",
        AdmonitionKind::Todo => "admonition todo admonition-todo",
    }
}

/// How a paragraph renders: plainly, or as one of Sphinx's forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParagraphStyle {
    Plain,
    /// No `<p>`, in tight lists and single-block cells.
    Compact,
    /// The centered directive: `<p class="centered"><strong>`.
    Centered,
    /// A toctree caption: `<p class="caption" role="heading">`.
    Caption,
}

impl Html<'_, '_> {
    /// Tells whether a paragraph renders without `<p>`: inside a tight list's item, or
    /// as the only block of a table cell.
    fn compact(&self) -> bool {
        let Some(p) = self.parent() else {
            return false;
        };
        let kind = self.doc.kind(p);
        if kind == NodeKind::ListItem {
            return self.ancestor(2).is_some_and(|g| {
                self.doc.kind(g) == NodeKind::List && self.doc.node(g).flags.has(NodeFlag::Tight)
            });
        }
        kind == NodeKind::Cell && !self.w.reference && self.doc.child_count(p) == 1
    }

    fn paragraph_style(&self, id: NodeId) -> ParagraphStyle {
        if self.compact() {
            return ParagraphStyle::Compact;
        }
        if self.w.reference {
            return ParagraphStyle::Plain;
        }
        if self.doc.has_class(id, "centered") {
            return ParagraphStyle::Centered;
        }
        let toctree = self.parent().is_some_and(|p| {
            self.doc.kind(p) == NodeKind::Container && self.doc.has_class(p, "toctree-wrapper")
        });
        if toctree && self.doc.has_class(id, "caption") {
            return ParagraphStyle::Caption;
        }
        ParagraphStyle::Plain
    }

    pub fn enter_paragraph(&mut self, id: NodeId) -> Visit {
        if let Some(visit) = self.enter_object_field_paragraph(id) {
            return visit;
        }
        let style = self.paragraph_style(id);
        match style {
            ParagraphStyle::Compact => return Visit::Descend(Close::Paragraph(style)),
            ParagraphStyle::Plain => self.line_open("p", id, ""),
            ParagraphStyle::Centered => {
                self.line_open("p", id, "");
                self.w.put("<strong>");
            }
            ParagraphStyle::Caption => {
                self.w.cr();
                self.open_tag("p", id, "");
                self.w.put(" role=\"heading\">");
                self.write_aliases(id);
                self.w.put("<span class=\"caption-text\">");
            }
        }
        if let Some(p) = self.parent()
            && p + 1 == id
            && self.doc.kind(p) == NodeKind::VersionModified
        {
            self.version_label(p, true);
        }
        Visit::Descend(Close::Paragraph(style))
    }

    pub fn leave_paragraph(&mut self, style: ParagraphStyle) {
        match style {
            ParagraphStyle::Compact => return,
            ParagraphStyle::Plain => {}
            ParagraphStyle::Centered => self.w.put("</strong>"),
            ParagraphStyle::Caption => self.w.put("</span>"),
        }
        self.line_close("p");
    }

    pub fn enter_attribution(&mut self, id: NodeId) -> Visit {
        self.line_open("p", id, "attribution");
        self.w.put("— ");
        Visit::Descend(Close::Line("p"))
    }

    pub fn enter_list(&mut self, id: NodeId) -> Visit {
        if let Some(visit) = self.enter_object_field_list(id) {
            return visit;
        }
        self.open_list(id, 0, true);
        Visit::Descend(Close::Block(list_tag(self.doc.list_info(id).style)))
    }

    /// Writes `<ul>` or `<ol>`; `offset` skips items already shown, as in the later
    /// columns of an hlist, and ids go only on the first part of a list.
    pub fn open_list(&mut self, id: NodeId, offset: i64, with_id: bool) {
        let info = self.doc.list_info(id);
        self.w.cr();
        if info.style == ListStyle::Bullet {
            self.open_tag_with("ul", id, "", with_id);
        } else {
            self.open_tag_with("ol", id, "", with_id);
            let start = i64::from(info.start) + offset;
            if start != 1 {
                self.w.put(" start=\"");
                self.w.put_int(start);
                self.w.put_byte(b'"');
            }
            match info.style {
                ListStyle::LowerAlpha => self.w.put(" type=\"a\""),
                ListStyle::UpperAlpha => self.w.put(" type=\"A\""),
                ListStyle::LowerRoman => self.w.put(" type=\"i\""),
                ListStyle::UpperRoman => self.w.put(" type=\"I\""),
                ListStyle::Bullet | ListStyle::Arabic => {}
            }
        }
        self.w.put_byte(b'>');
        self.w.cr();
    }

    pub fn enter_list_item(&mut self, id: NodeId) -> Visit {
        self.tag("li", id, "");
        if self.in_object_field_list() {
            self.w.continue_line();
        }
        Visit::Descend(Close::Line("li"))
    }

    pub fn enter_definition_list(&mut self, id: NodeId) -> Visit {
        let glossary = self.parent_kind(self.parent()) == NodeKind::Glossary;
        self.block("dl", id, if glossary { "simple glossary" } else { "" })
    }

    /// Opens a term. The first term of an item also carries the item's ids (a label
    /// before the item names it), as Docutils writes them on the `<dt>`.
    pub fn enter_term(&mut self, id: NodeId) -> Visit {
        let item = self.parent();
        let first = item.filter(|&i| self.doc.kind(i) == NodeKind::DefinitionItem && i + 1 == id);
        let labelled = first.is_some_and(|i| self.has_anchor(i));
        match first {
            Some(item) if labelled && !self.has_anchor(id) => self.line_open("dt", item, ""),
            _ => {
                self.line_open("dt", id, "");
                if let Some(item) = first.filter(|_| labelled) {
                    self.write_all_ids(item);
                }
            }
        }
        if self.next_is(id, NodeKind::Classifier) {
            return Visit::Descend(Close::Nothing);
        }
        // A glossary term has a permalink, as Sphinx's writer gives it (not with
        // classifiers).
        Visit::Descend(Close::Dt { permalink: self.in_glossary(id) })
    }

    pub fn leave_dt(&mut self, id: NodeId, permalink: bool) {
        if permalink {
            self.headerlink(Some(id), label(self.options, Label::LinkTerm));
        }
        self.line_close("dt");
    }

    pub fn enter_classifier(&mut self, id: NodeId) -> Visit {
        self.w.put(" <span class=\"classifier-delimiter\">:</span> ");
        self.tag("span", id, "classifier");
        Visit::Descend(Close::Classifier { last: !self.next_is(id, NodeKind::Classifier) })
    }

    /// Opens a definition, field body, or option description.
    pub fn enter_dd(&mut self, id: NodeId) -> Visit {
        self.w.cr();
        self.tag("dd", id, "");
        Visit::Descend(Close::Line("dd"))
    }

    pub fn enter_option(&mut self, id: NodeId) -> Visit {
        let before = self.doc.kind(id - 1);
        if before == NodeKind::Option || before == NodeKind::OptionArgument {
            self.w.put(", ");
        }
        self.open_tag("kbd", id, "option");
        self.w.put_byte(b'>');
        self.write_aliases(id);
        self.w.esc(self.doc.node_text(id));
        for child in self.doc.children(id) {
            self.enter_option_argument(child);
        }
        self.w.close("kbd");
        Visit::Skip
    }

    pub fn enter_option_argument(&mut self, id: NodeId) -> Visit {
        let delimiter = self.doc.node(id).value;
        self.w.put_byte(if delimiter == i32::from(b'=') { b'=' } else { b' ' });
        self.w.put("<var>");
        self.w.esc(self.doc.node_text(id));
        self.w.put("</var>");
        Visit::Skip
    }

    pub fn enter_line(&mut self, id: NodeId) -> Visit {
        self.line_open("div", id, "line");
        if !self.doc.has_children(id) {
            self.w.put("<br />");
        }
        Visit::Descend(Close::Line("div"))
    }

    pub fn enter_doctest(&mut self, id: NodeId) -> Visit {
        self.line_open("pre", id, "doctest");
        self.w.esc(self.doc.node_text(id));
        self.line_close("pre");
        Visit::Skip
    }

    pub fn enter_thematic_break(&mut self, id: NodeId) -> Visit {
        self.w.cr();
        self.open_tag("hr", id, "");
        self.w.put(" />");
        self.w.cr();
        Visit::Skip
    }

    /// Applies the raw policy: only raw HTML can appear in HTML output.
    fn raw_allowed(&mut self, id: NodeId) -> bool {
        let format = self.doc.attribute(id, guidedoc::AttributeKey::Format).unwrap_or("");
        let span = self.doc.node(id).span;
        let name = guidedoc::arg_text(if format.is_empty() { "untyped" } else { format });
        let none = guidedoc::Arg::None;
        if !format.eq_ignore_ascii_case("html") {
            let m =
                if self.policy.raw == ContentPolicy::Refuse { RAW_REFUSED } else { RAW_FORMAT };
            self.w.fail(m, span, name, none);
            return false;
        }
        match self.policy.raw {
            ContentPolicy::Allow => return true,
            ContentPolicy::Omit => self.w.fail(RAW_OMITTED, span, arg_static("html"), none),
            ContentPolicy::Refuse => self.w.fail(RAW_REFUSED, span, arg_static("html"), none),
        }
        false
    }

    pub fn enter_raw(&mut self, id: NodeId, block: bool) -> Visit {
        if !self.raw_allowed(id) {
            return Visit::Skip;
        }
        let s = self.doc.node_text(id);
        if !block {
            self.w.put(s);
            return Visit::Skip;
        }
        self.w.cr();
        self.w.put(s.strip_suffix('\n').unwrap_or(s));
        self.w.cr();
        Visit::Skip
    }

    /// Opens a figure; a width sizes it, and figwidth "image" fits its image.
    pub fn enter_figure(&mut self, id: NodeId) -> Visit {
        use guidedoc::AttributeKey;
        self.w.cr();
        self.open_tag("figure", id, "");
        if let Some(align) = self.doc.attribute(id, AttributeKey::Align) {
            self.w.put(" data-align=\"");
            self.w.esc(align);
            self.w.put_byte(b'"');
        }
        let width = self.doc.attribute(id, AttributeKey::Width).unwrap_or("");
        if self.custom(id, "figwidth") == "image" {
            self.w.put(" style=\"width: fit-content\"");
        } else if !width.is_empty() {
            self.w.put(" style=\"width: ");
            self.w.esc(width);
            let numeric = width.bytes().all(|c| c.is_ascii_digit());
            self.w.put(if numeric { "px\"" } else { "\"" });
        }
        self.w.put_byte(b'>');
        self.write_aliases(id);
        self.w.cr();
        Visit::Descend(Close::Block("figure"))
    }

    pub fn enter_caption(&mut self, id: NodeId) -> Visit {
        self.line_open("figcaption", id, "");
        self.caption_open(self.parent());
        Visit::Descend(Close::Caption)
    }

    pub fn leave_caption(&mut self) {
        self.caption_close(self.parent(), label(self.options, Label::LinkImage));
        self.line_close("figcaption");
    }

    pub fn enter_admonition(&mut self, id: NodeId) -> Visit {
        let kind =
            AdmonitionKind::from_value(self.doc.node(id).value).unwrap_or(AdmonitionKind::Generic);
        self.block_open("aside", id, admonition_class(kind));
        if kind != AdmonitionKind::Generic {
            self.w.put("<p class=\"admonition-title\">");
            self.w.esc(admonition_label(self.options, kind));
            self.w.put("</p>");
            self.w.cr();
        }
        Visit::Descend(Close::Block("aside"))
    }

    pub fn enter_container(&mut self, id: NodeId) -> Visit {
        let wrapper = self.doc.has_class(id, "toctree-wrapper");
        self.block("div", id, if wrapper { "" } else { "container" })
    }

    /// A label that names nothing after it keeps its ids, as an empty element.
    pub fn enter_target(&mut self, id: NodeId) -> Visit {
        self.write_all_ids(id);
        Visit::Skip
    }

    pub fn enter_directive(&mut self, id: NodeId) -> Visit {
        let name = self.directive_name(id);
        match name {
            "sectnum" | "section-numbering" | "meta" | "title" => Visit::Skip,
            "header" => self.block("header", id, ""),
            "footer" => self.block("footer", id, ""),
            "target-notes" => {
                self.target_notes(id);
                Visit::Skip
            }
            "sectionauthor" | "moduleauthor" | "codeauthor" => self.enter_author(id, name),
            "todolist" => Visit::Descend(Close::Nothing),
            _ => {
                self.w.cr();
                self.open_tag("div", id, "directive");
                self.w.put(" data-directive=\"");
                self.w.esc(name);
                self.w.put("\">");
                self.write_aliases(id);
                self.w.cr();
                Visit::Descend(Close::Block("div"))
            }
        }
    }
}

/// The element of a list of `style`.
pub(crate) fn list_tag(style: ListStyle) -> &'static str {
    if style == ListStyle::Bullet { "ul" } else { "ol" }
}
