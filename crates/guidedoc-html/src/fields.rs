//! Field lists, and those directly in an object's content, which Sphinx's info fields
//! rebuild. These render as Sphinx's HTML writer (Docutils' html5 writer with Sphinx's
//! rows) writes them: `<dl class="field-list simple">`; each field a
//! `<dt class="field-odd">Name<span class="colon">:</span></dt>` and a
//! `<dd class="field-odd">` whose first child follows on the same line, rows alternating
//! odd and even. A list in a field body is `<ul class="simple">` when it is compact, and
//! its items are `<li><p>...</p></li>`.

use guidedoc::{Document, ListStyle, NodeId, NodeKind};

use crate::blocks::list_tag;
use crate::state::Html;
use crate::walk::{Close, Visit};

impl Html<'_, '_> {
    /// Tells whether the ancestor `k` levels up is a field list in an object's content.
    fn object_fields(&self, k: usize) -> bool {
        let (Some(list), Some(owner)) = (self.ancestor(k), self.ancestor(k + 1)) else {
            return false;
        };
        !self.w.reference
            && self.doc.kind(list) == NodeKind::FieldList
            && self.doc.kind(owner) == NodeKind::DescContent
    }

    fn parent_is(&self, k: usize, kind: NodeKind) -> bool {
        self.ancestor(k).is_some_and(|p| self.doc.kind(p) == kind)
    }

    pub fn enter_field_list(&mut self, id: NodeId) -> Visit {
        if self.w.reference || !self.parent_is(1, NodeKind::DescContent) {
            return self.block("dl", id, "field-list");
        }
        self.w.cr();
        if self.aliases_before(id) {
            self.write_aliases(id);
        }
        self.w.put("<dl");
        self.write_id_attribute(id);
        self.w.put(" class=\"");
        if self.written_classes(id) {
            self.w.put_byte(b' ');
        }
        let simple = compact_fields(self.doc, id);
        self.w.put(if simple { "field-list simple\">" } else { "field-list\">" });
        self.w.cr();
        Visit::Descend(Close::Block("dl"))
    }

    pub fn enter_field_name(&mut self, id: NodeId) -> Visit {
        if !self.object_fields(2) {
            return self.line("dt", id, "");
        }
        self.w.cr();
        self.open_field_part("<dt", id);
        Visit::Descend(Close::ObjectFieldName)
    }

    pub fn enter_field_body(&mut self, id: NodeId) -> Visit {
        if !self.object_fields(2) {
            return self.enter_dd(id);
        }
        self.open_field_part("<dd", id);
        if !self.doc.has_children(id) {
            self.w.put("<p></p>");
        }
        self.w.continue_line();
        Visit::Descend(Close::Line("dd"))
    }

    /// Writes `<dt` or `<dd` with the field's classes and its row's parity.
    fn open_field_part(&mut self, start: &str, id: NodeId) {
        let field = self.parent();
        self.w.put(start);
        self.write_id_attribute(id);
        self.w.put(" class=\"");
        if let Some(field) = field
            && self.written_classes(field)
        {
            self.w.put_byte(b' ');
        }
        let row = match (self.ancestor(2), field) {
            (Some(list), Some(field)) => self.doc.children(list).position(|c| c == field),
            _ => None,
        };
        let odd = row.unwrap_or(0) % 2 == 0;
        self.w.put(if odd { "field-odd\">" } else { "field-even\">" });
        self.write_aliases(id);
    }

    /// Opens a list in an info field's body: `<ul class="simple">` when compact.
    pub fn enter_object_field_list(&mut self, id: NodeId) -> Option<Visit> {
        if !self.object_fields(3) || !self.parent_is(1, NodeKind::FieldBody) {
            return None;
        }
        self.w.cr();
        let style = self.doc.list_info(id).style;
        if style == ListStyle::Bullet && compact_node(self.doc, id) {
            self.open_tag("ul", id, "simple");
            self.w.put_byte(b'>');
            self.w.cr();
        } else {
            self.open_list(id, 0, true);
        }
        Some(Visit::Descend(Close::Block(list_tag(style))))
    }

    /// Tells whether a list item being entered is in a list in an info field's body.
    pub fn in_object_field_list(&self) -> bool {
        self.object_fields(4) && self.parent_is(2, NodeKind::FieldBody)
    }

    /// Opens a paragraph of an item of such a list.
    pub fn enter_object_field_paragraph(&mut self, id: NodeId) -> Option<Visit> {
        if !self.object_fields(5) || !self.parent_is(1, NodeKind::ListItem) {
            return None;
        }
        self.line_open("p", id, "");
        Some(Visit::Descend(Close::ObjectFieldParagraph))
    }

    /// The only child of its item ends on the item's line.
    pub fn leave_object_field_paragraph(&mut self) {
        self.w.close("p");
        if self.parent().is_some_and(|item| self.doc.child_count(item) > 1) {
            self.w.cr();
        }
    }
}

/// Docutils' is_compactable for a field list: the classes "compact" and "open" decide,
/// else every field must be simple. A field an info field rebuilt is simple, as its
/// body in Sphinx's tree is a paragraph or a list of paragraphs.
fn compact_fields(doc: Document<'_>, list: NodeId) -> bool {
    if doc.has_class(list, "compact") {
        return true;
    }
    if doc.has_class(list, "open") {
        return false;
    }
    doc.children(list)
        .all(|field| doc.custom_attribute(field, "docfield").is_some() || compact_node(doc, field))
}

/// Docutils' SimpleListChecker: an item, definition, or field body holds at most one
/// visible child, not counting a list or field list after a first paragraph;
/// paragraphs, terms, and field names are not looked into, and comments, targets, and
/// substitution definitions not counted; lists and their parts are simple when their
/// items are; anything else is not simple.
fn compact_node(doc: Document<'_>, id: NodeId) -> bool {
    use NodeKind as K;
    let kind = doc.kind(id);
    if matches!(
        kind,
        K::Paragraph
            | K::Term
            | K::Classifier
            | K::FieldName
            | K::Comment
            | K::Target
            | K::SubstitutionDefinition
    ) {
        return true;
    }
    let container = matches!(kind, K::ListItem | K::FieldBody | K::Definition);
    let structure =
        matches!(kind, K::List | K::FieldList | K::Field | K::DefinitionList | K::DefinitionItem);
    if !container && !structure {
        return false;
    }
    if container && visible_children(doc, id) > 1 {
        return false;
    }
    doc.children(id).all(|c| compact_node(doc, c))
}

/// Counts the children that show, not counting a list after a first paragraph.
fn visible_children(doc: Document<'_>, id: NodeId) -> usize {
    use NodeKind as K;
    let hidden = |c: &NodeId| {
        matches!(doc.kind(*c), K::Comment | K::Target | K::SubstitutionDefinition | K::Index)
    };
    let mut visible = doc.children(id).filter(|c| !hidden(c));
    let first = visible.clone().next();
    let last = visible.clone().last();
    let count = visible.by_ref().count();
    let (Some(first), Some(last)) = (first, last) else {
        return count;
    };
    let trailing_list = matches!(doc.kind(last), K::List | K::FieldList);
    if count > 1 && doc.kind(first) == K::Paragraph && trailing_list { count - 1 } else { count }
}
