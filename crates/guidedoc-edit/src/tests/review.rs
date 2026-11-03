//! Cases a review found: each once panicked, lost text or accepted a wrong edit.

use super::{all, edited, map_of, outline, refused};
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

#[test]
fn an_underline_is_not_the_next_title_overline() {
    let text = "A\n===\nB\n---\n\ntext\n";
    let map = map_of(Format::Rst, text);
    assert_eq!(outline(&map), "section(title section(title paragraph))");
    let titles = all(&map, BlockKind::Title);
    assert_eq!(map.blocks[titles[1].index()].span.text(text), "B\n---");
    let over = "===\nA\n===\n\nB\n---\n\ntext\n";
    let map = map_of(Format::Rst, over);
    let titles = all(&map, BlockKind::Title);
    assert_eq!(map.blocks[titles[0].index()].span.text(over), "===\nA\n===");
}

#[test]
fn code_in_a_markdown_quote_has_no_quote_markers() {
    let text = "> ```\n> code\n> ```\n";
    let map = map_of(Format::Markdown, text);
    let code = all(&map, BlockKind::LiteralBlock)[0];
    assert_eq!(map.content_text(code, text), "code");
    let op = Operation::ReplaceText { block: code, text: "a\n\nb" };
    assert_eq!(edited(Format::Markdown, text, op), "> ```\n> a\n>\n> b\n> ```\n");
    let text = "> ```{code-block} python\n> :linenos:\n>\n> x = 1\n> ```\n";
    let map = map_of(Format::Markdown, text);
    let code = all(&map, BlockKind::LiteralBlock)[0];
    assert_eq!(map.content_text(code, text), "x = 1");
    let options = &map.blocks[code.index()].directive.as_ref().unwrap().options;
    assert_eq!(options[0].name.text(text), "linenos");
    let op = Operation::ReplaceText { block: code, text: "y = 2" };
    assert_eq!(edited(Format::Markdown, text, op), text.replace("x = 1", "y = 2"));
}

#[test]
fn an_unclosed_fence_keeps_its_last_line() {
    let text = "```\ncode\nmore";
    let map = map_of(Format::Markdown, text);
    let code = all(&map, BlockKind::LiteralBlock)[0];
    assert_eq!(map.content_text(code, text), "code\nmore");
    let op = Operation::ReplaceText { block: code, text: "x" };
    assert_eq!(edited(Format::Markdown, text, op), "```\nx");
    let one = "```\ncode";
    let map = map_of(Format::Markdown, one);
    let code = all(&map, BlockKind::LiteralBlock)[0];
    assert_eq!(map.content_text(code, one), "code");
}

#[test]
fn a_replacement_must_read_back_as_the_text_asked_for() {
    let text = ":name: body\n";
    let map = map_of(Format::Rst, text);
    let field = all(&map, BlockKind::Field)[0];
    let op = Operation::ReplaceText { block: field, text: "a: b" };
    refused(Format::Rst, text, op, "edit.unsafe");
    let op = Operation::ReplaceText { block: field, text: "other" };
    assert_eq!(edited(Format::Rst, text, op), ":other: body\n");
}

#[test]
fn yaml_option_values_read_back_as_written() {
    let yaml = "```{image} a.png\n---\nalt: Old\n---\n```\n";
    let map = map_of(Format::Markdown, yaml);
    for value in ["x #y", "x # y-z", "a: b", "- x", "'q'", "#c", "[a]", "b:", " x"] {
        let op = Operation::SetOption { block: map.roots[0], name: "alt", value };
        refused(Format::Markdown, yaml, op, "edit.option-value");
    }
    let op = Operation::SetOption { block: map.roots[0], name: "alt", value: "a#b c:d" };
    assert_eq!(edited(Format::Markdown, yaml, op), yaml.replace("Old", "a#b c:d"));
    let fields = "```{image} a.png\n:alt: Old\n```\n";
    let map = map_of(Format::Markdown, fields);
    let op = Operation::SetOption { block: map.roots[0], name: "alt", value: "x #y" };
    assert_eq!(edited(Format::Markdown, fields, op), fields.replace("Old", "x #y"));
}
