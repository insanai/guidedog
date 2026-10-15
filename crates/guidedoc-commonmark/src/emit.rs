//! Emission: the finished skeleton becomes nodes, in preorder, with inline content
//! parsed per paragraph and heading.

use guidedoc::{AttributeKey, Fail, ListInfo, ListStyle, NodeFlags, NodeId, NodeKind, SourceSpan};

use crate::blocks::{Bid, BlockKind, Skeleton, is_blank};
use crate::host::Host;
use crate::inline::entities::push_unescaped;
use crate::inline::{parse_leaf, parse_leaf_range};
use crate::settings::SyntaxExtension;

/// Builds the nodes of a finished skeleton in preorder.
pub(crate) fn emit_document(h: &mut Host<'_, '_>, sk: &Skeleton<'_>) -> Result<(), Fail> {
    if h.myst && sk.sections && sk.main && h.settings.heading_anchors > 0 {
        crate::myst::prescan_headings(h, sk)?;
    }
    for id in sk.children(0) {
        // parse_body's classes go to every top-level element (the class directive).
        if let Some(each) = h.ms.each {
            h.ms.pending = Some(each);
        }
        emit_block(h, sk, id, true)?;
    }
    Ok(())
}

/// Opens a block node, giving it the classes a class directive left.
pub(crate) fn open_block(
    h: &mut Host<'_, '_>,
    kind: NodeKind,
    span: SourceSpan,
) -> Result<NodeId, Fail> {
    let id = h.b().begin_node(kind, span)?;
    if let Some(classes) = h.ms.pending.take() {
        h.b().add_attribute(id, AttributeKey::Class, classes)?;
    }
    Ok(id)
}

/// Emits a block and its children; `top` marks a top-level block of the parse.
pub(crate) fn emit_block(
    h: &mut Host<'_, '_>,
    sk: &Skeleton<'_>,
    id: Bid,
    top: bool,
) -> Result<(), Fail> {
    h.spend(1)?;
    match sk.blocks[id].kind {
        BlockKind::Item => emit_item(h, sk, id),
        BlockKind::BlockQuote | BlockKind::List | BlockKind::Definition | BlockKind::FieldList => {
            let node = open_container(h, sk, id)?;
            emit_children(h, sk, id)?;
            h.b().end_node(node)
        }
        BlockKind::DefinitionList => crate::myst::emit_definition_list(h, sk, id),
        BlockKind::Field => crate::myst::emit_field(h, sk, id),
        BlockKind::FootnoteDef => Ok(()),
        _ => emit_leaf(h, sk, id, top),
    }
}

pub(crate) fn emit_children(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    for c in sk.children(id) {
        emit_block(h, sk, c, false)?;
    }
    Ok(())
}

