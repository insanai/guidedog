use super::{all, edited, map_of, outline, refused};
use crate::*;

const DOC: &str = "\
=====
Title
=====

Intro para.

Section
-------

* item one
  continues
* item two

Body para.

.. note::

   Inside note.

.. code-block:: python
   :linenos:

   print(1)

Other
-----

Last para.
";

fn id(kind: BlockKind, n: usize) -> BlockId {
    all(&map_of(Format::Rst, DOC), kind)[n]
}

#[test]
fn replace_text_reindents_and_keeps_other_bytes() {
    let op =
        Operation::ReplaceText { block: id(BlockKind::ListItem, 0), text: "first\nsecond line" };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("item one\n  continues", "first\n  second line"));
    let op =
        Operation::ReplaceText { block: id(BlockKind::Paragraph, 4), text: "Inside *changed*." };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("Inside note.", "Inside *changed*."));
}

#[test]
fn replace_title_rewrites_the_adornment_to_display_width() {
    let op = Operation::ReplaceText {
        block: id(BlockKind::Title, 0),
        text: "\u{65e5}\u{672c}\u{8a9e} guide",
    };
    let out = edited(Format::Rst, DOC, op);
    let rule = "=".repeat(12);
    assert!(
        out.starts_with(&format!("{rule}\n\u{65e5}\u{672c}\u{8a9e} guide\n{rule}\n\nIntro")),
        "{out}"
    );
    let op = Operation::ReplaceText { block: id(BlockKind::Title, 1), text: "A longer section" };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("Section\n-------", "A longer section\n----------------"));
}

#[test]
fn replace_code_keeps_the_indentation() {
    let op = Operation::ReplaceText {
        block: id(BlockKind::LiteralBlock, 0),
        text: "a = 1\n\nif a:\n    print(a)",
    };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("   print(1)", "   a = 1\n\n   if a:\n       print(a)"));
}

#[test]
fn replace_text_refusals() {
    let para = id(BlockKind::Paragraph, 0);
    refused(
        Format::Rst,
        DOC,
        Operation::ReplaceText { block: para, text: "* a list" },
        "edit.unsafe",
    );
    refused(
        Format::Rst,
        DOC,
        Operation::ReplaceText { block: para, text: "a\n\nb" },
        "edit.invalid-text",
    );
    refused(
        Format::Rst,
        DOC,
        Operation::ReplaceText { block: para, text: "  " },
        "edit.invalid-text",
    );
    let title = id(BlockKind::Title, 0);
    refused(
        Format::Rst,
        DOC,
        Operation::ReplaceText { block: title, text: "a\nb" },
        "edit.invalid-text",
    );
    let list = id(BlockKind::BulletList, 0);
    refused(
        Format::Rst,
        DOC,
        Operation::ReplaceText { block: list, text: "x" },
        "edit.unsupported",
    );
    let none = BlockId(999);
    refused(Format::Rst, DOC, Operation::DeleteBlock { block: none }, "edit.unknown-block");
}

#[test]
fn insert_blocks_at_the_anchor_indentation() {
    let note_para = id(BlockKind::Paragraph, 4);
    let op = Operation::InsertBlock {
        anchor: note_para,
        position: Position::After,
        markup: "More *inside*.",
    };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("   Inside note.\n", "   Inside note.\n\n   More *inside*.\n"));
    let body = id(BlockKind::Paragraph, 3);
    let markup = ".. warning::\n\n   Careful.";
    let op = Operation::InsertBlock { anchor: body, position: Position::Before, markup };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("Body para.", ".. warning::\n\n   Careful.\n\nBody para."));
    let map = map_of(Format::Rst, &out);
    assert_eq!(all(&map, BlockKind::Admonition).len(), 2);
}

#[test]
fn insert_refuses_markup_that_merges() {
    let body = id(BlockKind::Paragraph, 3);
    let op =
        Operation::InsertBlock { anchor: body, position: Position::After, markup: "New\n---" };
    refused(Format::Rst, DOC, op, "edit.unsafe");
}

#[test]
fn delete_blocks_with_their_separation() {
    let op = Operation::DeleteBlock { block: id(BlockKind::Paragraph, 3) };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("Body para.\n\n", ""));
    let op = Operation::DeleteBlock { block: id(BlockKind::Paragraph, 5) };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("\n\nLast para.", ""));
    let op = Operation::DeleteBlock { block: id(BlockKind::ListItem, 1) };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("\n* item two", ""));
    let op = Operation::DeleteBlock { block: id(BlockKind::Section, 2) };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("\n\nOther\n-----\n\nLast para.", ""));
    refused(
        Format::Rst,
        DOC,
        Operation::DeleteBlock { block: id(BlockKind::Title, 1) },
        "edit.unsupported",
    );
}

