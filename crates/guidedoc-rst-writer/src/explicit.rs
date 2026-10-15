//! Explicit markup that is not a directive: comments, footnotes and citations,
//! hyperlink targets, and substitution definitions.

use guidedoc::{AttributeKey, LinkKind, NodeFlag, NodeId, NodeKind};

use crate::escape::is_simple_name;
use crate::state::Writer;

impl Writer<'_, '_> {
    /// `.. text`: a comment. Text that would read as other explicit markup on the
    /// first line starts on the next one.
    pub fn comment(&mut self, id: NodeId) {
        let text = self.doc.node_text(id).trim_end_matches('\n').trim_start_matches('\n');
        if text.trim().is_empty()
            && let Some(next) = self.next_block(id)
            && self.kind(next) == NodeKind::BlockQuote
        {
            // A class for the quote after the comment goes before the comment.
            self.class_prefix(next, &["epigraph", "highlights", "pull-quote"]);
        }
        self.block_start();
        if text.trim().is_empty() {
            self.line("..");
            self.block_end();
            return;
        }
        let mut lines = text.split('\n');
        let first = lines.next().unwrap_or("");
        let risky = first.starts_with(['[', '_', '|', ' ']) || first.contains("::");
        if risky {
            self.line("..");
            self.push_frame(3, "");
        } else {
            self.push_frame(3, ".. ");
        }
        self.line(first);
        for line in lines {
            self.line(line.trim_end());
        }
        self.pop_frame();
        self.block_end();
    }

    /// `.. [label] body`: a footnote or citation, its label as written.
    pub fn note(&mut self, id: NodeId) {
        let label = self.doc.node_text(id);
        self.class_prefix(id, &[]);
        self.block_start();
        let marker = format!(".. [{label}] ");
        let first = self.doc.first_child(id);
        let same_line = first.is_some_and(|f| self.kind(f) == NodeKind::Paragraph);
        self.push_frame(3, &marker);
        if !same_line {
            self.detach_marker();
        }
        self.blocks(id);
        self.pop_frame();
        self.block_end();
    }

    /// A hyperlink target: `.. _name: URI`, `.. _name: other_`, `.. _name:`, or the
    /// anonymous `.. __: URI`.
    pub fn target(&mut self, id: NodeId) {
        if self.names_earlier(id) {
            return;
        }
        self.write_target(id);
    }

    /// Tells whether a target names an element before it: a promoted title, which
    /// `title` writes the target before.
    pub fn names_earlier(&self, id: NodeId) -> bool {
        let link = self.doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
        link.kind == LinkKind::Internal
            && link.resolved < id
            && matches!(self.kind(link.resolved), NodeKind::Title | NodeKind::Subtitle)
    }

    /// Writes a target line.
    pub fn write_target(&mut self, id: NodeId) {
        let node = self.doc.node(id);
        let link = self.doc.link_info(id).unwrap_or(guidedoc::LinkInfo::UNRESOLVED);
        let anonymous = node.flags.has(NodeFlag::Anonymous);
        let name = self.doc.node_text(id);
        let head = if anonymous { "__".to_owned() } else { format!("_{}", target_name(name)) };
        let referenced = self.doc.text(link.name);
        let destination = self.doc.text(link.destination);
        let body = if !referenced.is_empty() {
            format!(" {}", reference_to(referenced))
        } else if link.kind == LinkKind::External || link.kind == LinkKind::Email {
            if destination.is_empty() {
                String::new()
            } else {
                format!(" {}", destination.replace('\\', "\\\\"))
            }
        } else {
            String::new()
        };
        if name.is_empty() && !anonymous {
            self.lossy(id, "a target without a name", "an anonymous target");
        }
        self.block_start();
        self.line(&format!(".. {head}:{body}"));
        self.block_end();
    }

    /// `.. |name| replace:: text`, or the image, or unicode form a definition needs.
    pub fn substitution_definition(&mut self, id: NodeId) {
        let name = self.doc.node_text(id).to_owned();
        let mut children = self.doc.children(id);
        let first = children.next();
        let alone = children.next().is_none();
        if let Some(image) = first.filter(|_| alone).and_then(|f| self.image_in(f)) {
            let wrapper = first.filter(|&f| f != image);
            self.image_directive(&format!("|{name}| image"), image, wrapper);
            return;
        }
        let trims = self.custom(id, "ltrim").is_some() || self.custom(id, "rtrim").is_some();
        if trims {
            self.unicode_definition(id, &name);
            return;
        }
        self.block_start();
        let marker = format!(".. |{name}| replace:: ");
        self.push_frame(3, &marker);
        let lines = self.collect(id, false, |_| {});
        if lines.iter().all(|l| l.is_empty()) {
            self.lossy(id, "an empty substitution", "a definition with no text");
        }
        for line in &lines {
            self.line(line);
        }
        self.pop_frame();
        self.block_end();
    }

    /// `.. |name| unicode:: U+0041 ...` with its trim options, for definitions whose
    /// trimming the replace directive cannot express.
    fn unicode_definition(&mut self, id: NodeId, name: &str) {
        let mut text = String::new();
        self.doc.plain_text(id, &mut text);
        let codes: Vec<String> = text.chars().map(|c| format!("U+{:04X}", c as u32)).collect();
        let mut options = Vec::new();
        for key in ["ltrim", "rtrim"] {
            if self.custom(id, key).is_some() {
                options.push((key, String::new()));
            }
        }
        if !self.doc.children(id).all(|c| self.kind(c) == NodeKind::Text) {
            self.lossy(id, "markup in a trimmed substitution", "its text");
        }
        self.directive_head(&format!("|{name}| unicode"), &codes.join(" "), &options);
    }

    /// The image a substitution or figure holds, alone or inside a link.
    pub fn image_in(&self, id: NodeId) -> Option<NodeId> {
        match self.kind(id) {
            NodeKind::Image => Some(id),
            NodeKind::Link | NodeKind::Reference => {
                let mut children = self.doc.children(id);
                let only = children.next()?;
                (children.next().is_none() && self.kind(only) == NodeKind::Image).then_some(only)
            }
            _ => None,
        }
    }

    /// Writes the substitution definitions inline images of the last block needed.
    pub fn flush_substitutions(&mut self) {
        let pending = std::mem::take(&mut self.pending_images);
        for (name, image, wrapper) in pending {
            self.image_directive(&format!("|{name}| image"), image, wrapper);
        }
    }

    /// The `:alt:`-like attribute values of an image.
    pub fn image_alt(&self, id: NodeId) -> Option<String> {
        if let Some(alt) = self.attr(id, AttributeKey::Alt) {
            return Some(alt.to_owned());
        }
        if self.doc.has_children(id) {
            let mut text = String::new();
            self.doc.plain_text(id, &mut text);
            return Some(text);
        }
        None
    }
}

/// A target name as `.. _name:` writes it: backquoted when it has a colon or starts
/// with an underscore, backslash-escaping backquotes.
fn target_name(name: &str) -> String {
    if name.contains(':') || name.starts_with('_') || name.contains('`') {
        format!("`{}`", name.replace('\\', "\\\\").replace('`', "\\`"))
    } else {
        name.replace('\\', "\\\\")
    }
}

/// A reference to `name` as an indirect target writes it: `name_` or `` `name`_ ``.
pub(crate) fn reference_to(name: &str) -> String {
    if is_simple_name(name) {
        format!("{name}_")
    } else {
        format!("`{}`_", name.replace('\\', "\\\\").replace('`', "\\`"))
    }
}
