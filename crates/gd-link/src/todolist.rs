//! sphinx.ext.todo's todolist (todolist.odin): every todo of the project, copied from
//! its document's object, each followed by a line that links to the original.

use gd_env::{DocId, TodoInfo};
use gd_resolve::{BuildView, Resolved};
use guidedoc::{AttributeKey, Document, Fail, LinkKind, NodeId, NodeKind, SourceSpan};

use crate::page::Page;
use crate::rewrite::Rewriter;
use crate::xref::link_destination;

/// Replaces a todolist directive by the project's todos, in reading order.
pub(crate) fn replace_todolist(
    page: &mut Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    id: NodeId,
) -> Result<(), Fail> {
    let span = rw.doc.node(id).span;
    // The spare workspace is taken while a document is loaded into it, and given back.
    let Some(spare) = page.spare.take() else { return Ok(()) };
    let mut result = Ok(());
    for doc_id in todo_order(&page.view) {
        let info = page.view.env.doc(doc_id);
        let Some(object) = &info.object else { continue };
        let other = match page.view.objects.load_object(spare, object, page.view.limits) {
            Ok(other) => other,
            Err(e) => {
                page.reports.push(e.problem.report());
                continue;
            }
        };
        result = copy_todos(page, rw, other, doc_id, &info.todos, span);
        if result.is_err() {
            break;
        }
    }
    page.spare = Some(spare);
    result
}

/// Copies a document's todos, each with its source line.
fn copy_todos(
    page: &Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    other: Document<'_>,
    doc_id: DocId,
    todos: &[TodoInfo],
    span: SourceSpan,
) -> Result<(), Fail> {
    for todo in todos {
        if todo.node as usize >= other.node_count() {
            continue;
        }
        copy_foreign(rw, other, NodeId(todo.node), span)?;
        todo_source(page, rw, doc_id, todo, span)?;
    }
    Ok(())
}

/// The documents with todos: the toctree's reading order first, then the documents no
/// toctree reaches, by name. Documents that failed have no object to copy from.
pub fn todo_order(view: &BuildView<'_>) -> Vec<DocId> {
    let mut seen = vec![false; view.env.docs.len()];
    let mut order = Vec::new();
    for &id in &view.tables.order {
        if let Some(slot) = seen.get_mut(id.index()).filter(|s| !**s) {
            *slot = true;
            order.push(id);
        }
    }
    order.extend(view.env.iter().map(|(id, _)| id).filter(|id| !seen[id.index()]));
    order.retain(|&id| {
        let d = view.env.doc(id);
        !d.todos.is_empty() && !d.failed
    });
    order
}

/// Copies a subtree from another document. Ids stay behind, since the original keeps
/// them; internal links and note references, whose targets are on the other page,
/// become their content.
fn copy_foreign(
    rw: &mut Rewriter<'_, '_, '_>,
    other: Document<'_>,
    id: NodeId,
    span: SourceSpan,
) -> Result<(), Fail> {
    match other.kind(id) {
        NodeKind::FootnoteReference
        | NodeKind::CitationReference
        | NodeKind::Target
        | NodeKind::Index => return Ok(()),
        NodeKind::Link | NodeKind::Reference
            if other.link_info(id).is_some_and(|l| l.kind == LinkKind::Internal) =>
        {
            return copy_foreign_children(rw, other, id, span);
        }
        _ => {}
    }
    let n = other.node(id);
    let copy = rw.b.begin_node(n.kind, span)?;
    rw.b.set_value(copy, n.value);
    rw.b.add_flags(copy, n.flags);
    let text = rw.b.add_text(other.node_text(id))?;
    rw.b.set_text(copy, text);
    for a in other.attributes(id) {
        let name = other.text(a.name);
        if a.key == AttributeKey::Id || (a.key == AttributeKey::Custom && name == "name") {
            continue;
        }
        let value = rw.b.add_text(other.text(a.value))?;
        let name = rw.b.add_text(name)?;
        rw.b.add_attribute_named(copy, a.key, value, name)?;
    }
    rw.b.copy_payload(other, id, copy)?;
    copy_foreign_children(rw, other, id, span)?;
    rw.b.end_node(copy)
}

fn copy_foreign_children(
    rw: &mut Rewriter<'_, '_, '_>,
    other: Document<'_>,
    id: NodeId,
    span: SourceSpan,
) -> Result<(), Fail> {
    for child in other.children(id) {
        copy_foreign(rw, other, child, span)?;
    }
    Ok(())
}

/// Writes "(The original entry is located in FILE, line N.)".
fn todo_source(
    page: &Page<'_>,
    rw: &mut Rewriter<'_, '_, '_>,
    doc_id: DocId,
    todo: &TodoInfo,
    span: SourceSpan,
) -> Result<(), Fail> {
    let para = rw.open(NodeKind::Paragraph, span)?;
    rw.add_class(para, "todo-source")?;
    rw.add_plain("(The ", span)?;
    let to = Resolved { doc: Some(doc_id), anchor: todo.anchor.clone(), ..Resolved::default() };
    let (destination, kind) = link_destination(page, &to);
    let link = rw.open_link(&destination, "", kind, span)?;
    let em = rw.open(NodeKind::Emphasis, span)?;
    rw.add_plain("original entry", span)?;
    rw.close(em)?;
    rw.close(link)?;
    let relative = &page.view.env.doc(doc_id).relative;
    rw.add_plain(&format!(" is located in {relative}, line {}.)", todo.line), span)?;
    rw.close(para)
}
