# gd-pyscan cheat sheet

Static analysis of Python source through tree-sitter: modules, docstrings, `__all__`,
imports; classes (bases, decorators, members in source order); functions (parameters as
written, `# type:` comments); and the attribute documentation Sphinx's `ModuleAnalyzer`
finds (`#:` comments, docstrings after assignments). Names resolve across the modules of
the source roots. Nothing is executed. `gd-autodoc` builds Sphinx autodoc output on it.

## Use

```toml
[dependencies]
gd-pyscan = "0.7"
```

We analyse a module without running it:

```rust
use gd_pyscan::{Object, Project};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join("gd-pyscan-example");
    std::fs::create_dir_all(&root)?;
    let source = "\"\"\"Shapes.\"\"\"\n\nclass Square:\n    \"\"\"A square.\"\"\"\n";
    std::fs::write(root.join("shapes.py"), source)?;

    let mut project = Project::new(&[&root])?;
    let id = project.load_module("shapes").ok_or("no module named shapes")?;
    assert_eq!(project.module(id).doc.as_deref(), Some("Shapes."));
    let square = project.get_attr(&Object::of_module(id), "Square");
    assert!(square.class.is_some());
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/gd-pyscan). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Cheat sheet

```rust
use gd_pyscan::{Object, Project};

let mut p = Project::new(&["src"])?;                  // Err only if the grammar is refused
let Some(id) = p.load_module("mypackage.core") else { return Ok(()) };
let m = p.module(id).clone();                         // Arc<Module>: immutable once loaded
println!("{:?}", m.doc);
let thing = p.get_attr(&Object::of_module(id), "Thing");
if let Some(c) = thing.class {
    let (order, complete) = p.mro(c);
    println!("{} classes in the MRO (complete: {complete})", order.len());
}
```

| Call | Does |
|---|---|
| `Project::new(roots)`, `Project::with_reader(roots, reader)` | a project; `reader` confines reads |
| `load_module(name) -> Option<ModuleId>` | find, read, analyse a module by dotted name, once |
| `module(id)`, `class(ClassRef)`, `def(DefRef)`, `files()` | loaded data; files read (dependencies) |
| `get_attr`, `module_attr`, `class_attr`, `lookup_name`, `reference`, `def_object` | resolve names to an `Object` without running code |
| `exported(m, name)` | whether `from m import *` binds a name |
| `mro(c)`, `base_objects(c)`, `slots(c)`, `is_exception(c)` | MRO (C3), bases, `__slots__` |
| `parse_expression(m, text)` | a string annotation parsed in `m`'s namespace |
| `Module::node(id)` -> `Node` | `kind`, `text`, `field`, `named_children`, `children`, ... |
| `unparse`, `try_unparse`, `write_arguments`, `number_repr` | expressions written back (Sphinx or Python style) |
| `string_value`, `str_value`, `repr_str`, `repr_bytes` | literals as Python evaluates and `repr`s them |
| `prepare_docstring`, `split_lines` | docstring lines as Sphinx prepares them |
| `is_builtin`, `is_builtin_exception` | Python's builtin names |

Limits: `MAX_SYNTAX_DEPTH` (256; deeper modules are `too_deep`, kind `Native`), and per
lookup `MAX_DEPTH` (48), `MAX_STEPS` (100,000), `LOOKUP_STACK_BYTES` (256 KiB, doubled in
debug builds), `MAX_MRO_DEPTH` (100). Past a limit a lookup finds nothing; nothing panics.

## Design notes

- The tree-sitter tree is copied into a flat `Syntax` (one `Vec` of nodes, children
  contiguous, `NodeId` indices) that owns its source, so analysed data holds ids and
  tree-sitter's tree is dropped right after parsing.
- Classes, functions and bindings are `Vec`s in their `Module`, named by `ClassRef`,
  `FunctionRef`, `DefRef` (module id + index) rather than pointers; lambdas become
  `FunctionKind::Lambda` and are built on demand. Loaded modules are `Arc<Module>`
  (immutable, shared); the MRO cache lives in the project. A `Project` is `Send` (its
  `FileReader` must be too), so a host may keep one behind a `Mutex` and analyse each
  module once for many directives.
- `Option` marks absent nodes and docstrings; closed sets of kinds are enums. Bytes
  literals hold one `char` per byte (Latin-1), since `String`
  must be UTF-8.
- `files()` yields the normalised text of each module read (the host hashes it).