#[test]
fn move_blocks_and_sections() {
    let body = id(BlockKind::Paragraph, 3);
    let code = id(BlockKind::LiteralBlock, 0);
    let op = Operation::MoveBlock { block: body, anchor: code, position: Position::After };
    let out = edited(Format::Rst, DOC, op);
    let want =
        DOC.replace("Body para.\n\n", "").replace("   print(1)\n", "   print(1)\n\nBody para.\n");
    assert_eq!(out, want);
    let other = id(BlockKind::Section, 2);
    let section = id(BlockKind::Section, 1);
    let op = Operation::MoveBlock { block: other, anchor: section, position: Position::Before };
    let out = edited(Format::Rst, DOC, op);
    assert!(
        out.contains("Intro para.\n\nOther\n-----\n\nLast para.\n\nSection\n-------"),
        "{out}"
    );
    assert!(out.ends_with("   print(1)\n"), "{out}");
    let map = map_of(Format::Rst, &out);
    let sections: Vec<usize> = map.blocks[map.roots[0].index()]
        .children
        .iter()
        .filter(|c| map.blocks[c.index()].kind == BlockKind::Section)
        .map(|c| map.blocks[c.index()].children.len())
        .collect();
    assert_eq!(sections, [2, 5], "{}", outline(&map));
    let into_itself =
        Operation::MoveBlock { block: section, anchor: body, position: Position::After };
    refused(Format::Rst, DOC, into_itself, "edit.unsupported");
}

#[test]
fn directive_options() {
    let code = id(BlockKind::LiteralBlock, 0);
    let op = Operation::SetOption { block: code, name: "caption", value: "Example" };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("   :linenos:\n", "   :linenos:\n   :caption: Example\n"));
    let op = Operation::RemoveOption { block: code, name: "linenos" };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("   :linenos:\n", ""));
    let note = id(BlockKind::Admonition, 0);
    let op = Operation::SetOption { block: note, name: "class", value: "tip" };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace(".. note::\n", ".. note::\n   :class: tip\n"));
    let op = Operation::SetOption { block: note, name: "clas", value: "x" };
    let options = MapOptions::new(Format::Rst);
    let error = apply_operation(DOC, &options, &op).unwrap_err();
    assert_eq!(error.code, "edit.unknown-option");
    assert!(error.hint.contains("Did you mean `class`?"), "{}", error.hint);
    let op = Operation::SetOption { block: code, name: "linenos", value: "yes" };
    refused(Format::Rst, DOC, op, "edit.option-value");
    refused(
        Format::Rst,
        DOC,
        Operation::RemoveOption { block: code, name: "caption" },
        "edit.missing-option",
    );
    let para = id(BlockKind::Paragraph, 0);
    refused(
        Format::Rst,
        DOC,
        Operation::SetOption { block: para, name: "a", value: "" },
        "edit.unsupported",
    );
}

#[test]
fn option_values_are_replaced_in_place() {
    let text = ".. image:: a.png\n   :width: 50%\n   :alt: Old\n";
    let map = map_of(Format::Rst, text);
    let op = Operation::SetOption { block: map.roots[0], name: "alt", value: "New text" };
    let out = edited(Format::Rst, text, op);
    assert_eq!(out, text.replace("Old", "New text"));
    let op = Operation::SetOption { block: map.roots[0], name: "align", value: "center" };
    let out = edited(Format::Rst, text, op);
    assert!(out.ends_with(":alt: Old\n   :align: center\n"), "{out}");
    let op = Operation::SetOption { block: map.roots[0], name: "align", value: "sideways" };
    refused(Format::Rst, text, op, "edit.option-value");
}

#[test]
fn list_items() {
    let item = id(BlockKind::ListItem, 1);
    let op = Operation::InsertItem { anchor: item, position: Position::After, text: "item three" };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("* item two\n", "* item two\n* item three\n"));
    let op =
        Operation::InsertItem { anchor: item, position: Position::Before, text: "between\nlines" };
    let out = edited(Format::Rst, DOC, op);
    assert_eq!(out, DOC.replace("* item two\n", "* between\n  lines\n* item two\n"));
    let op = Operation::ReplaceText { block: item, text: "renamed" };
    assert_eq!(edited(Format::Rst, DOC, op), DOC.replace("item two", "renamed"));
}

#[test]
fn numbered_lists_are_renumbered() {
    let text = "1. one\n\n2. two\n\n3. three\n";
    let map = map_of(Format::Rst, text);
    let items = all(&map, BlockKind::ListItem);
    let op = Operation::InsertItem { anchor: items[0], position: Position::After, text: "new" };
    let out = edited(Format::Rst, text, op);
    assert_eq!(out, "1. one\n\n2. new\n\n3. two\n\n4. three\n");
    let wide = "8. a\n9. b\n";
    let map = map_of(Format::Rst, wide);
    let op = Operation::InsertItem {
        anchor: map.blocks[map.roots[0].index()].children[0],
        position: Position::After,
        text: "x",
    };
    refused(Format::Rst, wide, op, "edit.renumber");
    let auto = "#. a\n#. b\n";
    let map = map_of(Format::Rst, auto);
    let op = Operation::InsertItem {
        anchor: all(&map, BlockKind::ListItem)[1],
        position: Position::After,
        text: "c",
    };
    assert_eq!(edited(Format::Rst, auto, op), "#. a\n#. b\n#. c\n");
}

#[test]
fn block_operations_follow_the_block() {
    let map = map_of(Format::Rst, DOC);
    let ops = |id| block_operations(&map, DOC, id);
    assert_eq!(ops(id(BlockKind::Title, 0)), vec![OperationKind::ReplaceText]);
    let code = ops(id(BlockKind::LiteralBlock, 0));
    assert!(code.contains(&OperationKind::SetOption) && code.contains(&OperationKind::MoveBlock));
    assert!(ops(id(BlockKind::ListItem, 0)).contains(&OperationKind::InsertItem));
    // The first paragraph of an item shares its line with the marker.
    assert_eq!(ops(id(BlockKind::Paragraph, 1)), vec![OperationKind::ReplaceText]);
    assert_eq!(operations(Format::Rst).len(), OperationKind::ALL.len());
}
