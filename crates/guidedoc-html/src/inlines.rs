//! Inline elements: code and math spans, links, images and media, and substitutions.

use guidedoc::{
    Arg, AttributeKey, Content, LinkInfo, LinkKind, MathStyle, MediaKind, NodeId, NodeKind,
    arg_int, arg_span, content_model, media_kind,
};

use crate::state::Html;
use crate::walk::{Close, Visit};
use crate::{EXPANSION, UNRESOLVED};

impl Html<'_, '_> {
    pub fn enter_code_span(&mut self, id: NodeId) -> Visit {
        self.open_tag("code", id, "");
        let language = self.doc.attribute(id, AttributeKey::Language).unwrap_or("");
        if !language.is_empty() && !self.w.reference {
            self.w.put(" data-language=\"");
            self.w.esc(language);
            self.w.put_byte(b'"');
        }
        self.w.put_byte(b'>');
        self.write_aliases(id);
        self.w.esc(self.doc.node_text(id));
        self.w.put("</code>");
        Visit::Skip
    }

    pub fn enter_math(&mut self, id: NodeId) -> Visit {
        let o = self.options;
        let sphinx = o.math == MathStyle::Sphinx;
        let class = if sphinx { "math notranslate nohighlight" } else { "math" };
        self.open_tag("span", id, class);
        self.w.put_byte(b'>');
        self.write_aliases(id);
        let [open, close] = o.math_inline;
        self.w.put(if sphinx && !open.is_empty() { open } else { "\\(" });
        self.w.esc(self.doc.node_text(id));
        self.w.put(if sphinx && !close.is_empty() { close } else { "\\)" });
        self.w.put("</span>");
        Visit::Skip
    }

    pub fn enter_abbreviation(&mut self, id: NodeId) -> Visit {
        self.open_tag("abbr", id, "");
        if let Some(title) = self.doc.attribute(id, AttributeKey::Title) {
            self.w.put(" title=\"");
            self.w.esc(title);
            self.w.put_byte(b'"');
        }
        self.w.put_byte(b'>');
        Visit::Descend(Close::Tag("abbr"))
    }

    pub fn write_title_attribute(&mut self, id: NodeId) {
        let l = self.doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
        if l.title.is_empty() {
            return;
        }
        self.w.put(" title=\"");
        self.w.esc(self.doc.text(l.title));
        self.w.put_byte(b'"');
    }

    /// Tells whether an unresolved link is kept as problematic text, as Docutils keeps
    /// it below its halt level; otherwise the link is an error.
    fn problematic(&self, id: NodeId) -> bool {
        self.policy.keep_documents_with_errors
            && self.doc.link_info(id).is_none_or(|l| l.kind == LinkKind::Unresolved)
    }

    pub fn enter_link(&mut self, id: NodeId) -> Visit {
        if self.problematic(id) {
            let span = self.doc.node(id).span;
            self.w.fail(UNRESOLVED, span, arg_span(span), Arg::None);
            self.open_tag("span", id, "problematic");
            self.w.put_byte(b'>');
            self.write_aliases(id);
            return Visit::Descend(Close::Link);
        }
        self.w.put("<a");
        self.write_target("href", id);
        self.write_title_attribute(id);
        if !self.w.reference {
            if self.custom(id, "download") == "1" {
                self.w.put(" download=\"\"");
            }
            self.write_id_attribute(id);
            self.write_classes(id, "");
        }
        self.w.put_byte(b'>');
        self.write_aliases(id);
        Visit::Descend(Close::Link)
    }

    pub fn leave_link(&mut self, id: NodeId) {
        self.w.close(if self.problematic(id) { "span" } else { "a" });
        self.note_references(id);
    }

    /// Writes an img element; its alt text comes from its children, or from the Alt
    /// attribute of a block-level image.
    pub fn enter_image(&mut self, id: NodeId) -> Visit {
        let block = content_model(self.parent_kind(self.parent())) == Content::Blocks;
        if block {
            self.w.cr();
        }
        self.write_aliases(id);
        let dest = self.doc.link_info(id).map_or("", |l| self.doc.text(l.destination));
        let kind = media_kind(dest);
        if !self.w.reference && kind != MediaKind::Image {
            self.write_media(id, kind, dest);
            if block {
                self.w.cr();
            }
            return Visit::Skip;
        }
        self.w.put("<img");
        if !self.write_target("src", id) {
            self.w.put(" src=\"\"");
        }
        self.w.put(" alt=\"");
        if self.doc.has_children(id) {
            self.write_alt(id);
        } else if let Some(alt) = self.doc.attribute(id, AttributeKey::Alt) {
            self.w.esc(alt);
        }
        self.w.put_byte(b'"');
        self.write_title_attribute(id);
        if !self.w.reference {
            self.write_id_attribute(id);
            self.write_dimension(id, AttributeKey::Width, "width");
            self.write_dimension(id, AttributeKey::Height, "height");
            if let Some(align) = self.doc.attribute(id, AttributeKey::Align) {
                self.w.put(" data-align=\"");
                self.w.esc(align);
                self.w.put_byte(b'"');
            }
            self.write_classes(id, "");
        }
        self.w.put(" />");
        if block {
            self.w.cr();
        }
        Visit::Skip
    }

