//! Operations on a mapped document and the edits they make.

use crate::error::EditError;
use crate::lines::line_start;
use crate::map::{Block, BlockId, BlockKind, SourceMap, load_source_map};
use crate::shape::{self, Entry};
use crate::{Format, MapOptions, Span, ops, options};

/// Where a new or moved block goes, relative to an anchor block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Position {
    /// Before the anchor, as its previous sibling.
    Before,
    /// After the anchor, as its next sibling.
    After,
}

/// One visual operation on a document. Block ids refer to the map of the same text read
/// with the same options ([`load_source_map`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation<'a> {
    /// Replaces the editable text of a block (a paragraph, title, caption, term, field
    /// name, comment, literal block or leaf directive's content, or a list item's first
    /// paragraph). Lines are joined by `\n`; indentation and an RST title's adornment
    /// are written for the author.
    ReplaceText {
        /// The block.
        block: BlockId,
        /// Its new text, without markup of the block itself.
        text: &'a str,
    },
    /// Inserts new blocks, written as markup at column 0, before or after a block, at
    /// the anchor's indentation and separated by blank lines.
    InsertBlock {
        /// The block the new ones go next to.
        anchor: BlockId,
        /// Before or after it.
        position: Position,
        /// The new blocks' markup, such as `".. note::\n\n   Text."`.
        markup: &'a str,
    },
    /// Deletes a block with the blank lines that separate it from its next sibling.
    DeleteBlock {
        /// The block.
        block: BlockId,
    },
    /// Moves a block, with its children, next to another block.
    MoveBlock {
        /// The block to move.
        block: BlockId,
        /// The block it goes next to.
        anchor: BlockId,
        /// Before or after it.
        position: Position,
    },
    /// Sets a directive option: replaces its value, or adds the option.
    SetOption {
        /// The block written as a directive.
        block: BlockId,
        /// The option's name, such as `caption`.
        name: &'a str,
        /// Its value; empty for a flag.
        value: &'a str,
    },
    /// Removes a directive option, with its value lines.
    RemoveOption {
        /// The block written as a directive.
        block: BlockId,
        /// The option's name.
        name: &'a str,
    },
    /// Inserts a list item with the same marker style next to another item; an RST list
    /// numbered explicitly is renumbered after it.
    InsertItem {
        /// The item the new one goes next to.
        anchor: BlockId,
        /// Before or after it.
        position: Position,
        /// The new item's paragraph text.
        text: &'a str,
    },
}

/// The kinds of [`Operation`], by stable name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationKind {
    /// [`Operation::ReplaceText`]: `replace-text`.
    ReplaceText,
    /// [`Operation::InsertBlock`]: `insert-block`.
    InsertBlock,
    /// [`Operation::DeleteBlock`]: `delete-block`.
    DeleteBlock,
    /// [`Operation::MoveBlock`]: `move-block`.
    MoveBlock,
    /// [`Operation::SetOption`]: `set-option`.
    SetOption,
    /// [`Operation::RemoveOption`]: `remove-option`.
    RemoveOption,
    /// [`Operation::InsertItem`]: `insert-item`.
    InsertItem,
}

impl OperationKind {
    /// Every kind, in declaration order.
    pub const ALL: [OperationKind; 7] = [
        OperationKind::ReplaceText,
        OperationKind::InsertBlock,
        OperationKind::DeleteBlock,
        OperationKind::MoveBlock,
        OperationKind::SetOption,
        OperationKind::RemoveOption,
        OperationKind::InsertItem,
    ];

    /// The stable name, such as `replace-text`.
    pub fn name(self) -> &'static str {
        match self {
            OperationKind::ReplaceText => "replace-text",
            OperationKind::InsertBlock => "insert-block",
            OperationKind::DeleteBlock => "delete-block",
            OperationKind::MoveBlock => "move-block",
            OperationKind::SetOption => "set-option",
            OperationKind::RemoveOption => "remove-option",
            OperationKind::InsertItem => "insert-item",
        }
    }
}

