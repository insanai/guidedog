//! A snapshot is one document's tables: the preorder node table, the attribute and
//! payload tables, and the text pool. A workspace keeps two and reuses their storage.

use std::sync::Arc;

use crate::types::{
    Attribute, CellInfo, LinkInfo, ListInfo, Node, NodeId, SourceId, SourceSpan, Text,
};

/// The tables of one document. Readers and passes fill them through a `Builder`; its
/// `snap` field gives direct access for the few edits the builder has no method for
/// (`Vec::len()` is Odin's `used`).
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Nodes in preorder; `nodes[0]` is the Document.
    pub nodes: Vec<Node>,
    /// Attribute rows; each node's are contiguous.
    pub attributes: Vec<Attribute>,
    /// List payload rows.
    pub lists: Vec<ListInfo>,
    /// Link payload rows.
    pub links: Vec<LinkInfo>,
    /// Cell payload rows.
    pub cells: Vec<CellInfo>,
    /// The text pool. Bytes, so a reader can copy a source byte by byte; validation
    /// checks the pool is UTF-8 and every `Text` lies on character boundaries.
    pub text: Vec<u8>,
}

impl Snapshot {
    /// Empties every table, keeping their storage.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.attributes.clear();
        self.lists.clear();
        self.links.clear();
        self.cells.clear();
        self.text.clear();
    }

    /// Returns node `id`, or `None` when out of range.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// Returns the text `t` names, or "" when it is out of range or not UTF-8 (which
    /// happens only while a reader is half-way through pushing a character).
    pub fn pool_str(&self, t: Text) -> &str {
        self.text.get(t.range()).and_then(|b| std::str::from_utf8(b).ok()).unwrap_or("")
    }
}

/// How the reader uses an included source: parsed as markup (include), or shown as it
/// is (literalinclude, a graph file). Hosts tell a document's parts from files it only
/// quotes, as Sphinx's `env.included` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SourceUse {
    /// Read as markup.
    #[default]
    Parsed,
    /// Shown as it is.
    Literal,
}

/// One entry of a workspace's source table. The main source's text lives in the
/// workspace (reused between conversions); included and added sources share theirs.
#[derive(Debug, Clone, Default)]
pub struct SourceEntry {
    /// Its name (canonical, for included sources).
    pub name: String,
    /// The including source; itself for the main source.
    pub parent: SourceId,
    /// The include directive, when included.
    pub included_at: SourceSpan,
    /// How an included source is read.
    pub usage: SourceUse,
    /// The text of a source other than the main one.
    pub(crate) shared: Option<Arc<str>>,
}

impl SourceEntry {
    /// Makes an entry for a source other than the main one.
    pub(crate) fn shared(name: String, text: Arc<str>, parent: SourceId) -> SourceEntry {
        SourceEntry {
            name,
            parent,
            included_at: SourceSpan::default(),
            usage: SourceUse::Parsed,
            shared: Some(text),
        }
    }
}

/// Returns the text of source `id`: `main` for the main source, the shared text for the
/// others, "" for an unknown id.
pub(crate) fn entry_text<'a>(main: &'a str, sources: &'a [SourceEntry], id: SourceId) -> &'a str {
    match sources.get(id.index()) {
        Some(SourceEntry { shared: Some(text), .. }) => text,
        Some(_) => main,
        None => "",
    }
}
