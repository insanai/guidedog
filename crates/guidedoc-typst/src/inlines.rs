//! Inline handlers: code spans and links, with the unsafe-URL policy.

use guidedoc::{AttributeKey, ContentPolicy, LinkKind, Node, NodeId, arg_span, arg_text};

use crate::messages::{LINK_UNRESOLVED, URL_OMITTED, URL_UNSAFE};
use crate::state::State;

impl State<'_, '_> {
    pub(crate) fn emit_code(&mut self, id: NodeId, n: &Node) {
        self.emit("#raw(");
        if let Some(lang) =
            self.doc.attribute(id, AttributeKey::Language).filter(|l| !l.is_empty())
        {
            self.emit("lang: ");
            self.emit_string(lang);
            self.emit(", ");
        }
        self.emit_string(self.doc.text(n.text));
        self.byte(b')');
    }

    /// Writes a Link or Reference by its resolved kind.
    pub(crate) fn render_link(&mut self, id: NodeId) {
        let doc = self.doc;
        let Some(info) = doc.link_info(id) else {
            self.render_inlines(id);
            return;
        };
        match info.kind {
            LinkKind::External | LinkKind::Email => {
                let destination = doc.text(info.destination);
                if !self.url_allowed(id, destination) || self.quiet {
                    self.render_inlines(id);
                    return;
                }
                self.emit("#link(");
                self.emit_url(destination, info.kind == LinkKind::Email);
                self.byte(b')');
                if doc.has_children(id) {
                    self.wrap_inlines("[", id, "]");
                }
            }
            LinkKind::Internal => self.emit_internal_link(id, info.resolved),
            LinkKind::Unresolved => {
                let name = doc.text(info.name);
                let span = doc.node(id).span;
                let arg = if name.is_empty() { arg_span(span) } else { arg_text(name) };
                self.fail(LINK_UNRESOLVED, span, arg);
                self.render_inlines(id);
            }
            LinkKind::Project => {
                // A key into another document of the same book. gd-link keeps the text
                // when the key names no label, so one stale fragment cannot fail a book.
                let key = doc.text(info.destination);
                self.emit("#gd-link(");
                self.emit_string(key);
                self.byte(b')');
                if doc.has_children(id) {
                    self.wrap_inlines("[", id, "]");
                } else {
                    self.wrap_text("[", key, "]");
                }
            }
        }
    }

    /// Returns the node whose label stands for internal link target `target`, when it
    /// has a label.
    pub(crate) fn labelled_target(&self, target: NodeId) -> Option<NodeId> {
        if target.index() >= self.doc.node_count() {
            return None;
        }
        let node = self.label_target(target);
        self.anchors.anchor_of(node).map(|_| node)
    }

    /// Links to the label of `target`; gd-link keeps the text when the label is absent,
    /// so a reference to unlabelled content still compiles.
    fn emit_internal_link(&mut self, id: NodeId, target: NodeId) {
        let doc = self.doc;
        let node = match self.labelled_target(target) {
            Some(node) if !self.quiet => node,
            _ => {
                self.render_inlines(id);
                return;
            }
        };
        let name = self.anchors.anchor_of(node).unwrap_or("");
        let out = &mut self.ctx.out;
        out.emit("#gd-link(");
        crate::escape::write_string_literal(out, name);
        out.emit(")[");
        if doc.has_children(id) {
            self.render_inlines(id);
        } else {
            let name = self.anchors.anchor_of(node).unwrap_or("");
            crate::escape::write_markup(&mut self.ctx.out, name, None);
        }
        self.byte(b']');
    }

    /// Writes a destination string literal; email addresses gain `mailto:`.
    pub(crate) fn emit_url(&mut self, destination: &str, email: bool) {
        self.byte(b'"');
        if email && !has_prefix_fold(destination, "mailto:") {
            self.emit("mailto:");
        }
        self.emit_string_body(destination, false);
        self.byte(b'"');
    }

    /// Applies the unsafe-URL policy to `javascript:`, `vbscript:`, and non-image
    /// `data:` destinations. A refused or omitted link keeps its text.
    pub(crate) fn url_allowed(&mut self, id: NodeId, destination: &str) -> bool {
        if !url_unsafe(destination) {
            return true;
        }
        let span = self.doc.node(id).span;
        match self.ctx.policy.unsafe_urls {
            ContentPolicy::Allow => return true,
            ContentPolicy::Omit => self.fail(URL_OMITTED, span, arg_span(span)),
            ContentPolicy::Refuse => self.fail(URL_UNSAFE, span, arg_span(span)),
        }
        false
    }
}

/// Tells whether `url` has an executable or non-image data scheme, after leading white
/// space and control characters.
pub(crate) fn url_unsafe(url: &str) -> bool {
    let rest = url.trim_start_matches(|c: char| c <= ' ');
    if has_prefix_fold(rest, "javascript:") || has_prefix_fold(rest, "vbscript:") {
        return true;
    }
    has_prefix_fold(rest, "data:") && !has_prefix_fold(&rest[5..], "image/")
}

/// Tells whether `s` starts with `prefix`, ignoring ASCII case.
pub(crate) fn has_prefix_fold(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}
