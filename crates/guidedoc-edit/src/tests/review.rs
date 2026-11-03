//! Cases a review found: each once panicked, lost text or accepted a wrong edit.

use super::{all, edited, map_of};
use crate::*;

#[test]
fn comment_markers_are_stripped_on_character_boundaries() {
    let text = "%\u{e9}x\n\npara\n";
    let map = map_of(Format::Markdown, text);
    let comment = all(&map, BlockKind::Comment)[0];
    assert_eq!(map.content_text(comment, text), "\u{e9}x");
    let text = "%abc\n";
    let map = map_of(Format::Markdown, text);
    let comment = all(&map, BlockKind::Comment)[0];
    assert_eq!(map.content_text(comment, text), "abc");
    let op = Operation::ReplaceText { block: comment, text: "new" };
    assert_eq!(edited(Format::Markdown, text, op), "%new\n");
    let text = "% a\n% b\n\npara\n";
    let map = map_of(Format::Markdown, text);
    let comment = all(&map, BlockKind::Comment)[0];
    assert_eq!(map.content_text(comment, text), "a\nb");
    let op = Operation::ReplaceText { block: comment, text: "x\ny" };
    assert_eq!(edited(Format::Markdown, text, op), "% x\n% y\n\npara\n");
    let rst = ".. a\n   b\n\npara\n";
    let map = map_of(Format::Rst, rst);
    let comment = all(&map, BlockKind::Comment)[0];
    assert_eq!(map.content_text(comment, rst), "a\nb");
}

#[test]
fn an_item_goes_next_to_an_empty_item() {
    for (format, text, want) in
        [(Format::Markdown, "- a\n-", "- a\n-\n- New"), (Format::Rst, "* a\n*", "* a\n*\n* New")]
    {
        let map = map_of(format, text);
        let last = *all(&map, BlockKind::ListItem).last().unwrap();
        let op = Operation::InsertItem { anchor: last, position: Position::After, text: "New" };
        assert_eq!(edited(format, text, op), want);
    }
}

#[test]
fn a_truncated_map_refuses_edits() {
    let text = "a\n\nb\n\nc\n";
    let options = MapOptions { max_blocks: 1, ..MapOptions::new(Format::Markdown) };
    let map = load_source_map(text, &options);
    assert!(map.truncated);
    let op = Operation::ReplaceText { block: map.roots[0], text: "a\n```" };
    let error = apply_operation(text, &options, &op).unwrap_err();
    assert_eq!(error.code, "edit.truncated");
    assert!(error.span.is_some() && error.hint.contains("max_blocks"), "{error}");
    let options = MapOptions { max_blocks: 3, ..MapOptions::new(Format::Markdown) };
    let map = load_source_map(text, &options);
    let op =
        Operation::InsertBlock { anchor: map.roots[2], position: Position::After, markup: "d" };
    let error = apply_operation(text, &options, &op).unwrap_err();
    assert_eq!(error.code, "edit.truncated");
}
