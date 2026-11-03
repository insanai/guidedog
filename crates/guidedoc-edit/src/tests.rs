//! Tests of maps and edits on whole documents.

mod edit_markdown;
mod edit_rst;
mod map;
mod review;

use crate::{
    BlockId, BlockKind, Format, MapOptions, Operation, SourceMap, apply_operation, load_source_map,
};

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

/// Applies `op` and checks the new text; the edits must reproduce it.
pub(crate) fn edited(format: Format, text: &str, op: Operation<'_>) -> String {
    let options = MapOptions::new(format);
    let result = apply_operation(text, &options, &op).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(crate::apply_edits(text, &result.edits), result.text);
    for pair in result.edits.windows(2) {
        assert!(pair[0].end <= pair[1].start, "edits overlap: {:?}", result.edits);
    }
    result.text
}

/// Applies `op`, which must be refused with `code`.
pub(crate) fn refused(format: Format, text: &str, op: Operation<'_>, code: &str) {
    let options = MapOptions::new(format);
    match apply_operation(text, &options, &op) {
        Ok(result) => panic!("expected {code}, got:\n{}", result.text),
        Err(e) => assert_eq!(e.code, code, "{e}"),
    }
}

/// The map of `text`.
pub(crate) fn map_of(format: Format, text: &str) -> SourceMap {
    load_source_map(text, &MapOptions::new(format))
}
