//! Tests of maps and edits on whole documents.

mod map;

use crate::{BlockId, BlockKind, Format, MapOptions, SourceMap, load_source_map};

/// The ids of the blocks of `kind`, in document order.
pub(crate) fn all(map: &SourceMap, kind: BlockKind) -> Vec<BlockId> {
    (0..map.blocks.len())
        .map(|i| BlockId(i as u32))
        .filter(|&id| map.blocks[id.index()].kind == kind)
        .collect()
}

/// The kinds of the top-level blocks and their children, as `kind(child kinds...)`.
pub(crate) fn outline(map: &SourceMap) -> String {
    fn walk(map: &SourceMap, ids: &[BlockId], out: &mut Vec<String>) {
        for &id in ids {
            let b = &map.blocks[id.index()];
            let mut inner = Vec::new();
            walk(map, &b.children, &mut inner);
            let name = b.kind.name();
            out.push(if inner.is_empty() {
                name.to_string()
            } else {
                format!("{name}({})", inner.join(" "))
            });
        }
    }
    let mut out = Vec::new();
    walk(map, &map.roots, &mut out);
    out.join(" ")
}

/// The map of `text`.
pub(crate) fn map_of(format: Format, text: &str) -> SourceMap {
    load_source_map(text, &MapOptions::new(format))
}
