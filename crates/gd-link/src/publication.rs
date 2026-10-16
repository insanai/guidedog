//! Publication fields are configuration; layouts render their structured values.

use guidedoc::{Document, NodeId, ROOT};

pub(crate) use gd_env::publication_field as field;

pub(crate) fn list(doc: Document<'_>, id: NodeId) -> bool {
    doc.first_child(ROOT) == Some(id) && doc.children(id).all(|id| field(doc, id))
}
