//! A flat copy of a tree-sitter syntax tree: one `Vec` of nodes indexed by [`NodeId`],
//! with each node's children stored contiguously, plus the source it spans. Copying the
//! tree out of tree-sitter lets analysed data hold plain ids instead of borrowing the
//! parser's tree, and every walk here is either iterative or bounded by
//! [`MAX_SYNTAX_DEPTH`](crate::MAX_SYNTAX_DEPTH).

use std::fmt;

/// The index of a node in its module's [`Syntax`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

const NONE: u32 = u32::MAX;

#[derive(Debug, Clone)]
struct RawNode {
    kind: &'static str,
    field: Option<&'static str>,
    start: u32,
    end: u32,
    row: u32,
    end_row: u32,
    first_child: u32,
    child_count: u32,
    named: bool,
    error: bool,
}

/// A parsed source: its text (line breaks normalised to `\n`) and its nodes.
#[derive(Clone, Default)]
pub struct Syntax {
    source: String,
    nodes: Vec<RawNode>,
}

impl fmt::Debug for Syntax {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Syntax")
            .field("bytes", &self.source.len())
            .field("nodes", &self.nodes.len())
            .finish()
    }
}

impl Syntax {
    /// Copies `tree`, parsed from `source`, into a flat syntax that owns the source.
    pub(crate) fn from_tree(tree: &tree_sitter::Tree, source: String) -> Syntax {
        let root = tree.root_node();
        let mut nodes = Vec::with_capacity(root.descendant_count());
        let mut pending = Vec::with_capacity(nodes.capacity());
        nodes.push(raw(root, None));
        pending.push(root);
        let mut cursor = root.walk();
        let mut i = 0;
        // Breadth first, so each node's children are contiguous.
        while i < pending.len() {
            let first = nodes.len() as u32;
            cursor.reset(pending[i]);
            if cursor.goto_first_child() {
                loop {
                    nodes.push(raw(cursor.node(), cursor.field_name()));
                    pending.push(cursor.node());
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
            nodes[i].first_child = first;
            nodes[i].child_count = nodes.len() as u32 - first;
            i += 1;
        }
        Syntax { source, nodes }
    }

    /// The source text the nodes span.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The root node (a `module`), or a null node for an empty syntax.
    pub fn root(&self) -> Node<'_> {
        self.node(if self.nodes.is_empty() { None } else { Some(NodeId(0)) })
    }

    /// The node an id names; a null node for `None` or an id from another syntax.
    pub fn node(&self, id: impl Into<Option<NodeId>>) -> Node<'_> {
        let id = match id.into() {
            Some(NodeId(i)) if (i as usize) < self.nodes.len() => i,
            _ => NONE,
        };
        Node { syntax: self, id }
    }
}

fn raw(n: tree_sitter::Node<'_>, field: Option<&'static str>) -> RawNode {
    RawNode {
        kind: n.kind(),
        field,
        start: n.start_byte() as u32,
        end: n.end_byte() as u32,
        row: n.start_position().row as u32,
        end_row: n.end_position().row as u32,
        first_child: 0,
        child_count: 0,
        named: n.is_named(),
        error: n.has_error(),
    }
}

/// Whether the tree below `root` nests more than `limit` levels (its children are level
/// 1). It walks with a cursor, so it needs no recursion however deep the tree is.
pub(crate) fn deeper_than(root: tree_sitter::Node<'_>, limit: usize) -> bool {
    let mut c = root.walk();
    let mut depth = 0;
    loop {
        if c.goto_first_child() {
            depth += 1;
            if depth > limit {
                return true;
            }
            continue;
        }
        while !c.goto_next_sibling() {
            if depth == 0 || !c.goto_parent() {
                return false;
            }
            depth -= 1;
        }
    }
}

/// A node of a [`Syntax`], or the null node (an absent field, child, or value), which
/// answers every question with nothing: kind `""`, no text, no children.
#[derive(Clone, Copy)]
pub struct Node<'a> {
    syntax: &'a Syntax,
    id: u32,
}

impl fmt::Debug for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node({} {:?})", self.kind(), self.id())
    }
}

impl PartialEq for Node<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.syntax, other.syntax) && self.id == other.id
    }
}