fn open_container(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<NodeId, Fail> {
    let blk = sk.blocks[id];
    let span = sk.block_span(id);
    match blk.kind {
        BlockKind::BlockQuote => open_block(h, NodeKind::BlockQuote, span),
        BlockKind::Definition => h.b().begin_node(NodeKind::Definition, span),
        BlockKind::FieldList => {
            let node = open_block(h, NodeKind::FieldList, span)?;
            if h.myst {
                h.b().add_attribute_string(node, AttributeKey::Class, "myst")?;
            }
            Ok(node)
        }
        _ => {
            let node = open_block(h, NodeKind::List, span)?;
            let style = if blk.ordered { ListStyle::Arabic } else { ListStyle::Bullet };
            let start = if blk.ordered { blk.list_start } else { 1 };
            if h.has(SyntaxExtension::Tasklist) && crate::myst::task_list(sk, id) {
                h.b().add_attribute_string(node, AttributeKey::Class, "contains-task-list")?;
            }
            h.b().set_list(node, ListInfo { style, delimiter: blk.marker, start })?;
            if blk.tight {
                h.b().add_flags(node, NodeFlags::TIGHT);
            }
            Ok(node)
        }
    }
}

/// Emits a list item; in a task list ("tasklist") an item that starts with "[ ]" or
/// "[x]" gets class task-list-item and a checkbox before its text.
fn emit_item(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    let node = h.b().begin_node(NodeKind::ListItem, sk.block_span(id))?;
    let mut state = 0;
    if h.has(SyntaxExtension::Tasklist) {
        state = crate::myst::task_state(sk, id);
    }
    if state > 0 {
        h.b().add_attribute_string(node, AttributeKey::Class, "task-list-item")?;
    }
    let saved = std::mem::replace(&mut h.ms.task, state);
    let result = emit_children(h, sk, id);
    h.ms.task = saved;
    result?;
    h.b().end_node(node)
}

fn emit_leaf(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid, top: bool) -> Result<(), Fail> {
    let blk = sk.blocks[id];
    let span = sk.block_span(id);
    match blk.kind {
        BlockKind::Paragraph => emit_paragraph(h, sk, id),
        BlockKind::Heading if h.myst => crate::myst::emit_heading(h, sk, id, top),
        BlockKind::Heading => {
            let node = h.b().begin_node(NodeKind::Heading, span)?;
            h.b().set_value(node, i32::from(blk.level));
            parse_leaf(h, sk, id)?;
            h.b().end_node(node)
        }
        BlockKind::ThematicBreak => {
            let node = open_block(h, NodeKind::ThematicBreak, span)?;
            h.b().end_node(node)
        }
        BlockKind::HtmlBlock => {
            if h.myst && crate::myst::emit_html_extension(h, sk, id)? {
                return Ok(());
            }
            let format = h.html_format()?;
            let node = open_block(h, NodeKind::RawBlock, span)?;
            let text = h.b().add_text(sk.content(id))?;
            h.b().set_text(node, text);
            h.b().add_attribute(node, AttributeKey::Format, format)?;
            h.b().end_node(node)
        }
        BlockKind::CodeBlock => {
            if h.myst && blk.fenced && crate::myst::is_directive_fence(h, sk, id) {
                return crate::myst::run_fence_directive(h, sk, id, top);
            }
            emit_code(h, sk, id)
        }
        _ => crate::myst::emit_leaf(h, sk, id),
    }
}

fn emit_paragraph(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    if is_blank(sk.content(id)) {
        return Ok(());
    }
    if h.myst && crate::myst::block_substitution(h, sk, id)? {
        return Ok(());
    }
    let span = sk.block_span(id);
    let node = open_block(h, NodeKind::Paragraph, span)?;
    let task = std::mem::take(&mut h.ms.task);
    if task > 0 {
        crate::myst::task_checkbox(h, task == 2, span)?;
        let blk = sk.blocks[id];
        parse_leaf_range(h, sk, id, blk.content_start + 3, blk.content_end)?;
    } else {
        parse_leaf(h, sk, id)?;
    }
    h.b().end_node(node)
}

fn emit_code(h: &mut Host<'_, '_>, sk: &Skeleton<'_>, id: Bid) -> Result<(), Fail> {
    let blk = sk.blocks[id];
    let node = open_block(h, NodeKind::CodeBlock, sk.block_span(id))?;
    if blk.fenced && blk.info_end > blk.info_start {
        let mut info = String::new();
        push_unescaped(&mut info, sk.arena_str(blk.info_start, blk.info_end));
        let word = info.find(|c: char| c.is_ascii_whitespace()).unwrap_or(info.len());
        let b = h.b();
        let text = b.add_text(&info)?;
        if word > 0 {
            let language = guidedoc::Text { start: text.start, len: word as u32 };
            b.add_attribute(node, AttributeKey::Language, language)?;
        }
        if word < info.len() {
            let name = b.add_text("info")?;
            b.add_attribute_named(node, AttributeKey::Custom, text, name)?;
        }
    } else if h.myst && h.settings.extension.is_some() {
        // As Sphinx does for MyST: fences use the highlighting default, indented code none.
        let language = if blk.fenced { "default" } else { "none" };
        h.b().add_attribute_string(node, AttributeKey::Language, language)?;
    }
    let text = h.b().add_text(sk.content(id))?;
    h.b().set_text(node, text);
    h.b().end_node(node)
}
