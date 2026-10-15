Documenting Rust
================

The Rust domain gives crates, modules, items, and members named targets. Guidedog also
generates API pages from a crate's source, as rustdoc would lay them out. It reads the
source with tree-sitter. It does not run ``cargo``, compile the crate, or expand macros.
A generated signature still needs a useful doc comment.

Describing objects
------------------

A crate or module is the context of what follows it, as a Python module is:

.. code-block:: rst

   .. rust:crate:: shapes
      :synopsis: Plane shapes and their measures.

   .. rust:struct:: pub struct Square

      A square with sides parallel to the axes.

      .. rust:field:: pub side: f32

         The length of a side.

      .. rust:impl:: impl Square

         .. rust:method:: pub fn area(&self) -> f32

            Measures the square.

   .. rust:enum:: #[non_exhaustive] pub enum Kind

      .. rust:variant:: Circle
      .. rust:variant:: Rect { width: f32, height: f32 }

   .. rust:function:: pub fn parse(text: &str) -> Result<Shape, Error>

Signatures are written as Rust writes the declaration, on one line, with its
visibility and qualifiers. The reader finds the name as the identifier after the
keyword (``fn``, ``struct``, ``enum``, ``union``, ``trait``, ``type``, ``const``,
``static``, ``macro_rules!``, ``mod``), or as the first identifier for fields and
variants. The directives:

``rust:crate`` and ``rust:module``
   ``my_crate``, or the full path ``my_crate::parser``. Either sets the current module
   for what follows. Option: ``:synopsis:``, one sentence for the index.

``rust:function`` and ``rust:method``
   ``pub const unsafe fn f<T: Copy>(x: &T) -> Option<T> where T: Send``. Methods sit
   inside a type or an impl (``pub fn new(x: T) -> Self``, ``fn draw(&self) -> u8``).

``rust:struct``, ``rust:enum``, ``rust:union``, ``rust:trait``
   ``pub struct Point<T = f32>``, ``pub struct Pair(pub i32, _)``, ``pub enum Kind``,
   ``pub union Bits``, ``pub trait Draw: Clone + Send``. The content holds the members.

``rust:field`` and ``rust:variant``
   ``pub x: T`` (tuple fields are named ``0``, ``1``, ...); ``Circle``,
   ``Polygon(u8)``, ``Rect { width: f32, height: f32 }``, ``Unknown = 9``. The fields of
   a variant nest in the ``rust:variant``.

``rust:impl``
   ``impl<T: Copy> Point<T>`` or ``impl<T> fmt::Display for Point<T>``. Never a target.
   Its members belong to the type it is nested in (to its ``:for:`` type when it stands
   alone at module level).

``rust:type``, ``rust:const``, ``rust:static``, ``rust:macro``
   ``pub type Result<T> = std::result::Result<T, Error>``, ``pub const MAX: usize =
   4_096``, ``pub static mut COUNT: u64``, ``macro_rules! shape`` (its arms follow in a
   ``code-block`` in the content).

``rust:associatedtype`` and ``rust:associatedconst``
   ``type Output: Default``, ``const PASSES: usize = 1``, inside a trait or impl.

``rust:use``
   ``pub use geometry::point::Point``: a re-export line, never a target. It links to its
   ``:target:``.

Options: ``:module: path`` (describe the object in another module than the current
one), ``:visibility:`` (``pub(crate)``, ``pub(super)``, ``pub(in path)`` or
``private``), ``:cfg: predicate`` (shown as "Available on ``predicate`` only."),
``:unsafe:``, ``:async:`` and ``:const:`` (flags for index entries), ``:provided:``
(a trait member with a default), ``:attributes:`` (``#[non_exhaustive] #[repr(C)]``,
shown above the signature), ``:deprecated: note``, ``:for:`` and ``:trait:`` (on an
impl), ``:target:`` (on a use), and ``:noindex:``.

Roles and ids
-------------

``:rust:mod:`` (crates and modules), ``:rust:fn:`` (functions and methods),
``:rust:struct:``, ``:rust:enum:``, ``:rust:union:``, ``:rust:trait:``,
``:rust:variant:``, ``:rust:field:``, ``:rust:type:`` (type aliases and associated
types), ``:rust:const:`` (constants and associated constants), ``:rust:static:``,
``:rust:macro:``, and ``:rust:any:`` (any of them) link to objects.

