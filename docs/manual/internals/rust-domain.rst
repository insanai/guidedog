The rust and odin domains (guidedoc-sphinx)
===========================================

``crates/guidedoc-sphinx/src/domains/rust.rs`` and ``.../odin.rs`` implement the
Sphinx layer's two domains for compiled languages. The odin domain is a port of
``lib/readers/sphinx/odin.odin`` (GDS 0005 remains its specification, and its tests are
ported unchanged in ``tests/odin_domain.rs``); the rust domain is new, designed after
it, and reads the vocabulary ``gd-rustdoc`` writes (see :doc:`../rust` for the user's
view). This page records the node shapes the rust domain produces and how both are
built.

Shape of the code
-----------------

Each domain is a value implementing the crate's ``Domain`` trait: tables of directives,
roles and info fields, and hooks the shared object description calls (``signature``,
``content_begin``, ``before_content``/``after_content``, ``toc_name``, ``map_target``,
``xref_context``, ``autodoc_context``). Their state lives in ``State::odin`` and
``State::rust``. Each is split in three modules:

``scan.rs``
   Byte scanning of declarations: matching brackets while skipping strings (and, for
   Rust, the ``>`` of ``->`` and ``=>``, and lifetimes, which have no closing quote),
   top-level searches, and the split of a signature at its name.

``render.rs``
   What follows the name: parameter lists, return types, records, bounds, values, and
   the type text whose names become cross-references.

the module itself
   Tables, hooks, the module or package directives, ids and index entries.

The Odin version kept its context as space-separated pool strings and a fixed array of
polymorphic names; the port keeps the import aliases as ``Vec<(alias, package)>``, the
private names as a ``Vec<String>``, and a signature's polymorphic names as borrowed
``&str`` slices of the signature (``SigCx``). Nothing else changed in behaviour.

Rust signatures
---------------

A signature is split by ``scan::parse_head`` into what is written before the name
(attributes, visibility, ``const``/``async``/``unsafe``/``extern "abi"`` qualifiers and
the keyword of the object type), the name, and the rest. Fields and variants have no
keyword; their name is the first identifier (a tuple field's is a number). Impls and
uses have no name: their rest follows the keyword. A short form without the keyword
(``f(x: u8)`` for a function) is accepted and the keyword is shown. Text with no name
falls back to Sphinx's plain signature: the text as the name, no id, no entry.

.. list-table::
   :header-rows: 1
   :widths: 1 2

   * - **Part**
     - **Nodes**
   * - ``:attributes:``, attributes, visibility, qualifiers, keyword
     - DescAnnotation ``"#[non_exhaustive] pub enum "``
   * - module (top-level objects, with ``add_module_names``)
     - DescAddname ``"geometry::"`` (the module's last segment)
   * - name
     - DescName
   * - generics, bounds, tuple fields, variant fields, where clauses
     - Text, with Xref for paths
   * - a function's parameters
     - DescParameterlist > DescParameter (``"x"``, ``": "``, the type)
   * - return type
     - Text ``" -> "`` and the type
   * - a constant's or discriminant's value
     - Text ``" = "`` and the value as written
   * - a use's path
     - Xref to ``:target:`` (or the path), then ``" as name"``

Paths in type positions become ``Xref{reftype "rust:any"}`` with the context
``rust:module`` and, inside a type or impl, ``rust:type``. Keywords, primitive types,
generic parameters (the signature's own and those of the types and impls it is nested
in), lifetimes, numbers, array lengths, and names being declared (``x:`` in a field
list, ``Item =`` in an associated type binding) stay text. ``crate::``, ``self::``,
``super::`` and ``Self::`` paths are resolved against the module and type; every other
path is left as written for the project's resolver (the current module first, then
inventories; an unresolved link shows as code).

Desc_Signature carries ``fullname`` (the path within the module, ``Point::new``),
``module`` (``my_crate::geometry``), ``toc``, ``visibility`` when the option is given,
and Id ``rust-`` + the full path with ``::`` written ``.``. A crate's or module's
directive is Index{Id ``rust-module-<path>``; entries ``single: <path> (crate)`` or
``(module)``; ``module``, ``domain`` = ``rust``, ``synopsis``, ``platform``,
``deprecated``}.

Nesting and targets
-------------------

``rust:struct``, ``rust:enum``, ``rust:union``, ``rust:trait``, ``rust:variant`` and
``rust:impl`` push a scope while their content is read; members are named
``Scope::member``. An impl's scope is the type it is nested in, else its ``:for:``
type, else the self type of its signature, without generic arguments.

Impls and uses are never targets: the domain marks their Desc ``noindex`` and gives
their signatures no ``fullname``. The members of a trait impl (an impl with
``:trait:``, or ``for`` in its signature) are marked ``noindex`` too, whatever the
directive says, as they are documented on the trait. Only an object's first signature
gets an id and an index entry. Entries read ``Point::new (method in
my_crate::geometry)``; functions, methods and traits name their qualifiers
(``draw_all (unsafe function in my_crate)``).

``:cfg:`` and ``:deprecated:`` start the content with a paragraph, "Available on
Code(predicate) only." and Strong("Deprecated") ": note.", their added words marked
Generated; on a crate or module the paragraphs follow the Index.

Roles
-----

``mod``, ``fn``, ``struct``, ``enum``, ``union``, ``trait``, ``variant``, ``field``,
``type``, ``const``, ``static``, ``macro`` and ``any`` are cross-reference roles in
Code with the Python domain's processing (``fn`` adds ``()`` with
``add_function_parentheses``). The domain's ``map_target`` resolves ``crate::``,
``self::``, ``super::`` and ``Self::`` targets as in signatures.

Open points
-----------

- The shared ``~`` processing (``DOTTED``) shortens a title at its last ``.``; for Rust
  it should also cut at ``::`` (``~a::b::Lexer`` shows ``Lexer``). Until the shared role
  code does, such a title shows the whole path; the target is right.
- The project's object collector joins ``module`` and ``fullname`` with ``.``; for the
  rust domain it must join them with ``::`` to give the targets above.

Tests
-----

``tests/rust_domain.rs`` documents ``gd-rustdoc``'s ``shapes`` fixture (with default
options and with private items) and reads every page with no report, then checks the
node shapes of representative items (a generic struct with an inherent and a trait
impl, variants with fields, a union, a trait with associated items, an unsafe generic
function with a where clause, a deprecated async function, re-exports, a cfg-gated
module), hand-written short forms, ``:module:``, standalone impls, roles, no-index
forms, and the table of contents settings. ``tests/odin_domain.rs`` holds the Odin
port's tests.
