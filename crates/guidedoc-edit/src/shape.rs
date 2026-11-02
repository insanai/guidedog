//! The shape of a map: what an edit must preserve. An edit is accepted only when the
//! edited text, read again, has exactly the shape the operation intends.

use std::ops::Range;

use crate::Format;
use crate::map::{BlockId, BlockKind, SourceMap};

/// One block of a shape: its depth, its kind, and a Markdown title's level (an RST
/// title's level is its depth, which the shape already holds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    pub depth: usize,
    pub kind: BlockKind,
    pub level: u8,
}

/// The shape of `map`, in the order of its blocks (a parent before its children).
pub(crate) fn shape(map: &SourceMap) -> Vec<Entry> {
    let mut depths = vec![0usize; map.blocks.len()];
    for (i, block) in map.blocks.iter().enumerate() {
        if let Some(parent) = block.parent {
            depths[i] = depths[parent.index()] + 1;
        }
    }
    let markdown = map.format == Format::Markdown;
    map.blocks
        .iter()
        .zip(depths)
        .map(|(b, depth)| Entry { depth, kind: b.kind, level: if markdown { b.level } else { 0 } })
        .collect()
}

/// The indices of block `id` and its descendants.
pub(crate) fn subtree(map: &SourceMap, id: BlockId) -> Range<usize> {
    let start = id.index();
    let mut end = start + 1;
    let mut stack = vec![id];
    while let Some(top) = stack.pop() {
        for &child in &map.blocks[top.index()].children {
            end = end.max(child.index() + 1);
            stack.push(child);
        }
    }
    start..end
}

/// `entries` with `depth` added to every depth.
pub(crate) fn shifted(entries: &[Entry], depth: usize) -> Vec<Entry> {
    entries.iter().map(|e| Entry { depth: e.depth + depth, ..*e }).collect()
}

/// `old` with `inserted` placed at `at`.
pub(crate) fn spliced(old: &[Entry], at: usize, inserted: &[Entry]) -> Vec<Entry> {
    let mut out = Vec::with_capacity(old.len() + inserted.len());
    out.extend_from_slice(&old[..at]);
    out.extend_from_slice(inserted);
    out.extend_from_slice(&old[at..]);
    out
}

/// `old` without the entries in `range`.
pub(crate) fn removed(old: &[Entry], range: Range<usize>) -> Vec<Entry> {
    let mut out = old.to_vec();
    out.drain(range);
    out
}
