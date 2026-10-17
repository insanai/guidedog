//! The document title may remain a section in a reader's AST.

use guidedoc::{Document, NodeId, NodeKind, ROOT};

/// Returns the first root section, whose heading is the document title, or the root
/// when the reader has already promoted that title out of its section.
pub fn excerpt_root(doc: Document<'_>) -> NodeId {
    if doc.children(ROOT).any(|id| matches!(doc.kind(id), NodeKind::Title | NodeKind::Heading)) {
        return ROOT;
    }
    doc.children(ROOT).find(|&id| doc.kind(id) == NodeKind::Section).unwrap_or(ROOT)
}
