Documenting Odin
================

The Odin domain gives packages, procedures, types, and members named targets.
Guidedog also generates API pages by parsing Odin source. It does not run the
compiler or execute the package. A generated signature still needs a useful contract.

Describing objects
------------------

A package is the context of what follows it, as a Python module is:

.. code-block:: rst

   .. odin:package:: core:strings
      :synopsis: Procedures to manipulate UTF-8 encoded strings.
      :imports: rt=base:runtime

   .. odin:procedure:: @(require_results) clone :: proc(s: string, allocator := context.allocator) -> (res: string, err: rt.Allocator_Error) #optional_allocator_error

      Clones a string.

      :param s: The string to be cloned.
      :result res: The cloned string.
      :result err: An allocator error, or ``nil``.

   .. odin:struct:: Builder :: struct

      A dynamic byte buffer.

      .. odin:field:: buf: [dynamic]byte

Signatures are written as Odin writes declarations, on one line, and shown the same
way. This is how the ones below look:

.. odin:package:: shapes
   :no-index:

.. odin:enum:: Kind :: enum u8
   :no-index:

   .. odin:enumerator:: Circle = 1
      :no-index:

.. odin:procedure:: @(require_results) area :: proc(side: f32, scale: f32 = 1) -> (a: f32, ok: bool) #optional_ok
   :no-index:

   Measures a square.

   :param side: Its side.
   :result a: Its area.
   :result ok: Whether its kind is known.

The directives, and the signatures they take:

``odin:package`` and ``odin:currentpackage``
   ``core:fmt``, or a path below a project's roots, such as ``readers/sphinx``. The
   package's import path names its objects: ``core:fmt.println``.
   ``odin:currentpackage`` changes the context without a target. ``None`` clears it.
   Options: ``:synopsis:``, ``:platform:``, ``:deprecated:``, ``:no-index:``,
   ``:imports:``, and ``:private:``. ``:imports:`` lists the import aliases the package's signatures use, as
   ``alias=package`` pairs (``gd=core rst=readers/rst``), so ``gd.Node_Id`` links to
   ``core.Node_Id``. ``:private:`` lists the package's private types and the private members of its
   procedure groups (``State Rules catalog_text``). Its signatures show them as text,
   since they have no descriptions to link to. The generated pages list them
   automatically.

