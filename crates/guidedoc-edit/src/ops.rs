//! The block operations: replacing text, inserting, deleting and moving blocks, and
//! inserting list items. Each returns a plan: the edits and the shape the edited text
//! must have when read again.

use crate::edit::{Plan, Position, TextEdit, apply_edits, starts_line};
use crate::error::EditError;
use crate::lines::{
    continuation, is_blank, line_at, line_end_trimmed, line_start, next_line, skip_blank_lines,
};
use crate::map::{Block, BlockId, BlockKind, SourceMap, load_source_map};
use crate::shape::{self, shifted, spliced};
use crate::walk::is_adornment;
use crate::{Format, MapOptions, Span};

/// What every operation reads: the text, how it is read, and its map.
pub(crate) struct Cx<'a> {
    pub text: &'a str,
    pub options: &'a MapOptions<'a>,
    pub map: &'a SourceMap,
}

impl Cx<'_> {
    fn block(&self, id: BlockId) -> Result<&Block, EditError> {
        self.map.block(id).ok_or_else(EditError::unknown_block)
    }

    fn markdown(&self) -> bool {
        self.map.format == Format::Markdown
    }

    /// The depth of block `id`: 0 at the top level.
    fn depth(&self, id: BlockId) -> usize {
        let mut depth = 0;
        let mut at = self.map.blocks[id.index()].parent;
        while let Some(parent) = at {
            depth += 1;
            at = self.map.blocks[parent.index()].parent;
        }
        depth
    }
}

/// Tells whether a block has text [`replace_text`] edits.
pub(crate) fn replaceable(block: &Block) -> bool {
    let kinds = matches!(
        block.kind,
        BlockKind::Paragraph
            | BlockKind::Title
            | BlockKind::Caption
            | BlockKind::DefinitionItem
            | BlockKind::Field
            | BlockKind::Comment
            | BlockKind::LiteralBlock
            | BlockKind::Directive
            | BlockKind::ListItem
    );
    kinds && !block.content.is_empty()
}

/// The text `text` with `\r\n` as `\n` and without white space at its end.
fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_string()
}

pub(crate) fn replace_text(cx: &Cx<'_>, id: BlockId, new: &str) -> Result<Plan, EditError> {
    let block = cx.block(id)?;
    if !replaceable(block) {
        return Err(EditError::unsupported(
            &format!("the text of a {}", block.kind.name()),
            block.span,
        ));
    }
    if block.kind == BlockKind::ListItem {
        // An item's text is its first paragraph's.
        let first = block.children.first().copied().ok_or_else(EditError::unknown_block)?;
        return replace_text(cx, first, new);
    }
    let new = normalized(new);
    check_text(block, &new)?;
    let first = block.content[0];
    let last = block.content[block.content.len() - 1];
    let prefix = &cx.text[line_start(cx.text, first.start)..first.start];
    let code = matches!(block.kind, BlockKind::LiteralBlock | BlockKind::Directive);
    let indent = if code { prefix.to_string() } else { continuation(prefix, cx.markdown()) };
    let mut edits = Vec::new();
    let title = block.kind == BlockKind::Title && cx.map.format == Format::Rst;
    let insert = indent_lines(&new, &indent);
    if title {
        edits.extend(adornment_edit(cx.text, block, first, &new, true));
    }
    edits.push(TextEdit { start: first.start, end: last.end, insert });
    if title {
        edits.extend(adornment_edit(cx.text, block, first, &new, false));
    }
    Ok(Plan::new(edits, shape::shape(cx.map), block.span))
}

/// Refuses text that cannot be the block's: empty text, a blank line in inline text,
/// or a line break in a one-line block.
fn check_text(block: &Block, new: &str) -> Result<(), EditError> {
    if new.trim().is_empty() {
        return Err(EditError::invalid_text("is empty", block.span));
    }
    let one_line =
        matches!(block.kind, BlockKind::Title | BlockKind::DefinitionItem | BlockKind::Field);
    if one_line && new.contains('\n') {
        return Err(EditError::invalid_text("has a line break in a one-line block", block.span));
    }
    let inline = !matches!(block.kind, BlockKind::LiteralBlock | BlockKind::Directive);
    if inline && new.split('\n').any(is_blank) {
        return Err(EditError::invalid_text(
            "has a blank line, which would split the block",
            block.span,
        ));
    }
    Ok(())
}

/// `text` with every line after the first indented by `indent`; blank lines keep only
/// the quote markers of `indent`, so no line ends in white space.
fn indent_lines(text: &str, indent: &str) -> String {
    let blank = indent.trim_end();
    let mut out = String::with_capacity(text.len() + 16);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(if is_blank(line) { blank } else { indent });
        }
        out.push_str(if is_blank(line) { "" } else { line });
    }
    out
}

