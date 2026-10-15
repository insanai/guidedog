# guidedoc-cpp

**C++ declarations as Sphinx's C++ domain reads them** (Sphinx 9.1 `DefinitionParser`):
the parser, the tree, the ids of every id version (1 to 4), `str()` and display text,
symbol paths, cross-reference targets, and `:cpp:expr:` texts. Std-only; it knows
nothing of documents. `guidedoc-sphinx` turns its plain data into nodes (signatures,
`Desc_Name`/`Desc_Addname`, `cpp:identifier` xrefs).

## Use

```toml
[dependencies]
guidedoc-cpp = "0.7"
```

We parse a `cpp:function` signature and read its Sphinx id:

```rust
use guidedoc_cpp::{CppError, Directive, Limits, namespace_path, parse_declaration};

fn main() -> Result<(), CppError> {
    let limits = Limits::default();
    let scope = namespace_path("", "ns", &limits)?; // cpp:namespace:: ns
    let signature = "template<typename T> void apply(T t)";
    let decl = parse_declaration(signature, Directive::Function, &limits)?;
    let symbol = decl.symbol(&scope)?;
    assert_eq!(symbol.newest_id(), "_CPPv4I0EN2ns5applyEv1T");
    assert_eq!(symbol.full_name, "ns::apply");
    Ok(())
}
```

The API documentation is on [docs.rs](https://docs.rs/guidedoc-cpp). This is one of Guidedog's reusable crates: [Crates](https://insanai.github.io/guidedog/manual/internals/crates.html) in the manual lists the others and how they depend on each other.

## Overview

- Design, bounds, and conformance: `docs/manual/internals/cpp-declarations.rst`
- Conformance with a recorded oracle (702 of 702 corpus lines match):
  `tests/oracle.rs`, `tests/data/oracle.tsv`

```rust
use guidedoc_cpp::{Directive, Limits, namespace_path, parse_declaration};

let limits = Limits::default();
let scope = namespace_path("", "ns", &limits)?;              // cpp:namespace:: ns
let decl = parse_declaration("template<typename T> void apply(T t)",
    Directive::Function, &limits)?;
let symbol = decl.symbol(&scope)?;
assert_eq!(symbol.newest_id(), "_CPPv4I0EN2ns5applyEv1T");
assert_eq!(symbol.full_name, "ns::apply");
assert_eq!((symbol.template_params, symbol.function_params), (vec!["T".into()], vec!["t".into()]));
```

## Cheat sheet

| Need | Call |
|---|---|
| a `cpp:*` signature | `Directive::from_name("enum-class")`, `parse_declaration(def, directive, &limits) -> Declaration` |
| its symbol | `decl.symbol(scope_path) -> Symbol { ids[0..4] (v1..v4, prefixed), elements, full_name, display_name, path, template_params, function_params }`; `symbol.newest_id()` |
| one id | `decl.id(version, &elements)`; any node: `ast.node_id(id, version)` |
| what is declared | `decl.object_type()`, `decl.directive()`, `decl.name()`, `decl.template_lists()` |
| namespace scope | `namespace_path(base_path, arg, &limits)` (`""` = root for `NULL`/`0`/`nullptr`) |
| xref target | `parse_xref_target(target, parens, &limits) -> XrefTarget { names, is_rooted, template_decls, is_shorthand, query, own }`; `encoded_names()` (`cpp:name`), `encoded_template_decls()` (`cpp:tdecls`) |
| `:cpp:expr:` / `:cpp:texpr:` | `parse_expression_or_type(text, &limits) -> Expression { ast, root }` |
| walk the tree | `ast.node(id) -> &Node` (enum, public fields), `ast.list(head)`, `ast.next`, `ast.count`, `ast.last`, `ast.nth` |
| text | `ast.text(id, display)`, `ast.write_text(id, display, &mut out)`, `write_opt`, `write_list`; `is_spaced_operator(op)` |
| describer queries | `declared_name`, `declarator_name`, `function_params`, `trailing_return`, `is_function_type`, `space_after_specs`, `template_params`, `param_identifier`, `param_name`, `param_is_pack`, `specs_cv`, `is_specialization`, `name_elements`, `element` |
| symbol paths | `Element { ident, args, params, display, ids, decl }`, `encode_path`, `decode_path`, `path_count`, `full_name`, `display_name`, `FIELD`, `NO_ID` |
| errors | `CppError { kind: Invalid / Limit(Depth/Stack/Work) / NoId, code, title, message, hint, span, limit }`, `is_limit()` |
| bounds | `Limits { max_depth, stack_bytes }` (256, 512 KiB); `STEPS_PER_BYTE` (64) |

## Design notes

- **Typed tree.** `Node<'a>` is an enum whose variants name their fields (`Class { name,
  bases, is_final, attrs }`); lists stay linked through the nodes, so the tree is one flat
  `Vec` with `NodeId` indices. Texts borrow the definition (`&'a str`, `Cow` for the
  canonical fundamental words).
- **No buffers or pools from the caller.** Ids, text, and paths are written into
  `&mut String` or returned as `String`; the parser's node table is a `Vec` bounded at
  16 nodes per byte + 256.
- **Each id piece is its own string.** A piece computed while another is being written
  never lands twice, so `void f(int (*cb)(double))` gives Sphinx's `_CPPv41fPFidE`
  (likewise for template arguments, array sizes, `decltype`, and call arguments).
- **Plain data, not node writing.** Signatures, cross-reference targets, and namespace
  paths are returned as `Symbol`, `XrefTarget`, and a path `String`; the caller writes
  the attributes (paths, `cpp:name`, `cpp:query`).
- **Limits as errors.** A parse returns
  `ErrorKind::Limit` with the engine's codes (`core.limit.depth`, `.stack`, `.work`) and
  the value passed. The stack guard is local (the crate does not depend on `guidedoc`);
  a caller inside its own guarded recursion passes what remains of its budget as
  `Limits::stack_bytes`.
- **Flat precedence levels.** Sphinx recurses once per binary
  precedence level, eleven frames per level of parentheses (about 5 KiB). One loop over
  a reused buffer of open levels (`parse_binop`) builds the same tree in about 1.5 KiB,
  so the default stack budget always holds the default depth.
- **The C domain** has its own simpler parser and stays in `guidedoc-sphinx`.