impl Operation<'_> {
    /// The operation's kind.
    pub fn kind(&self) -> OperationKind {
        match self {
            Operation::ReplaceText { .. } => OperationKind::ReplaceText,
            Operation::InsertBlock { .. } => OperationKind::InsertBlock,
            Operation::DeleteBlock { .. } => OperationKind::DeleteBlock,
            Operation::MoveBlock { .. } => OperationKind::MoveBlock,
            Operation::SetOption { .. } => OperationKind::SetOption,
            Operation::RemoveOption { .. } => OperationKind::RemoveOption,
            Operation::InsertItem { .. } => OperationKind::InsertItem,
        }
    }
}

/// One replacement in the old text: bytes `start..end` become `insert`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TextEdit {
    /// The first replaced byte.
    pub start: usize,
    /// One past the last replaced byte.
    pub end: usize,
    /// The new text.
    pub insert: String,
}

/// An accepted operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EditResult {
    /// The minimal replacements, in increasing order of position in the old text, not
    /// overlapping: every byte outside them is unchanged.
    pub edits: Vec<TextEdit>,
    /// The whole new text.
    pub text: String,
}

/// The operations the edits support in `format`: every kind, in both formats.
pub fn operations(format: Format) -> &'static [OperationKind] {
    match format {
        Format::Rst | Format::Markdown => &OperationKind::ALL,
    }
}

/// The operations that apply to block `id` of `map`, made from `text`: what an editor
/// may offer for it. An operation listed here may still be refused for a particular
/// new text or anchor.
pub fn block_operations(map: &SourceMap, text: &str, id: BlockId) -> Vec<OperationKind> {
    let Some(block) = map.block(id) else { return Vec::new() };
    let mut out = Vec::new();
    if ops::replaceable(block) {
        out.push(OperationKind::ReplaceText);
    }
    if block.kind != BlockKind::Title && starts_line(text, block, map.format) {
        out.extend([OperationKind::InsertBlock, OperationKind::DeleteBlock]);
        out.push(OperationKind::MoveBlock);
    }
    if block.directive.is_some() {
        out.extend([OperationKind::SetOption, OperationKind::RemoveOption]);
    }
    if block.kind == BlockKind::ListItem {
        out.push(OperationKind::InsertItem);
    }
    out
}

/// Tells whether only indentation (and Markdown quote markers) precedes the block on
/// its first line, so whole lines can be inserted, deleted or moved around it.
pub(crate) fn starts_line(text: &str, block: &Block, format: Format) -> bool {
    let prefix = &text[line_start(text, block.span.start)..block.span.start];
    prefix.chars().all(|c| c == ' ' || c == '\t' || (format == Format::Markdown && c == '>'))
}

/// Applies `op` to `text`, read as `options` say, and returns the minimal edits and the
/// new text. Fails without changing anything when the block is unknown or does not
/// support the operation (`edit.unknown-block`, `edit.unsupported`), when the new text
/// or option does not fit (`edit.invalid-text`, `edit.unknown-option`,
/// `edit.option-value`, `edit.missing-option`, `edit.renumber`), when the map of the
/// text or of the edited text lists only its first `options.max_blocks` blocks, so the
/// rest could not be checked (`edit.truncated`), or when the edited text, read again,
/// would not have the intended structure (`edit.unsafe`).
pub fn apply_operation(
    text: &str,
    options: &MapOptions<'_>,
    op: &Operation<'_>,
) -> Result<EditResult, EditError> {
    let map = load_source_map(text, options);
    let mut planned = plan(text, options, &map, op)?;
    if uses_crlf(text) {
        // The operations write `\n`; a file that ends its lines with `\r\n` keeps them.
        for edit in &mut planned.edits {
            edit.insert = edit.insert.replace("\r\n", "\n").replace('\n', "\r\n");
        }
    }
    if map.truncated {
        return Err(EditError::truncated(planned.target, options.max_blocks));
    }
    let new_text = apply_edits(text, &planned.edits);
    let after = load_source_map(&new_text, options);
    if after.truncated {
        return Err(EditError::truncated(planned.target, options.max_blocks));
    }
    if shape::shape(&after) != planned.expected || !(planned.check)(&after, &new_text) {
        return Err(EditError::unsafe_edit(planned.target));
    }
    let edits = minimize(text, planned.edits);
    Ok(EditResult { edits, text: new_text })
}

