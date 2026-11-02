use super::{all, map_of, outline};
use crate::*;

const RST: &str = "\
=====
Title
=====

Intro *para* one
line two.

Section
-------

* item one
  continues
* item two

  second para

#. first
#. second

term
   definition text

.. note::

   Inside note.

.. code-block:: python
   :linenos:
   :caption: Cap

   print(1)

.. figure:: img.png
   :alt: Alt

   Caption text.

.. _target:

.. |sub| replace:: replaced

.. comment here

:field: value

::

   literal

=====  =====
A      B
=====  =====

.. image:: pic.png

.. highlight:: rust
";

#[test]
fn rst_maps_every_construct() {
    let map = map_of(Format::Rst, RST);
    assert_eq!(
        outline(&map),
        "section(title paragraph section(title bullet-list(list-item(paragraph) \
         list-item(paragraph paragraph)) enumerated-list(list-item(paragraph) \
         list-item(paragraph)) definition-list(definition-item(paragraph)) \
         admonition(paragraph) literal-block figure(caption) target \
         substitution-definition comment field-list(field(paragraph)) literal-block \
         table image directive))"
    );
    let text = |id: BlockId| map.content_text(id, RST);
    let titles = all(&map, BlockKind::Title);
    assert_eq!((text(titles[0]), map.blocks[titles[0].index()].level), ("Title".into(), 1));
    assert_eq!(map.blocks[titles[0].index()].span.text(RST), "=====\nTitle\n=====");
    assert_eq!(map.blocks[titles[1].index()].level, 2);
    let items = all(&map, BlockKind::ListItem);
    assert_eq!(text(items[0]), "item one\ncontinues");
    let code = all(&map, BlockKind::LiteralBlock);
    assert_eq!(text(code[0]), "print(1)");
    let parts = map.blocks[code[0].index()].directive.as_ref().unwrap();
    assert_eq!(parts.name.text(RST), "code-block");
    assert_eq!(parts.options[1].value.unwrap().text(RST), "Cap");
    assert_eq!(map.blocks[code[1].index()].span.text(RST), "::\n\n   literal");
    let comment = map.find(BlockKind::Comment).unwrap();
    assert_eq!(text(comment), "comment here");
    let gap = map.find(BlockKind::Directive).unwrap();
    assert_eq!(map.blocks[gap.index()].span.text(RST), ".. highlight:: rust");
    assert_eq!(map.find(BlockKind::DefinitionItem).map(text).as_deref(), Some("term"));
}

const MARKDOWN: &str = "\
# Head

Para *x*
more.

## Sub

- item
- two

1. one
2. two

> quoted
> text

```python
print(1)
```

```{note}
Inside.
```

```{code-block} python
:linenos:

x = 1
```

| a | b |
|---|---|
| 1 | 2 |

<!-- comment -->

(label)=
Text after label.

---
";

#[test]
fn markdown_maps_every_construct() {
    let map = map_of(Format::Markdown, MARKDOWN);
    assert_eq!(
        outline(&map),
        "section(title paragraph section(title bullet-list(list-item(paragraph) \
         list-item(paragraph)) enumerated-list(list-item(paragraph) list-item(paragraph)) \
         block-quote(paragraph) literal-block admonition(paragraph) literal-block table \
         comment target paragraph transition))"
    );
    let titles = all(&map, BlockKind::Title);
    assert_eq!(map.blocks[titles[1].index()].level, 2);
    assert_eq!(map.content_text(titles[1], MARKDOWN), "Sub");
    let paras = all(&map, BlockKind::Paragraph);
    let quoted = paras.iter().find(|&&p| map.content_text(p, MARKDOWN).starts_with("quoted"));
    assert_eq!(map.content_text(*quoted.unwrap(), MARKDOWN), "quoted\ntext");
    let code = all(&map, BlockKind::LiteralBlock);
    assert_eq!(map.content_text(code[0], MARKDOWN), "print(1)");
    assert_eq!(map.content_text(code[1], MARKDOWN), "x = 1");
    let section = all(&map, BlockKind::Section)[1];
    assert!(map.blocks[section.index()].span.text(MARKDOWN).ends_with("---"));
}

#[test]
fn block_at_finds_the_innermost_block() {
    let map = map_of(Format::Rst, RST);
    let at = RST.find("Inside").unwrap();
    let id = map.block_at(at).unwrap();
    assert_eq!(map.blocks[id.index()].kind, BlockKind::Paragraph);
    let parent = map.blocks[id.index()].parent.unwrap();
    assert_eq!(map.blocks[parent.index()].kind, BlockKind::Admonition);
    assert_eq!(map.block_at(RST.len() + 5), None);
}

#[test]
fn configuration_decides_directive_rules() {
    // `.. note:: text` has content on its first line: note takes no argument.
    let text = ".. note:: Look here.\n\n.. topic:: Heading\n\n   Body.\n";
    let map = map_of(Format::Rst, text);
    let note = &map.blocks[map.roots[0].index()];
    let parts = note.directive.as_ref().unwrap();
    assert!(parts.arguments.is_none());
    assert_eq!(parts.content.unwrap().text(text), "Look here.");
    let topic = map.blocks[map.roots[1].index()].directive.as_ref().unwrap();
    assert_eq!(topic.arguments.unwrap().text(text), "Heading");
}

#[test]
fn json_lists_blocks_with_operations() {
    let text = "Para.\n\n.. note::\n   :class: x\n\n   Inside.\n";
    let map = map_of(Format::Rst, text);
    let mut out = String::new();
    write_source_map_json(&map, text, &mut out);
    let value = gd_json::parse(&out).unwrap();
    let blocks = value.get("blocks").and_then(|b| b.as_array()).unwrap();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].get("kind").and_then(|k| k.as_str()), Some("paragraph"));
    let ops = blocks[0].get("operations").and_then(|o| o.as_array()).unwrap();
    assert_eq!(ops[0].as_str(), Some("replace-text"));
    let directive = blocks[1].get("directive").unwrap();
    assert_eq!(directive.get("name").and_then(|n| n.as_str()), Some("note"));
    assert!(out.contains("\"set-option\""), "{out}");
}

#[test]
fn every_kind_has_a_stable_name() {
    for kind in BlockKind::ALL {
        assert_eq!(BlockKind::from_name(kind.name()), Some(kind));
    }
    assert_eq!(Format::of_path("docs/Intro.MD"), Some(Format::Markdown));
    assert_eq!(Format::of_path("a.rst"), Some(Format::Rst));
    assert_eq!(Format::of_path("a.typ"), None);
}