/// The edit of an RST title's overline (`over`) or underline: it keeps its character
/// and takes the width of the title line with the new title, unless it was already
/// longer than the old title and still covers the new one.
fn adornment_edit(
    text: &str,
    block: &Block,
    title: Span,
    new: &str,
    over: bool,
) -> Option<TextEdit> {
    let line = line_start(text, title.start);
    let at = if over {
        (block.span.start < line).then_some(block.span.start)?
    } else {
        let under = next_line(text, line);
        (under > line && under < block.span.end).then_some(under)?
    };
    let rule = line_at(text, at);
    if !is_adornment(rule) {
        return None;
    }
    let lead = guidedoc_rst::column_width(&text[line..title.start]);
    let trail = guidedoc_rst::column_width(&text[title.end..line_end_trimmed(text, title.end)]);
    let width = (lead + guidedoc_rst::column_width(new) + trail).max(2);
    let old_width = lead + guidedoc_rst::column_width(title.text(text)) + trail;
    let rule_width = rule.trim_end().chars().count();
    // An adornment longer than its title is the author's style: keep it while it
    // still covers the title. One that fit the title fits the new title.
    if rule_width != old_width && rule_width >= width {
        return None;
    }
    let mark = rule.chars().next().unwrap_or('=');
    Some(TextEdit {
        start: at,
        end: at + rule.trim_end().len(),
        insert: mark.to_string().repeat(width),
    })
}

/// The indentation of a block's lines: what precedes it on its first line, which must
/// be white space (and quote markers in Markdown).
fn block_indent(cx: &Cx<'_>, block: &Block) -> Result<String, EditError> {
    if !starts_line(cx.text, block, cx.map.format) {
        return Err(EditError::unsupported(
            "a block that shares its first line with another",
            block.span,
        ));
    }
    Ok(cx.text[line_start(cx.text, block.span.start)..block.span.start].to_string())
}

/// The separator between blocks at `indent`: an empty line, or the quote markers alone.
fn separator(indent: &str) -> String {
    format!("\n{}\n", indent.trim_end())
}

/// The root shapes of `markup` read on its own, at depth `depth`.
fn markup_shape(cx: &Cx<'_>, markup: &str, depth: usize) -> Vec<shape::Entry> {
    shifted(&shape::shape(&load_source_map(markup, cx.options)), depth)
}

pub(crate) fn insert_block(
    cx: &Cx<'_>,
    anchor: BlockId,
    position: Position,
    markup: &str,
) -> Result<Plan, EditError> {
    let block = cx.block(anchor)?;
    let markup = normalized(markup);
    if markup.trim().is_empty() {
        return Err(EditError::invalid_text("is empty", block.span));
    }
    let edit = insertion(cx, block, position, &markup)?;
    let old = shape::shape(cx.map);
    let at = match position {
        Position::Before => anchor.index(),
        Position::After => shape::subtree(cx.map, anchor).end,
    };
    let expected = spliced(&old, at, &markup_shape(cx, &markup, cx.depth(anchor)));
    Ok(Plan::new(vec![edit], expected, block.span))
}

/// The edit that writes `markup` next to `block`.
fn insertion(
    cx: &Cx<'_>,
    block: &Block,
    position: Position,
    markup: &str,
) -> Result<TextEdit, EditError> {
    if block.kind == BlockKind::Title {
        return Err(EditError::unsupported("a block next to a title", block.span));
    }
    let indent = block_indent(cx, block)?;
    let body = indent_lines(markup, &indent);
    Ok(match position {
        Position::Before => {
            let at = line_start(cx.text, block.span.start);
            TextEdit {
                start: at + indent.len(),
                end: at + indent.len(),
                insert: format!("{body}{}{indent}", separator(&indent)),
            }
        }
        Position::After => {
            let at = block.span.end;
            TextEdit {
                start: at,
                end: at,
                insert: format!("{}{indent}{body}", separator(&indent)),
            }
        }
    })
}

pub(crate) fn delete_block(cx: &Cx<'_>, id: BlockId) -> Result<Plan, EditError> {
    let block = cx.block(id)?;
    let edit = deletion(cx, id)?;
    let expected = shape::removed(&shape::shape(cx.map), shape::subtree(cx.map, id));
    Ok(Plan::new(vec![edit], expected, block.span))
}

/// The edit that removes block `id`: its lines and the blank lines after them, or,
/// for the last block of its parent, the blank lines before them.
fn deletion(cx: &Cx<'_>, id: BlockId) -> Result<TextEdit, EditError> {
    let block = cx.block(id)?;
    if block.kind == BlockKind::Title {
        return Err(EditError::unsupported("deleting a title alone", block.span));
    }
    block_indent(cx, block)?;
    let text = cx.text;
    let start = line_start(text, block.span.start);
    let next = skip_blank_lines(text, block.span.end);
    let limit = block.parent.map_or(text.trim_end().len(), |p| cx.map.blocks[p.index()].span.end);
    if next < limit {
        return Ok(TextEdit { start, end: next, insert: String::new() });
    }
    let before = text[..start].trim_end().len();
    let parent_start = block.parent.map_or(0, |p| cx.map.blocks[p.index()].span.start);
    if before <= parent_start {
        // The only block of its parent: remove its lines.
        return Ok(TextEdit {
            start,
            end: next_line(text, block.span.end).max(block.span.end),
            insert: String::new(),
        });
    }
    Ok(TextEdit { start: before, end: block.span.end, insert: String::new() })
}