/// Tells whether `text` ends its lines with `\r\n`, judged by its first line break.
fn uses_crlf(text: &str) -> bool {
    text.find('\n').is_some_and(|at| at > 0 && text.as_bytes()[at - 1] == b'\r')
}

/// A further check of an edited text's map, beyond its shape.
pub(crate) type ResultCheck = Box<dyn Fn(&SourceMap, &str) -> bool>;

/// What an operation will do: its edits, the shape the result must have, a further
/// check of the result, and the block it concerns (for a refusal to point at).
pub(crate) struct Plan {
    pub edits: Vec<TextEdit>,
    pub expected: Vec<Entry>,
    pub check: ResultCheck,
    pub target: Span,
}

impl Plan {
    /// A plan whose result must have shape `expected` and nothing more.
    pub(crate) fn new(edits: Vec<TextEdit>, expected: Vec<Entry>, target: Span) -> Plan {
        Plan { edits, expected, check: Box::new(|_, _| true), target }
    }
}

fn plan(
    text: &str,
    options: &MapOptions<'_>,
    map: &SourceMap,
    op: &Operation<'_>,
) -> Result<Plan, EditError> {
    let cx = ops::Cx { text, options, map };
    match *op {
        Operation::ReplaceText { block, text: new } => ops::replace_text(&cx, block, new),
        Operation::InsertBlock { anchor, position, markup } => {
            ops::insert_block(&cx, anchor, position, markup)
        }
        Operation::DeleteBlock { block } => ops::delete_block(&cx, block),
        Operation::MoveBlock { block, anchor, position } => {
            ops::move_block(&cx, block, anchor, position)
        }
        Operation::SetOption { block, name, value } => {
            options::set_option(&cx, block, name, Some(value))
        }
        Operation::RemoveOption { block, name } => options::set_option(&cx, block, name, None),
        Operation::InsertItem { anchor, position, text: new } => {
            ops::insert_item(&cx, anchor, position, new)
        }
    }
}

/// Applies non-overlapping edits given in increasing order of position.
pub fn apply_edits(text: &str, edits: &[TextEdit]) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut at = 0;
    for edit in edits {
        out.push_str(&text[at..edit.start]);
        out.push_str(&edit.insert);
        at = edit.end;
    }
    out.push_str(&text[at..]);
    out
}

/// Each edit without the bytes it would write unchanged at its ends; edits that change
/// nothing are dropped.
fn minimize(text: &str, edits: Vec<TextEdit>) -> Vec<TextEdit> {
    let mut out = Vec::with_capacity(edits.len());
    for edit in edits {
        let old = &text[edit.start..edit.end];
        let new = edit.insert.as_str();
        let prefix = common_prefix(old, new);
        let suffix = common_suffix(&old[prefix..], &new[prefix..]);
        if prefix == old.len() && prefix == new.len() {
            continue;
        }
        out.push(TextEdit {
            start: edit.start + prefix,
            end: edit.end - suffix,
            insert: new[prefix..new.len() - suffix].to_string(),
        });
    }
    out
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.char_indices()
        .zip(b.chars())
        .find(|((_, x), y)| x != y)
        .map_or(a.len().min(b.len()), |((i, _), _)| i)
        .min(b.len())
}

fn common_suffix(a: &str, b: &str) -> usize {
    let mut n = 0;
    for (x, y) in a.chars().rev().zip(b.chars().rev()) {
        if x != y {
            break;
        }
        n += x.len_utf8();
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimize_keeps_only_changed_bytes() {
        let text = "Hello brave world";
        let edits = vec![TextEdit { start: 0, end: 17, insert: "Hello bold world".into() }];
        let small = minimize(text, edits);
        assert_eq!(small, vec![TextEdit { start: 7, end: 11, insert: "old".into() }]);
        assert_eq!(apply_edits(text, &small), "Hello bold world");
        let same = vec![TextEdit { start: 0, end: 5, insert: "Hello".into() }];
        assert!(minimize(text, same).is_empty());
        let wide = vec![TextEdit { start: 0, end: 2, insert: "\u{e9}e".into() }];
        assert_eq!(minimize("ee", wide)[0].insert, "\u{e9}");
    }
}
