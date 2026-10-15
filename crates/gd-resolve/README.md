# gd-resolve cheat sheet

The project tables and cross-reference resolution of a build, without the document
tree. Ported from `guidedog_old/lib/project` (resolve.odin, numbering.odin,
lookups.odin, cpp_symbols.odin, cpp_lookup.odin, odin_xref.odin, intersphinx.odin,
intersphinx_resolve.odin, inventory.odin, the resolution half of xref.odin, the data
models of genindex.odin and modindex.odin) plus the new rust domain resolution.

**Wave 0: signatures only.** The types and functions below are final; bodies are
`todo!("wave 1")`.

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
| `BuildView<'b>` (+ `page_name`, `page_url`, `generated_url`, `page_anchor`, `item_number`), `Dates`, `OutputBuilder` | what builders read |
| `XrefQuery`, `Resolved`, `Unresolved` (warn, code, category, message, hint, suggestion), `resolve_xref`, `lookup_answer`, `role_takes` | references |
| `IndexGroup`/`IndexTerm`/`IndexSub`/`IndexLink`, `create_index`, `ModindexLetter`/`ModindexEntry`, `module_index`, `has_module_index`, `write_inventory` | index data |
| `relative_url`, `normalize_doc`, `doc_target`, `file_target`, `generated_page` | names and URLs |

## Redesign notes

- **AST-free**: resolution takes an `XrefQuery` (what gd-link reads off a
  `pending_xref` node) instead of a `gd.View` and node id, so gd-link owns the tree walk
  and gd-resolve the tables.
- **Ids**: `DocId(u32)` and `CppSymbolId(u32)` newtypes with `Option` for "none"
  (was `NO_DOC` and `-1`).
- **Traces**: every table lookup is noted as a typed `gd_env::Lookup`;
  `lookup_answer` returns the answer's `Digest`, stored next to the key in the manifest.
- **Builders are a trait** (`OutputBuilder`) over a borrowed `BuildView`, replacing the
  `^Build` every Odin builder procedure took.
- **Rust references (new)**: `:rust:*:` roles resolve full `::` paths, the current
  module first; `~` and a leading `.` behave as for Python; `rust:any` falls back to
  intersphinx; inventory priorities are 0 for modules and 1 for other objects.
