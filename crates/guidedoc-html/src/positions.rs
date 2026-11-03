//! Source positions in preview HTML: with `RenderOptions::source_positions`, the first
//! tag a block writes gets `data-gd-source="<source>:<start>-<end>"`, the byte span of
//! the markup it comes from, so a click in the preview maps back to the source.

use guidedoc::{NodeId, NodeKind};

use crate::state::Html;

/// The attribute that carries a block's source span.
pub const SOURCE_ATTRIBUTE: &str = "data-gd-source";

impl Html<'_, '_> {
    /// Adds the source attribute of node `id` to the first tag written since `from`
    /// bytes had been, when positions are asked for and the node is a block with a
    /// source span.
    pub(crate) fn mark_source(&mut self, id: NodeId, from: usize) {
        let Some(root) = self.options.source_positions else { return };
        let node = self.doc.node(id);
        if !is_marked(node.kind, self.parent().map(|p| self.doc.kind(p))) || node.span.is_zero() {
            return;
        }
        let Some(at) = tag_name_end(self.w.written_since(from)) else { return };
        let name = self.doc.source_entry(node.span.source).map_or("", |e| e.name.as_str());
        let shown = relative(name, root);
        let mut attribute = format!(" {SOURCE_ATTRIBUTE}=\"");
        escape_attribute(&mut attribute, &shown);
        attribute.push_str(&format!(":{}-{}\"", node.span.start, node.span.end));
        self.w.insert(from + at, &attribute);
    }
}

/// Tells whether a node of `kind` (in a parent of kind `parent`) is a block whose
/// element carries its position.
fn is_marked(kind: NodeKind, parent: Option<NodeKind>) -> bool {
    use NodeKind as K;
    match kind {
        K::Paragraph | K::BlockQuote | K::List | K::ListItem | K::DefinitionList | K::Term => true,
        K::FieldList | K::LineBlock | K::Preformatted | K::CodeBlock | K::DoctestBlock => true,
        K::MathBlock | K::ThematicBreak | K::Figure | K::Caption | K::Admonition => true,
        K::Topic | K::Sidebar | K::Rubric | K::Container | K::Compound | K::Section => true,
        K::Heading | K::Title | K::Table | K::Footnote | K::Citation | K::Directive => true,
        K::Desc | K::Glossary | K::VersionModified | K::Hlist | K::OptionList => true,
        // An image on its own is a block; inside a paragraph or a link it is inline.
        K::Image => !matches!(parent, Some(K::Paragraph | K::Link | K::Reference | K::Figure)),
        _ => false,
    }
}

/// The offset just after the name of the first start tag in `bytes`.
fn tag_name_end(bytes: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'<' && bytes[i + 1].is_ascii_alphabetic() {
            let name = bytes[i + 1..].iter().take_while(|c| c.is_ascii_alphanumeric()).count();
            return Some(i + 1 + name);
        }
        i += 1;
    }
    None
}

/// `name` relative to the folder `root`, with `/` separators.
fn relative(name: &str, root: &str) -> String {
    let name = name.replace('\\', "/");
    let root = root.replace('\\', "/");
    let root = root.trim_end_matches('/');
    match name.strip_prefix(root) {
        Some(rest) if !root.is_empty() && rest.starts_with('/') => rest[1..].to_string(),
        _ => name,
    }
}

fn escape_attribute(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(tag_name_end(b"\n<p class=\"x\">"), Some(3));
        assert_eq!(tag_name_end(b"</p><section>"), Some(12));
        assert_eq!(tag_name_end(b"text"), None);
        assert_eq!(relative("/w/docs/a/b.rst", "/w/docs"), "a/b.rst");
        assert_eq!(relative("/w/docsx/b.rst", "/w/docs"), "/w/docsx/b.rst");
        assert_eq!(relative("b.rst", ""), "b.rst");
    }
}
