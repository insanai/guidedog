use super::{all, edited, map_of, refused};
use crate::*;

const DOC: &str = "\
# Guide

Intro *para*.

## Steps

1. First
2. Second

> Quoted line
> continues.

```python
print(1)
```

```{note}
:class: tip

Inside note.
```

## End

Last para.
";

fn id(kind: BlockKind, n: usize) -> BlockId {
    all(&map_of(Format::Markdown, DOC), kind)[n]
}

#[test]
fn replace_headings_quotes_and_code() {
    let op = Operation::ReplaceText { block: id(BlockKind::Title, 1), text: "All the steps" };
    assert_eq!(edited(Format::Markdown, DOC, op), DOC.replace("## Steps", "## All the steps"));
    let quoted = id(BlockKind::Paragraph, 3);
    let op = Operation::ReplaceText { block: quoted, text: "One\nTwo\nThree" };
    let out = edited(Format::Markdown, DOC, op);
    assert_eq!(out, DOC.replace("> Quoted line\n> continues.", "> One\n> Two\n> Three"));
    let code = id(BlockKind::LiteralBlock, 0);
    let op = Operation::ReplaceText { block: code, text: "x = 1\n\ny = 2" };
    assert_eq!(edited(Format::Markdown, DOC, op), DOC.replace("print(1)", "x = 1\n\ny = 2"));
    let op = Operation::ReplaceText { block: code, text: "```\nescaped" };
    refused(Format::Markdown, DOC, op, "edit.unsafe");
}

#[test]
fn items_in_ordered_lists() {
    let first = id(BlockKind::ListItem, 0);
    let op = Operation::InsertItem { anchor: first, position: Position::After, text: "Between" };
    let out = edited(Format::Markdown, DOC, op);
    assert_eq!(out, DOC.replace("1. First\n", "1. First\n1. Between\n"));
    let op = Operation::DeleteBlock { block: id(BlockKind::ListItem, 1) };
    assert_eq!(edited(Format::Markdown, DOC, op), DOC.replace("\n2. Second", ""));
}

#[test]
fn myst_options() {
    let note = id(BlockKind::Admonition, 0);
    let op = Operation::SetOption { block: note, name: "class", value: "warning" };
    assert_eq!(edited(Format::Markdown, DOC, op), DOC.replace(":class: tip", ":class: warning"));
    let op = Operation::SetOption { block: note, name: "name", value: "first-note" };
    let out = edited(Format::Markdown, DOC, op);
    assert_eq!(out, DOC.replace(":class: tip\n", ":class: tip\n:name: first-note\n"));
    let op = Operation::RemoveOption { block: note, name: "class" };
    assert_eq!(edited(Format::Markdown, DOC, op), DOC.replace(":class: tip\n", ""));
    let yaml = "```{image} a.png\n---\nalt: Old\n---\n```\n";
    let map = map_of(Format::Markdown, yaml);
    let op = Operation::SetOption { block: map.roots[0], name: "alt", value: "New" };
    assert_eq!(edited(Format::Markdown, yaml, op), yaml.replace("Old", "New"));
}

#[test]
fn sections_move_and_blocks_insert() {
    let end = id(BlockKind::Section, 2);
    let steps = id(BlockKind::Section, 1);
    let op = Operation::MoveBlock { block: end, anchor: steps, position: Position::Before };
    let out = edited(Format::Markdown, DOC, op);
    assert!(
        out.starts_with("# Guide\n\nIntro *para*.\n\n## End\n\nLast para.\n\n## Steps\n"),
        "{out}"
    );
    assert!(out.ends_with("Inside note.\n```\n"), "{out}");
    let intro = id(BlockKind::Paragraph, 0);
    let op =
        Operation::InsertBlock { anchor: intro, position: Position::After, markup: "- a\n- b" };
    let out = edited(Format::Markdown, DOC, op);
    assert_eq!(out, DOC.replace("Intro *para*.\n", "Intro *para*.\n\n- a\n- b\n"));
    let quoted = id(BlockKind::Paragraph, 3);
    let op = Operation::InsertBlock { anchor: quoted, position: Position::After, markup: "More." };
    let out = edited(Format::Markdown, DOC, op);
    assert_eq!(out, DOC.replace("> continues.\n", "> continues.\n>\n> More.\n"));
}
