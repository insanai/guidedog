//! Discussion metadata (discussion_metadata.odin). A discussion record (GDS and its
//! relatives) starts with a field list such as `:gds-number:`; its leading fields get
//! readable names ("Authors") and its JSON lists become comma-separated text, in every
//! output. Ordinary fields, examples and the source keep their names. The metadata is
//! a source contract, not a reader-facing vocabulary.

use guidedoc::{Document, Fail, NodeId, NodeKind, ROOT};

use crate::page::Page;
use crate::rewrite::{Action, Rewriter};

/// The key (`authors`) and readable label (`Authors`) of a field of a discussion's
/// leading metadata list; `None` for any other field, and for a key without a label.
pub fn discussion_field<'d>(doc: Document<'d>, field: NodeId) -> Option<(&'d str, &'static str)> {
    if doc.kind(field) != NodeKind::Field {
        return None;
    }
    let list = doc.parent_of(field)?;
    let leading = doc.kind(list) == NodeKind::FieldList
        && doc.parent_of(list) == Some(ROOT)
        && doc.first_child(ROOT) == Some(list);
    if !leading {
        return None;
    }
    let prefix = discussion_prefix(doc, list)?;
    let name = doc.first_child(field).filter(|&n| doc.kind(n) == NodeKind::FieldName)?;
    let (field_prefix, key) = doc.source_text(doc.node(name).span).split_once('-')?;
    if field_prefix != prefix {
        return None;
    }
    let label = match key {
        "number" => "Number",
        "title" => "Title",
        "state" => "State",
        "type" => "Type",
        "authors" => "Authors",
        "created" => "Created",
        "updated" | "last-updated" => "Updated",
        "discussion" => "Discussion",
        "labels" => "Labels",
        "reserved-through" => "Reserved through",
        "category" => "Category",
        "status" => "Status",
        "summary" => "Summary",
        _ => return None,
    };
    Some((key, label))
}

/// The prefix of a discussion's metadata list (`gds` for `:gds-number:`), or `None` for
/// an ordinary field list. The first field names the prefix, of 2 to 16 lowercase
/// letters, and the list must carry an identity field with it: a number, a title, or a
/// registry's reserved-through. So a page that starts with `:last-updated:` keeps its
/// field as written.
pub fn discussion_prefix<'d>(doc: Document<'d>, list: NodeId) -> Option<&'d str> {
    let field_name = |field: NodeId| {
        doc.first_child(field)
            .filter(|&n| doc.kind(n) == NodeKind::FieldName)
            .map(|n| doc.source_text(doc.node(n).span))
    };
    let (prefix, _) = field_name(doc.first_child(list)?)?.split_once('-')?;
    let lowercase = prefix.bytes().all(|c| c.is_ascii_lowercase());
    if !(2..=16).contains(&prefix.len()) || !lowercase {
        return None;
    }
    let identified = doc.children(list).filter_map(field_name).any(|name| {
        name.split_once('-').is_some_and(|(key, rest)| {
            key == prefix && matches!(rest, "number" | "title" | "reserved-through")
        })
    });
    identified.then_some(prefix)
}

/// What the link pass does with a field's name or body: the names of the metadata get
/// their labels, and the author and label lists their text.
pub(crate) fn decide(doc: Document<'_>, id: NodeId) -> Action {
    let Some((key, _)) = doc.parent_of(id).and_then(|field| discussion_field(doc, field)) else {
        return Action::Copy;
    };
    let named = doc.kind(id) == NodeKind::FieldName;
    if named || key == "authors" || key == "labels" { Action::Replace } else { Action::Copy }
}

/// Writes a metadata field's label, or its JSON list as comma-separated text. The JSON
/// is read from the source, before smart quotes; the values become plain text nodes,
/// so names holding markup stay data and every renderer escapes them.
pub(crate) fn replace_field(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let doc = rw.doc;
    let span = doc.node(id).span;
    let found = doc.parent_of(id).and_then(|field| discussion_field(doc, field));
    let Some((key, label)) = found else { return rw.copy_node(page, id) };
    if doc.kind(id) == NodeKind::FieldName {
        let name = rw.open(NodeKind::FieldName, span)?;
        rw.moved_to(id, name);
        rw.add_plain(label, span)?;
        return rw.close(name);
    }
    let values = match key {
        "authors" | "labels" => json_strings(doc.source_text(span).trim()),
        _ => None,
    };
    let Some(values) = values else { return rw.copy_node(page, id) };
    let body = rw.open(NodeKind::FieldBody, span)?;
    rw.moved_to(id, body);
    let paragraph = rw.open(NodeKind::Paragraph, span)?;
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            rw.add_plain(", ", span)?;
        }
        rw.add_plain(value, span)?;
    }
    rw.close(paragraph)?;
    rw.close(body)
}

/// A JSON array of strings, or `None` for anything else.
fn json_strings(text: &str) -> Option<Vec<String>> {
    let value = gd_json::parse(text).ok()?;
    value.as_array()?.iter().map(|v| v.as_str().map(str::to_string)).collect()
}
