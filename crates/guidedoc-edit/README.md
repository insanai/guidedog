# guidedoc-edit

Editable source maps and source-preserving edits for reStructuredText and Markdown, made with Guidedog's own readers. A map lists the blocks of one document as a tree: each block has a stable kind, the byte span of its whole markup, the spans of the text an author edits without markup, the parts of a directive (name, arguments, options, content), and its children. An edit operation, such as replacing a paragraph or moving a section, becomes minimal text edits that leave every other byte as it was. Every edit is read again and refused unless the document still has the intended structure. Visual editors build on this crate, so what they show agrees with what Guidedog renders.

## Use

```toml
[dependencies]
guidedoc-edit = "1.0.0-beta.12"
```

We replace the text of a paragraph inside a list item:

```rust
use guidedoc_edit::{BlockKind, Format, MapOptions, Operation, apply_operation, load_source_map};

fn main() -> Result<(), guidedoc_edit::EditError> {
    let text = "Steps\n=====\n\n* Open the file\n  and read it.\n";
    let options = MapOptions::new(Format::Rst);
    let map = load_source_map(text, &options);
    let item = map.find(BlockKind::ListItem).expect("the list has an item");
    let op = Operation::ReplaceText { block: item, text: "Open the file\nand edit it." };
    let edit = apply_operation(text, &options, &op)?;
    assert_eq!(edit.text, "Steps\n=====\n\n* Open the file\n  and edit it.\n");
    println!("{:?}", edit.edits);
    Ok(())
}
```

Build the API documentation locally with `cargo doc -p guidedoc-edit --no-deps`. This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others. GDS 0017 records the design.

## Cheat sheet

```rust
use guidedoc_edit::*;

let options = MapOptions { sphinx: Some(&config), ..MapOptions::new(Format::Rst) };
let map = load_source_map(text, &options);       // never fails; unreadable text maps to source blocks
map.block(id); map.find(BlockKind::Title); map.block_at(offset); map.content_text(id, text);
block_operations(&map, text, id);                 // what an editor may offer for one block
operations(Format::Markdown);                     // what the format supports
apply_operation(text, &options, &Operation::MoveBlock { block, anchor, position: Position::After })?;
write_source_map_json(&map, text, &mut out);      // the JSON form, SOURCE_MAP_VERSION
```

- Formats: `Format::Rst` (reader `sphinx`) and `Format::Markdown` (reader `myst`), as projects read them; `MapOptions::sphinx` and `MapOptions::myst` carry the project's configuration, which decides which directives exist and which options they take.
- Kinds (`BlockKind::name`): `section`, `title`, `paragraph`, `bullet-list`, `enumerated-list`, `definition-list`, `list-item`, `definition-item`, `field-list`, `field`, `literal-block`, `directive`, `admonition`, `table`, `figure`, `image`, `caption`, `block-quote`, `comment`, `target`, `substitution-definition`, `transition`, `source`.
- Operations (`OperationKind::name`): `replace-text`, `insert-block`, `delete-block`, `move-block`, `set-option`, `remove-option`, `insert-item`.
- Refusals are `EditError` values with stable codes (`edit.unsafe`, `edit.unknown-option`, ...) and `EditError::report` for an Elm-style report with the block's excerpt.
