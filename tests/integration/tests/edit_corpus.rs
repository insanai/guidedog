//! Source maps over Guidedog's own documentation: every reStructuredText and Markdown
//! file of `docs/` maps to well-formed blocks that cover every non-blank line, and
//! replacing a block's text with the same text is accepted and changes no byte.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

use std::fs;
use std::path::{Path, PathBuf};

use guidedoc_edit::{
    BlockId, Format, MapOptions, Operation, SourceMap, apply_operation, block_operations,
    load_source_map,
};

/// The documents under `dir`, skipping build output and translations.
fn documents(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.is_dir() {
            if !name.starts_with('_') && name != "locales" {
                documents(&path, out);
            }
        } else if Format::of_path(&name).is_some() && !name.ends_with(".txt") {
            out.push(path);
        }
    }
}

/// Spans lie inside the text and inside their parent, siblings do not overlap, and
/// content lies inside its block.
fn check_well_formed(map: &SourceMap, text: &str, path: &Path) {
    let check_level = |ids: &[BlockId], outer: (usize, usize)| {
        let mut end = outer.0;
        for id in ids {
            let b = &map.blocks[id.index()];
            assert!(b.span.start >= end && b.span.end <= outer.1, "{path:?}: {b:?}");
            assert!(text.is_char_boundary(b.span.start) && text.is_char_boundary(b.span.end));
            for c in &b.content {
                assert!(c.start >= b.span.start && c.end <= b.span.end, "{path:?}: {b:?}");
            }
            end = b.span.end;
        }
    };
    check_level(&map.roots, (0, text.len()));
    for b in &map.blocks {
        check_level(&b.children, (b.span.start, b.span.end));
    }
    // Every non-blank line belongs to a top-level block.
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let lead = line.len() - line.trim_start().len();
        if !line.trim().is_empty() {
            let offset = at + lead;
            let covered = map.roots.iter().any(|id| {
                let s = map.blocks[id.index()].span;
                s.start <= offset && offset < s.end
            });
            assert!(covered, "{path:?}: line at byte {offset} is in no block: {line:?}");
        }
        at += line.len();
    }
}

#[test]
fn documentation_maps_and_identity_edits_change_nothing() {
    let root = guidedog_tests::repository_root();
    let mut files = Vec::new();
    documents(&root.join("docs"), &mut files);
    assert!(files.len() > 50, "found {} documents", files.len());
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        let format = Format::of_path(&path.to_string_lossy()).unwrap();
        let options = MapOptions::new(format);
        let map = load_source_map(&text, &options);
        check_well_formed(&map, &text, &path);
        let editable = (0..map.blocks.len() as u32)
            .map(BlockId)
            .filter(|&id| !map.blocks[id.index()].content.is_empty())
            .filter(|&id| {
                block_operations(&map, &text, id)
                    .contains(&guidedoc_edit::OperationKind::ReplaceText)
            });
        for id in editable.take(3) {
            let same = map.content_text(id, &text);
            if same.trim().is_empty() {
                continue;
            }
            let op = Operation::ReplaceText { block: id, text: &same };
            let result =
                apply_operation(&text, &options, &op).unwrap_or_else(|e| panic!("{path:?}: {e}"));
            assert!(result.edits.is_empty(), "{path:?}: {:?}", result.edits);
            assert_eq!(result.text, text);
        }
    }
}