Targets are full paths with ``::``, the crate's name first: ``shapes``,
``shapes::parser::Lexer``, ``shapes::parser::Lexer::next_token``, ``shapes::Kind::Rect``,
``shapes::Kind::Rect::width``. As in the Python domain, ``~`` shows only the last part
(``:rust:struct:`~shapes::parser::Lexer``` shows ``Lexer``), a leading ``.`` finds the
name at the end of any target, and a relative name is looked up in the current module
first. ``:rust:any:`` also falls back to intersphinx inventories. So a link to
``std::vec::Vec`` resolves when the standard library's inventory is mapped, and shows
as code otherwise. With ``default-domain:: rust`` (or ``primary_domain = "rust"``), the
roles need no ``rust:``.

An object's id is Sphinx's ``make_id`` of ``rust-`` and its path, with ``::`` written
``.``: ``rust-shapes.parser.Lexer.next_token``. A module's id is ``rust-module-`` and its
path. Index entries read "Lexer (struct in shapes::parser)". ``objects.inv`` lists
every object with the domain ``rust``, modules first.

Generating from source
----------------------

We tell Guidedog where our crates are, relative to the source folder:

.. code-block:: toml

   rust_autoapi_crates = ["../crates/shapes", "../crates/shapes-cli"]

Each entry is a crate folder with a ``Cargo.toml`` (its ``[lib] path``, ``src/lib.rs``,
or ``src/main.rs`` is the root), a folder with ``lib.rs``, ``main.rs`` or ``mod.rs``, a
folder of loose ``.rs`` files (each a public module), or a root ``.rs`` file. Guidedog
follows the module tree the way rustc does (``mod x;`` finds ``x.rs`` or ``x/mod.rs``,
and ``#[path]`` is honored). As the build reads the modules, it writes one page per documented module and an index.
The pages are documents of the project (in the toctree, search,
the general index, ``singlehtml``, and the PDF book). They are never written into
the source folder. Settings:

``rust_autoapi_crates``
   The crate folders (or root files) that get pages, relative to the source folder.
   Only files below each crate's folder are read. A module file elsewhere is refused
   with ``rust.autodoc.confined``. The folders join the roots a ``literalinclude`` may
   read, so a page can quote the code it documents.

``rust_autoapi_root``
   The folder of the pages, ``"rustapi"`` by default. A module's page is named by its
   path: ``rustapi/shapes``, ``rustapi/shapes/parser``. ``rustapi/index`` lists the
   crates and their modules with their synopses.

``rust_autoapi_options``
   Empty by default. ``private-members`` documents private and ``pub(crate)`` items
   (shown with ``:visibility:``). ``hidden-members`` documents ``#[doc(hidden)]`` items.
   ``noindex`` makes every generated object ``:noindex:``.

``rust_autoapi_member_order``
   ``"source"`` (default) or ``"alphabetical"``. Members are grouped in rustdoc's order
   either way (modules, macros, structs, enums, ..., functions).

``rust_autoapi_add_toctree_entry``
   ``true`` (default): the API index joins the root document's first toctree.

``rust_autoapi_generate_api_docs``
   ``true`` (default). ``false`` reads the settings but generates no pages.

``--untrusted`` drops ``rust_autoapi_crates`` folders outside the source folder, with a
warning. Guidedog's own API reference, ``docs/api``, is built this way.

What is documented, and where
-----------------------------

Guidedog follows rustdoc's rules:

- An item is documented in its module when it is ``pub`` and every module on the way is
  ``pub``. ``#[cfg(test)]`` modules and ``#[test]`` functions are never documented.
- ``pub use`` of an item documented anyway is a re-export line. A ``pub use`` of an item
  that is not reachable otherwise (it lives in a private module) documents the item in
  place, under the re-export's name. ``#[doc(inline)]`` and ``#[doc(no_inline)]`` force
  either. ``pub use private::*`` inlines every public item of the module. External
  re-exports (``pub use std::...``) are lines.
- ``#[macro_export]`` macros are documented at the crate root.
- ``impl`` blocks attach to the type their self type names (through ``use`` imports):
  inherent impls first, then trait impls. An impl for a type outside the crate is listed
  under "Implementations" on its module's page. Trait-impl members are ``:noindex:``.
  They are documented on the trait.
- A type's description groups its content as rustdoc does, each group under a rubric: "Fields" (or "Variants"), "Implementations" (inherent impls in source order), then "Trait Implementations" (sorted by trait name).
- A trait ends with "Implementors": the head of every impl of it in the crate, sorted by the implementing type. Each head links its types. The impl's members stay with the type (or on the module page, for a type of another crate).
- Tuple fields, and the fields of enum variants, are listed only when one of them has
  docs. A tuple struct's signature shows private fields as ``_``. With ``include_private``, it shows their types.
- ``#[cfg(...)]`` and ``#[doc(cfg(...))]`` become ``:cfg:``. Several are joined as
  ``all(a, b)``.
- Signatures are the source on one line: attributes, comments and bodies removed,
  whitespace collapsed, rustfmt's vertical lists folded. Constant values longer than 48
  characters and every static's value are left out.

Doc comments
------------

``///``, ``//!``, ``/** */``, ``/*! */``, ``#[doc = "..."]`` and
``#[doc = include_str!("file")]`` (read relative to the source file, only inside the
crate folder) are joined and unindented as rustdoc does. They are CommonMark, and become
reStructuredText in the directive's content:

- Paragraphs, lists (nested, with their start numbers), block quotes, pipe tables (as
  ``list-table``), and HTML blocks (as ``raw:: html``) keep their structure.
- Headings (``# Examples``, ``# Panics``, ``# Errors``, ``# Safety``) become rubrics,
  since sections cannot sit inside directives.
- Code blocks become ``code-block:: rust`` under rustdoc's rules. No info string,
  ``ignore``, ``no_run``, ``should_panic``, ``compile_fail``, ``edition2021``, or an
  error code means Rust. ``text``, ``toml`` and other languages are kept. Hidden ``# ``
  lines are removed (``##`` shows ``#``).
- Intra-doc links (``[`Foo`]``, ``[Foo]``, ``[text](Foo::bar)``, ``[text][Foo]``, with
  disambiguators such as ``struct@`` or ``foo()``) are resolved as rustdoc resolves them,
  relative to the item's module and ``Self``, and become ``rust`` roles. Other links and
  autolinks stay links.
- Links into the standard library become links to doc.rust-lang.org. These are primitives (``[u8]``, ``[`prim@str`]``, ``[u8::MAX]``), paths that start with ``std::``, ``core::`` or ``alloc::``, and prelude names the crate does not define (``[Vec]``, ``[Some]``). The standard library publishes no inventory we could read, so a stable URL is the practical choice. Primitives and common items get their exact page (``primitive.u8.html``, ``struct.Vec.html#method.new``). A disambiguator such as ``macro@`` or ``mod@`` gives the page for others. Anything else opens rustdoc's search for the path. The shown text drops the disambiguator: ``[`prim@u8`]`` shows ``u8``.
- Footnotes (``[^1]``) become auto-numbered footnotes at the end of the item's docs.
- An image on its own line becomes an ``image`` with its alt text, and a linked image (a badge) gets a ``:target:``. An image inside a sentence becomes a substitution. An image with a relative path keeps only its alt text, because nothing places the file next to the generated page.
- A paragraph with a hard line break (two trailing spaces, or a backslash at the end of a line) becomes a line block.
- Everything else reStructuredText would read as markup is escaped, so a comment reads
  as written and never warns.

Rebuilding
----------

Every Rust file a page read is a dependency of that page. When we edit a module's source,
the next build reads again only the pages generated from that crate. ``guidedog
serve`` checks modification times in ``rust_autoapi_crates`` before serving an HTML
page, as it does for ``odin_autoapi_dirs``. We refresh the browser
after editing.

Warnings and limits
-------------------

The source is parsed, never compiled, type-checked, or macro-expanded. Types are shown
as written and nothing is inferred. Items a macro would create are not documented.
Problems in one file never stop the rest of the crate:

``rust.autodoc.parse``
   A syntax error. The rest of the file is documented.
``rust.autodoc.module``
   No file for ``mod x;``.
``rust.autodoc.link`` and ``rust.autodoc.private-link``
   An intra-doc link into the crate that names nothing, or names a private or hidden
   item.
``rust.autodoc.confined`` and ``rust.autodoc.read``
   A file outside the crate folder, or one that could not be read.
``rust.autodoc.nesting``, ``rust.autodoc.size`` and ``rust.autodoc.limit``
   A file nested deeper than 512 levels, a file larger than 4 MiB, or a crate with more
   than 4,096 files or 100,000 items.

A crate root that cannot be found or read is an error (``rust.autodoc.crate``,
``rust.autodoc.root``). Every warning has a hint and, where there is one, the file and
line with an excerpt.

Links to types outside the documented crates (``std::fmt::Display``) resolve only
through intersphinx. With ``-n``, we silence the rest with
``nitpick_ignore_regex = [["rust:any", "(std|core|alloc)::.*"]]``.