``odin:procedure`` (or ``odin:proc``)
   ``name :: proc(params) -> results``, with attributes before the name
   (``@(require_results)``), ``#force_inline`` before ``proc``, a calling convention
   (``proc "c" (...)``), parameters with defaults (``x := 1``, ``x: int = 1``), ``$T``
   polymorphic parameters, ``..`` variadics, ``using``, ``#c_vararg``, named results,
   tags (``#optional_ok``), and a ``where`` clause. ``name(params) -> results`` is
   short for the same.

``odin:procgroup``
   ``name :: proc{a, b}``: each member links to its procedure.

``odin:struct``, ``odin:union``, ``odin:enum``, ``odin:bitset``, ``odin:bitfield``, ``odin:type``
   ``Name :: struct($T: typeid) #packed``, ``Name :: union #no_nil {A, B}``,
   ``Name :: enum u8``, ``Name :: bit_set[Flag; u8]``, ``Name :: bit_field u32``, and
   for ``odin:type``, distinct types and aliases (``Handle :: distinct uintptr``,
   ``Callback :: proc(x: int) -> bool``). A type's content holds its members.

``odin:field`` and ``odin:enumerator``
   Inside a type: ``name: Type`` with a tag (``name: string `json:"n"```) or a bit
   size (``low: u8 | 3``), and ``Name`` or ``Name = 3``. Their names are
   ``Type.member``.

``odin:const``, ``odin:var``, ``odin:foreign``
   ``NAME :: 64`` or ``NAME : int : 64``; ``name: Type``, ``name := value``, or
   ``name: Type = value``; ``libc "system:c"`` for a foreign import.

Every object takes the options objects take in Sphinx's domains: ``:no-index:``,
``:no-index-entry:``, ``:no-contents-entry:``, and ``:no-typesetting:``. It also takes these of
its own: ``:package:`` (describe it in another package), ``:private:`` and
``:deprecated: message`` (shown as ``@(private)`` and ``@(deprecated="message")``
unless the signature has them), ``:availability: Windows, Linux`` (a first line of the
content saying where it exists), and ``:foreign: libc`` (a procedure of a foreign
block). A directive with several signature lines describes one object whose signature
differs between targets. Only the first line gets the id and the index entry.

Names in type positions link to the types they name. They are looked up first in the package and type the
signature is written in, then as written. Built-in types (``int``, ``string``,
``rawptr``, ``typeid``, ``any``, ...), keywords, polymorphic names (``$T``, and ``T``
after it, in the signature or in the type the object is nested in), array lengths,
default values, constants, tags, and ``where`` clauses are shown as written, never as
links.

Roles and ids
-------------

``:odin:pkg:``, ``:odin:proc:``, ``:odin:type:`` (structs, unions, enums, bit sets, bit
fields, and other types), ``:odin:const:``, ``:odin:var:``, ``:odin:field:``,
``:odin:enumerator:``, and ``:odin:obj:`` (any of them) link to objects. As in the
Python domain, ``~`` shows only the last part (``:odin:proc:`~core:fmt.println``` shows
``println``), and a leading ``.`` finds the name at the end of any object's name. A name
is looked up in the reference's type and package first, then as written, then after a
package path's ``:`` or ``/``. So ``fmt.println`` finds ``core:fmt.println``, and
``sphinx.Config`` finds ``readers/sphinx.Config``, as Odin code names a package by its
last segment. An import alias the current package declares with ``:imports:`` is
followed first. With ``default-domain:: odin`` (or ``primary_domain = "odin"``), the
roles need no ``odin:``.

An object's id is Sphinx's ``make_id`` of ``odin-`` and its full name:
``odin-core-fmt.println``, ``odin-readers-sphinx.Config.docname``. Case and dots are
kept, as Sphinx keeps them for Python ids. The ``:`` and ``/`` of the import path become
hyphens. Ids are therefore readable in URLs, the same in every build, and distinct for two
packages with the same last name. A package's id is ``odin-package-`` and its path.
Index entries read "println (procedure in core:fmt)". ``objects.inv`` lists every
object with the domain ``odin`` and its type (``odin:procedure``, ``odin:struct``,
``odin:field``, ...), packages first, as Sphinx lists modules.

Info fields in the content are grouped as in the other domains: ``:param name:`` (with
``:type name:``), ``:result name:`` for a named result, ``:returns:``, and ``:rtype:``.

Generating from source
----------------------

We tell Guidedog where our packages are, relative to the source folder:

.. code-block:: toml

   odin_autoapi_dirs = ["../src"]
   odin_collections = {core = "/usr/local/lib/odin/core"}  # optional

A package below ``odin_autoapi_dirs`` is named by its path there (``src/shapes/round``
is ``shapes/round``). A package in a collection is named as Odin imports it
(``core:strings``). The generating directives then work as autodoc's do:

.. code-block:: rst

   .. odin:autopackage:: shapes
      :members:
      :undoc-members:
      :member-order: groupwise

   .. odin:autoproc:: shapes.area
   .. odin:autotype:: Shape

``odin:autopackage`` writes the package, its doc comment, and with ``:members:`` its
declarations (all of them, or those listed). ``odin:autoproc``, ``odin:autoprocgroup``,
``odin:autotype``, ``odin:autoconst``, and ``odin:autovar`` write one declaration,
named in the current package or as ``package.name``. Options: ``:members:``,
``:undoc-members:``, ``:private-members:`` (``@(private)`` declarations, which are left
out otherwise), ``:exclude-members:``, ``:member-order:``, and ``:no-index:``.
``:member-order:`` is ``source`` (the default, by
file name and then position), ``alphabetical``, or ``groupwise``. ``groupwise`` heads the
groups "Types", "Procedures", "Procedure groups", "Constants", "Variables", and
"Foreign imports".

Doc comments are the ``//`` lines directly above a declaration, or a ``/* */`` block
there, and a field's or enumerator's comment at the end of its line. They are plain
text, and become reStructuredText as follows:

- Paragraphs stay paragraphs. A list of ``- item`` lines stays a list.
- An indented block (a tab or spaces) after a blank line, or after a line ending in a
  colon (``Example:``), is a ``code-block:: odin``, or ``text`` after ``Output:``. A
  line indented more than the text directly after a paragraph line continues it.
- ``Inputs:`` followed by ``- name: text`` items become ``:param name:`` fields, and
  ``Returns:`` items ``:result name:`` fields (or ``:returns:`` when they have no names),
  as Odin's core library writes them.
- A line starting ``NOTE:`` or ``WARNING:`` is a note or a warning.
- ```code``` is literal text. Every other character reStructuredText would read as
  markup (``*``, ``|``, a ``_`` ending a word, ...) is escaped, so the comment reads as
  written and never warns.

API pages
---------

With ``odin_autoapi_dirs`` set, every package below those folders also gets a page of
its own. The build generates it as it reads it, like sphinx-autoapi does. ``api/shapes`` holds
``odin:autopackage:: shapes`` with ``odin_autoapi_options``. ``api/index`` lists every
package with its synopsis. The pages are documents of the project (in the toctree,
search, the general index, ``singlehtml``, and the PDF book). They are never written
into the source folder. Settings:

``odin_autoapi_dirs``
   The folders whose packages get pages, relative to the source folder. Only files below
   them (and below ``odin_collections``) are read, and a link leading elsewhere is
   refused.

``odin_collections``
   A table of collection names and folders, for directive arguments such as
   ``core:strings``. These packages get no pages.

``odin_autoapi_root``
   The folder of the pages, ``"api"`` by default. A document of the source folder with a
   page's name keeps its name, with a warning.

``odin_autoapi_options``
   ``["members", "undoc-members"]`` by default. ``private-members`` may be added.

``odin_autoapi_member_order``
   ``"source"`` (default), ``"alphabetical"``, or ``"groupwise"``.

``odin_autoapi_add_toctree_entry``
   ``true`` (default): ``api/index`` joins the root document's first toctree.

``odin_autoapi_generate_api_docs``
   ``true`` (default). ``false`` reads the folders for the directives only.

Types from packages we do not document (``core:io.Writer``) cannot be linked. With
``-n``, we silence them with ``nitpick_ignore_regex = [["odin:type", "(core|base):.*"]]``.
Guidedog's own API reference, ``docs/api``, is built the same way from Rust source
with ``rust_autoapi_crates``. See :doc:`rust`.

Platforms
---------

A package is documented once for every target Odin builds, as Odin itself sees its
files. A file named ``*_windows.odin``, ``*_linux_arm64.odin``, or ``*_amd64.odin``
builds only for those targets. ``#+build linux, darwin`` and ``#+build !windows`` tags
(and the older ``//+build``) narrow it. A file-scope ``when ODIN_OS == .Windows``
block counts for its targets (and its ``else`` for the others) when its condition only
compares ``ODIN_OS`` and ``ODIN_ARCH``. Files named ``*_test.odin``, and files tagged
``#+ignore``, are left out. A declaration made for every target the package builds for
is shown once. One made for some targets says which, "Availability: Windows". One whose
signature differs between targets is shown once per signature, each with its targets,
the first with the id. A package that builds only for some targets says so with its
``:platform:``.

Rebuilding
----------

Every Odin file a page read is a dependency of that page. When we edit a package's source,
the next build reads again only the pages that document it. The package index is
read again when a package appears, disappears, or changes its synopsis. ``guidedog
serve`` checks modification times in ``odin_autoapi_dirs`` and ``odin_collections``
before serving an HTML page. We refresh the browser after editing.

Limits
------

- The source is parsed, never compiled or type-checked. Types are shown as written, a
  constant's value as written (and left out beyond 100 characters), and nothing is
  inferred. Whether ``A :: B`` declares a type or a constant is told from the names: a
  capitalized name not in capitals throughout, or a built-in type, is a type.
- Conditions of ``when`` blocks other than comparisons of ``ODIN_OS`` and
  ``ODIN_ARCH`` are not evaluated: both branches are documented.
- A declaration that mentions a private type shows it as text, since private
  declarations have no entries unless ``:private-members:`` is given. The generated
  package lists them in ``:private:``, so ``-n`` has nothing to report.
- Guidedog reads declarations with its own tokenizer and parser for Odin. The parser
  skips procedure bodies. A syntax error in a declaration is reported
  (``odin.autodoc.syntax``), and what could be read is still documented.
- A declaration nested deeper than 160 levels of brackets or types is far deeper than
  real code. Its file is left out with the warning ``odin.autodoc.nesting``, and the
  rest of its package is documented. Long flat chains (``a + b + ...``) and anything
  inside procedure bodies do not count. A ``when`` condition is evaluated to 64 levels
  of ``&&``, ``||`` and ``!``. A longer one counts as not evaluated.
- A file larger than the per-file limit is left out with ``odin.autodoc.memory``.