pub(crate) fn move_block(
    cx: &Cx<'_>,
    id: BlockId,
    anchor: BlockId,
    position: Position,
) -> Result<Plan, EditError> {
    let block = cx.block(id)?;
    cx.block(anchor)?;
    let moved = shape::subtree(cx.map, id);
    if moved.contains(&anchor.index()) {
        return Err(EditError::unsupported(
            "moving a block next to itself or into itself",
            block.span,
        ));
    }
    let markup = dedented(cx, block)?;
    let delete = deletion(cx, id)?;
    let removed_text = apply_edits(cx.text, std::slice::from_ref(&delete));
    let map1 = load_source_map(&removed_text, cx.options);
    let shift = |i: usize| if i > moved.start { i - moved.len() } else { i };
    let anchor1 = BlockId(shift(anchor.index()) as u32);
    let cx1 = Cx { text: &removed_text, options: cx.options, map: &map1 };
    let insert = insertion(&cx1, cx1.block(anchor1)?, position, &markup)?;
    let at = if insert.start > delete.start {
        insert.start + delete.end - delete.start
    } else {
        insert.start
    };
    let edits = combine(delete, TextEdit { start: at, end: at, ..insert });
    let old = shape::shape(cx.map);
    let rest = shape::removed(&old, moved.clone());
    let anchor_depth = cx.depth(anchor);
    let entries = shifted(&shifted_back(&old[moved.clone()], cx.depth(id)), anchor_depth);
    let at = match position {
        Position::Before => shift(anchor.index()),
        Position::After => shift(shape::subtree(cx.map, anchor).end),
    };
    Ok(Plan::new(edits, spliced(&rest, at, &entries), block.span))
}

fn shifted_back(entries: &[shape::Entry], depth: usize) -> Vec<shape::Entry> {
    entries.iter().map(|e| shape::Entry { depth: e.depth - depth, ..*e }).collect()
}

/// A deletion and an insertion as edits in increasing order, merged where they meet.
fn combine(delete: TextEdit, insert: TextEdit) -> Vec<TextEdit> {
    if insert.start == delete.start || insert.start == delete.end {
        return vec![TextEdit { start: delete.start, end: delete.end, insert: insert.insert }];
    }
    let mut edits = vec![delete, insert];
    edits.sort_by_key(|e| e.start);
    edits
}

/// The block's lines without its indentation, as markup at column 0.
fn dedented(cx: &Cx<'_>, block: &Block) -> Result<String, EditError> {
    let indent = block_indent(cx, block)?;
    let lines = cx.text[block.span.start..block.span.end].split('\n');
    let mut out = Vec::new();
    for (i, line) in lines.enumerate() {
        let line = line.trim_end_matches('\r');
        match line.strip_prefix(indent.as_str()) {
            _ if i == 0 => out.push(line),
            Some(rest) => out.push(rest),
            None if is_blank(line) || line.trim_end() == indent.trim_end() => out.push(""),
            None => {
                return Err(EditError::unsupported(
                    "moving a block with irregular indentation",
                    block.span,
                ));
            }
        }
    }
    Ok(out.join("\n"))
}

pub(crate) fn insert_item(
    cx: &Cx<'_>,
    anchor: BlockId,
    position: Position,
    new: &str,
) -> Result<Plan, EditError> {
    let block = cx.block(anchor)?;
    if block.kind != BlockKind::ListItem {
        return Err(EditError::unsupported(
            &format!("an item next to a {}", block.kind.name()),
            block.span,
        ));
    }
    let new = normalized(new);
    if new.trim().is_empty() || new.split('\n').any(is_blank) {
        return Err(EditError::invalid_text("is empty or has a blank line", block.span));
    }
    let edits = crate::items::item_edits(cx.text, cx.map, anchor, position, &new, cx.markdown())?;
    let depth = cx.depth(anchor);
    let item = [
        shape::Entry { depth, kind: BlockKind::ListItem, level: 0 },
        shape::Entry { depth: depth + 1, kind: BlockKind::Paragraph, level: 0 },
    ];
    let at = match position {
        Position::Before => anchor.index(),
        Position::After => shape::subtree(cx.map, anchor).end,
    };
    let expected = spliced(&shape::shape(cx.map), at, &item);
    Ok(Plan::new(edits, expected, block.span))
}