impl<'a> Node<'a> {
    fn raw(&self) -> Option<&'a RawNode> {
        self.syntax.nodes.get(self.id as usize)
    }

    /// The node's id, or `None` for the null node.
    pub fn id(&self) -> Option<NodeId> {
        if self.id == NONE { None } else { Some(NodeId(self.id)) }
    }

    /// Whether this is the null node.
    pub fn is_null(&self) -> bool {
        self.id == NONE
    }

    /// The grammar's name for the node (`"identifier"`, `"call"`, ...); `""` when null.
    pub fn kind(&self) -> &'static str {
        self.raw().map_or("", |r| r.kind)
    }

    /// Whether the grammar names the node (rather than an anonymous token such as `(`).
    pub fn is_named(&self) -> bool {
        self.raw().is_some_and(|r| r.named)
    }

    /// Whether the node or one below it is a syntax error or a missing token.
    pub fn has_error(&self) -> bool {
        self.raw().is_some_and(|r| r.error)
    }

    /// The source the node spans; `""` when null.
    pub fn text(&self) -> &'a str {
        match self.raw() {
            Some(r) => self.syntax.source.get(r.start as usize..r.end as usize).unwrap_or(""),
            None => "",
        }
    }

    /// The byte offset where the node starts.
    pub fn start_byte(&self) -> usize {
        self.raw().map_or(0, |r| r.start as usize)
    }

    /// The byte offset where the node ends.
    pub fn end_byte(&self) -> usize {
        self.raw().map_or(0, |r| r.end as usize)
    }

    /// The 0-based line the node starts on.
    pub fn start_row(&self) -> usize {
        self.raw().map_or(0, |r| r.row as usize)
    }

    /// The 0-based line the node ends on.
    pub fn end_row(&self) -> usize {
        self.raw().map_or(0, |r| r.end_row as usize)
    }

    /// The 1-based line the node starts on.
    pub fn line(&self) -> u32 {
        self.raw().map_or(0, |r| r.row + 1)
    }

    /// The syntax the node belongs to.
    pub fn syntax(&self) -> &'a Syntax {
        self.syntax
    }

    /// Another node of the same syntax, by id.
    pub fn node_at(&self, id: NodeId) -> Node<'a> {
        self.syntax.node(id)
    }

    /// The source between two byte offsets of the node's syntax; `""` when out of range.
    pub fn source_slice(&self, start: usize, end: usize) -> &'a str {
        self.syntax.source.get(start..end).unwrap_or("")
    }

    /// The source of the whole syntax the node belongs to.
    pub fn source(&self) -> &'a str {
        &self.syntax.source
    }

    /// The field the node fills in its parent, if any.
    pub fn field_name(&self) -> Option<&'static str> {
        self.raw().and_then(|r| r.field)
    }

    /// How many children the node has, named and anonymous.
    pub fn child_count(&self) -> usize {
        self.raw().map_or(0, |r| r.child_count as usize)
    }

    /// Child `i` of [`child_count`](Self::child_count), or the null node.
    pub fn child(&self, i: usize) -> Node<'a> {
        let id = match self.raw() {
            Some(r) if i < r.child_count as usize => r.first_child + i as u32,
            _ => NONE,
        };
        Node { syntax: self.syntax, id }
    }

    /// Every child, named and anonymous, in order.
    pub fn children(self) -> impl Iterator<Item = Node<'a>> + 'a {
        let (first, count) = self.raw().map_or((0, 0), |r| (r.first_child, r.child_count));
        let syntax = self.syntax;
        (first..first + count).map(move |id| Node { syntax, id })
    }

    /// The named children (comments among them), in order.
    pub fn named_children(self) -> impl Iterator<Item = Node<'a>> + 'a {
        self.children().filter(|c| c.is_named())
    }

    /// How many named children the node has.
    pub fn named_child_count(&self) -> usize {
        self.named_children().count()
    }

    /// Named child `i`, or the null node.
    pub fn named_child(&self, i: usize) -> Node<'a> {
        self.named_children().nth(i).unwrap_or(Node { syntax: self.syntax, id: NONE })
    }

    /// The first child filling the field `name` (`"name"`, `"body"`, ...), or null.
    pub fn field(&self, name: &str) -> Node<'a> {
        self.children()
            .find(|c| c.field_name() == Some(name))
            .unwrap_or(Node { syntax: self.syntax, id: NONE })
    }

    /// The source of a field, or `""`.
    pub fn field_text(&self, name: &str) -> &'a str {
        self.field(name).text()
    }
}
