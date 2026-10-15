C++ declarations
================

``crates/guidedoc-cpp`` reads C++ declarations as Sphinx's C++ domain reads them
(``sphinx/domains/cpp``, Sphinx 9.1). It is the machinery under the ``cpp:*`` directives
and roles of the Sphinx layer: a recursive-descent parser (Sphinx's
``DefinitionParser``), the tree it builds, the ids of every id version, the text of
every node, symbol paths, cross-reference targets, and the expressions of
``:cpp:expr:`` and ``:cpp:texpr:``. The crate is std-only and builds no document
nodes; ``guidedoc-sphinx`` describes a signature from the tree.

The tree
--------

A parse builds one flat table of ``Node`` values addressed by ``NodeId``. Each variant
follows one of Sphinx's AST classes and names its fields (``DPtr { next, is_const,
is_volatile, attrs }``); a list of nodes (parameters, name elements, the operands of a
binary operator) is linked through the nodes and walked with ``Ast::list``. Texts
borrow the definition, so a tree lives no longer than the string it was parsed from.

A failed alternative gives its nodes back: the parser marks the table's length and the
position before trying an alternative and truncates to the mark when it fails, as
Sphinx's parser rewinds its position.

Ids
---

``Declaration::symbol`` computes what ``CPPObject.handle_signature`` and
``add_target_and_index`` compute: the declaration's id in versions 1 to 4 (version 1 is
Sphinx 1.2's; versions 2 to 4 follow the Itanium C++ ABI's mangling, ``_CPPv2`` to
``_CPPv4``), the full name and its display string (anonymous entities shown as
``[anonymous]``), the symbol's path, and the names of its template and function
parameters. A version with no id (Sphinx's ``NoOldIdError``) is ``None``; a declaration
without a version 4 id is an error (``cpp.no-id``), as Sphinx then warns.

A symbol path holds the symbols from the root to the declared name: each element's
identifier or operator, its template arguments when they specialize (arguments that
merely repeat the parameters are dropped, as ``_is_specialization`` decides), its
template parameter list, its display text, its id in each version, and the newest id of
its declaration. The Sphinx layer keeps paths as text in node attributes (one element
per line, fields apart by U+001F, a missing id as U+001E) so the host can build the
project's symbol tree; ``encode_path`` and ``decode_path`` convert.

Bounds
------

Every parse is bounded, as GDS 0003 ("Recursion is bounded by the depth limit and the
stack budget") requires:

- Every recursive production (names, types, declarators, template parameter lists,
  attributes, braced lists, and the primary, unary, and cast expressions) passes a
  guard that fails the parse once it nests past ``Limits::max_depth``
  (``core.limit.depth``).
- The same guard compares the address of a local in the current frame with the one
  recorded where the parse started, and fails past ``Limits::stack_bytes``
  (``core.limit.stack``). A caller that runs the parser inside a guarded recursion of
  its own passes what remains of its budget.
- Backtracking is bounded by the definition's length: 64 steps per byte plus 4096
  (``core.limit.work``). Alternatives are retried at every level, so nested
  parentheses would otherwise cost time exponential in their depth.
- A fold expression is tried only when its ``...`` stands in the parenthesized group,
  outside nested brackets and literals, so nested parentheses parse in linear time.
- The node table holds at most 16 nodes per byte plus 256; a parse that needs more
  fails as an invalid declaration, as the Odin reader's fixed pool did.

Once a parse stops at a limit, every later production fails, and the parse returns
``ErrorKind::Limit``: a declaration too deep or too costly to parse is not an invalid
one. The eleven precedence levels of binary operators are read by one loop that keeps
the open levels in a buffer (``parse_binop``), not by one recursive call per level as
Sphinx does, so a level of parentheses in an expression, the costliest nesting, takes
about 1.5 KiB of stack in an optimized build (4.4 KiB without optimization) for three
units of depth, and a level of template arguments about 1 KiB (2.3 KiB). The default
budget of 512 KiB therefore holds the default depth of 256 in either build: the depth
limit stops a parse first. ``cargo test --release -p guidedoc-cpp --test limits --
--ignored --nocapture stack_per_level`` prints the measures.

Conformance
-----------

``crates/guidedoc-cpp/tests/oracle.rs`` checks 702 inputs (declarations of every object
type, in scopes, namespace arguments, cross-reference targets, and role expressions,
many from Sphinx's ``test_domain_cpp``) against what the Odin implementation gives for
them: ids of every version, names, parameter names, ``str()`` and display text, symbol
paths, and expression ids. All 702 match. Run it with

::

    cargo test -p guidedoc-cpp --test oracle

The expectations (``tests/data/oracle.tsv``) were written by the Odin package with
``tests/data/oracle_test.odin`` added to it, reading ``tests/data/corpus.txt``. One
Odin defect was fixed first: it built an id's pieces in a shared arena, so a piece
computed while writing into that arena was written twice (``int (*)(double)`` as
``PFiddE`` where Sphinx writes ``PFidE``; also in template arguments, array sizes,
``decltype``, and call arguments). The Rust port builds each piece in its own string
and gives Sphinx's ids; 28 of the 702 lines differ from the unfixed Odin output.

``tests/limits.rs`` ports the Odin nesting tests: seven constructs nested 1000 deep stop
at the depth limit; nested 4000 deep with a huge depth limit, they stop at the stack
limit; nested 12 deep, none stops at a limit; and 60 levels of parentheses parse in
linear time.
