# gd-resolve cheat sheet

The project tables and cross-reference resolution of a build, without the document
tree: toctrees, numbering, labels and objects, the C++ symbol tree, intersphinx, the
index models and the resolution of every domain's references.

```rust
let tables = gd_resolve::resolve(&config, &env, &ui, &mut reports);
let inventories = gd_resolve::load_intersphinx(&config,
    IntersphinxOptions { network: !untrusted, budget: &budget }, &mut reports)?;
let view = BuildView { config: &config, env: &env, tables: &tables, /* ... */ };
let to = gd_resolve::resolve_xref(&view, &query, &mut trace)?; // Err(Box<Unresolved>)
let answer = gd_resolve::lookup_answer(&view, "guide/intro", &lookup); // a Digest
builder.write(&view, &mut generation, &mut reports)?;       // impl OutputBuilder
```

| Item | Does |
|---|---|
| `Tables { by_name, order, parent, trees, secnumbers, fignumbers, labels, terms, objects, cpp, keys }`, `TargetRef`, `TreeEntry`, `resolve`, `object_key` | project tables |
| `CppTree`, `CppSymbol`, `CppSymbolId(u32)`, `CppDeclaration` | the C++ symbol tree |
| `Intersphinx { named, main, digest }`, `IntersphinxOptions`, `load_intersphinx` | inventories |
| `BuildView<'b>` (+ `doc`, `doc_key`, `page_name`, `page_url`, `generated_url`, `page_anchor`, `item_number`, `section_number`, `number_template`, `image_file` -> `ImageFile`), `Dates`, `OutputBuilder` | what builders read |
| `XrefQuery` (+ `explicit`, `content`, `inventory`, `context` pairs; `context(key)`, `flag(key)`), `Resolved` (+ `asset`, `warning`), `Unresolved` (warn, code, category, title, message, hint, suggestion; `report(path)`), `resolve_xref`, `lookup_answer`, `role_takes` | references |
| `document_settings`, `page_inputs` | the input digests every page builder shares |
| `IndexGroup`/`IndexTerm`/`IndexSub`/`IndexLink`, `create_index`, `ModindexLetter`/`ModindexEntry`, `module_index`, `has_module_index`, `write_inventory` | index data |
| `relative_url`, `normalize_doc`, `doc_target`, `file_target`, `generated_page`, `chapter_key`, `normalize_name` and `option_key` (from gd-env), `format_number`, `python_format` | names, URLs, formats |

Warning codes: `build.toctree.{missing,duplicate,untitled,cycle,orphan,renumbered}`,
`build.root.missing` (error), `build.label.duplicate`, `build.object.duplicate`,
`build.cpp.duplicate`, `{odin,rust}.autoapi.toctree`, `build.intersphinx.fetch`; on
references `build.xref.unresolved`, `build.xref.external`, `build.xref.anchor`,
`build.numref`, `build.cpp.role`.

## Design notes

- **AST-free**: resolution takes an `XrefQuery` (what gd-link reads off a
  `pending_xref` node) instead of a tree view and node id, so gd-link owns the tree walk
  and gd-resolve the tables.
- **Ids**: `DocId(u32)` and `CppSymbolId(u32)` newtypes with `Option` for "none"
  instead of sentinel values.
- **Traces**: every table lookup is noted as a typed `gd_env::Lookup`;
  `lookup_answer` returns the answer's `Digest`, stored next to the key in the manifest.
- **Builders are a trait** (`OutputBuilder`) over a borrowed `BuildView`, so every
  builder reads the same tables and none can change them.
- **The reader's context is data**: the node attributes a reference depends on
  (`js:module`, `c:parent`, `std:program`, `odin:package`, `cpp:name`, ...) arrive as
  `XrefQuery::context` pairs, so gd-link copies a node's custom attributes and
  gd-resolve never sees the tree. Warnings about references that still resolve
  (numref without a number, heading anchor, C++ role) come back in
  `Resolved::warning`; gd-link places them.
- **Elm-style failures**: `Unresolved` carries a title, a hint per role, and a
  "Did you mean ...?" from `gd_report::suggest` over names of the right kind (labels,
  documents, terms, objects of the role's types, relative to the current module too).
  Suggestions are computed only when the reference warns. A missing toctree entry's
  hint suggests a docname too.
- **Answers are digests** of what the reference shows: besides an object's fields, its
  alias flag and a section label's number (a `:numref:` to a section changes when
  renumbering does), so `Generation::reuse` misses no change. Suggestions in warnings
  are not part of any answer: a new near name does not re-link a page.
- **No recursion on input**: the toctree walk keeps its own stack; section numbering
  stops nesting at 256 documents deep, figure numbering at 64.
- **Intersphinx**: inventories are fetched with gd-fetch and cached with a `.base` file;
  `--untrusted` (`network: false`) reads remote inventories only from the cache, at any
  age, and local ones only inside `readable_roots`. The budget is charged about four
  times an inventory's bytes while loading and released once it is loaded.
  `gd_inventory::Item` has no project, so `Intersphinx::origin` finds the inventory an
  item of the merged one came from, for the "(in Python v3.12)" tooltip.
- **Images**: `BuildView::image_file` lives here, as an `Image` lookup's answer needs it.
- **Rust references**: `:rust:*:` roles resolve full `::` paths, the current
  module first; `~` and a leading `.` behave as for Python; `rust:any` falls back to
  intersphinx; inventory priorities are 0 for modules and 1 for other objects.
  Details: the module's enclosing modules are tried outward after the module itself;
  `crate::`, `self::` and `super::` read as in Rust; a whole path is found whatever its
  type (as Sphinx's exact search), suffix searches keep to the role's types
  (`fn`: function, method; `type`: type aliases, associated types, struct, enum, union,
  trait; `mod`: module, crate). `rust:any` searches only the rust domain's inventory
  entries; Rust objects are warned about when described twice, as Python ones.