    /// Writes a video or audio player. Its content, a link to the file, is what a
    /// browser that cannot play the file shows, as in Docutils' video element.
    fn write_media(&mut self, id: NodeId, kind: MediaKind, path: &str) {
        let element = if kind == MediaKind::Video { "video" } else { "audio" };
        self.w.put_byte(b'<');
        self.w.put(element);
        if !self.write_target("src", id) {
            self.w.put(" src=\"\"");
        }
        self.w.put(" controls");
        let alt = self.doc.attribute(id, AttributeKey::Alt).unwrap_or("");
        if !alt.is_empty() {
            self.w.put(" title=\"");
            self.w.esc(alt);
            self.w.put_byte(b'"');
        }
        self.write_id_attribute(id);
        if kind == MediaKind::Video {
            self.write_dimension(id, AttributeKey::Width, "width");
            self.write_dimension(id, AttributeKey::Height, "height");
        }
        self.write_classes(id, "");
        self.w.put("><a");
        self.write_target("href", id);
        self.w.put_byte(b'>');
        if self.doc.has_children(id) {
            self.write_alt(id);
        } else if !alt.is_empty() {
            self.w.esc(alt);
        } else {
            self.w.esc(path.rsplit('/').next().unwrap_or(path));
        }
        self.w.put("</a></");
        self.w.put(element);
        self.w.put_byte(b'>');
    }

    /// Writes a width or height: a number as an attribute, a length as a style.
    fn write_dimension(&mut self, id: NodeId, key: AttributeKey, name: &str) {
        let Some(value) = self.doc.attribute(id, key).filter(|v| !v.is_empty()) else {
            return;
        };
        self.w.put_byte(b' ');
        if value.bytes().all(|c| c.is_ascii_digit()) {
            self.w.put(name);
            self.w.put("=\"");
        } else {
            self.w.put("style=\"");
            self.w.put(name);
            self.w.put(": ");
        }
        self.w.esc(value);
        self.w.put_byte(b'"');
    }

    /// Writes the plain text of an image description, as the reference renderer does
    /// with tags disabled.
    fn write_alt(&mut self, id: NodeId) {
        let doc = self.doc;
        for i in id.0 + 1..doc.subtree_end(id).0 {
            let node = NodeId(i);
            // Markup adds no text.
            let kind = doc.kind(node);
            if matches!(
                kind,
                NodeKind::Text | NodeKind::Code | NodeKind::Math | NodeKind::RawInline
            ) {
                self.w.esc(doc.node_text(node));
            } else if matches!(kind, NodeKind::SoftBreak | NodeKind::HardBreak) {
                self.w.put_byte(b'\n');
            }
        }
    }

    /// Renders the resolved definition's children in place.
    pub fn enter_substitution(&mut self, id: NodeId) -> Visit {
        let doc = self.doc;
        let span = doc.node(id).span;
        let definition = doc.link_info(id).filter(|l| {
            l.kind == LinkKind::Internal
                && l.resolved.index() < doc.node_count()
                && doc.kind(l.resolved) == NodeKind::SubstitutionDefinition
        });
        let Some(l) = definition else {
            self.w.fail(UNRESOLVED, span, arg_span(span), Arg::None);
            if self.policy.keep_documents_with_errors {
                self.open_tag("span", id, "problematic");
                self.w.put(">|");
                self.w.esc(doc.node_text(id));
                self.w.put("|</span>");
            }
            return Visit::Skip;
        };
        let limit = if self.limits.max_expansion > 0 { self.limits.max_expansion } else { 16 };
        if self.expansion >= limit {
            let max = arg_int(i64::try_from(limit).unwrap_or(i64::MAX));
            self.w.fail(EXPANSION, span, arg_span(span), max);
            return Visit::Skip;
        }
        self.expansion += 1;
        let definition = l.resolved;
        self.walk(definition + 1, doc.subtree_end(definition), self.parent());
        self.expansion -= 1;
        Visit::Skip
    }
}
