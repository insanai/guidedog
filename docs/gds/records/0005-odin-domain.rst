:gds-number: 0005
:gds-title: An Odin domain and Odin API documentation
:gds-state: committed
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-09-30
:gds-updated: 2026-10-15
:gds-discussion: No external discussion URL assigned
:gds-labels: ["architecture", "build"]

An Odin domain and Odin API documentation
=========================================


Abstract
--------


Guidedog documents Odin APIs as Sphinx documents Python ones. An ``odin`` domain, always
on, describes packages, procedures, procedure groups, structs, unions, enums, bit sets,
bit fields, other types, constants, variables, foreign imports, fields, and enumerators,
with signatures written and shown as Odin writes declarations. Roles link to them. Each
object has an id, an index entry, a table of contents entry, and an ``objects.inv`` entry.
Generating directives (``odin:autopackage`` and five others) and API pages
(``odin_autoapi_dirs``, as sphinx-autoapi's ``autoapi_dirs``) write that domain's
reStructuredText from Odin source, parsed as Odin's own ``core:odin/parser`` parses it. The
compiler never runs. The ``rust`` domain and ``gd-rustdoc`` follow the same design for Rust.


Motivation and scope
--------------------


Odin has no documentation tool that produces a cross-referenced site and book with prose
around the reference. Sphinx has no Odin
domain. The Python pieces Guidedog already has (the Python domain, ``sphinx.ext.autodoc``
from static analysis, the project's object tables and inventories) give the design.
This record applies it to Odin.

In scope: the domain in the Sphinx reader (``guidedoc-sphinx``),
``gd-odindoc``, a host library that analyses Odin packages and writes the domain's text, and
the project's generating directives, API pages, cross-references, inventory, and
incremental rebuilds. Out of scope: type checking, evaluating constants, and running
anything.


Specification
-------------


The domain
~~~~~~~~~~


``odin:package`` sets the current package, as ``py:module`` sets the module, with
``:synopsis:``, ``:platform:``, ``:deprecated:``, ``:no-index:``, and ``:imports:``, the import
aliases the package's signatures use (``gd=core rst=readers/rst``). A package is named by
its import path: ``core:fmt`` for a collection, or its folder below a project root
(``readers/sphinx``). ``odin:currentpackage`` changes the context without a target.


.. list-table::
   :header-rows: 1
   :widths: 1 1

   * - **Directive**
     - **Signature, as Odin writes it**
   * - ``odin:procedure``, ``odin:proc``
     - ``[@(attrs)] name :: [#force_inline ]proc["c"](params)[ -> results][ tags][ where ...]``,
            or the short form ``name(params) -> results``
   * - ``odin:procgroup``
     - ``name :: proc{a, b}``; each member links to its procedure
   * - ``odin:struct``, ``odin:union``
     - ``Name :: struct($T: typeid) #packed``, ``Name :: union #no_nil {A, B}``
   * - ``odin:enum``, ``odin:bitset``, ``odin:bitfield``
     - ``Name :: enum u8``, ``Name :: bit_set[E; u8]``, ``Name :: bit_field u32``
   * - ``odin:type``
     - distinct types and aliases: ``Handle :: distinct uintptr``
   * - ``odin:field``, ``odin:enumerator``
     - inside a type: ``name: T`` with a tag or a bit size; ``Name = 3``
   * - ``odin:const``, ``odin:var``
     - ``NAME :: 64``, ``NAME : int : 64``; ``name: T = value``
   * - ``odin:foreign``
     - a foreign import: ``libc "system:c"``


Every object takes ``:no-index:``, ``:no-index-entry:``, ``:no-contents-entry:``,
``:no-typesetting:``, and the domain's own ``:package:``, ``:private:``, ``:deprecated:``,
``:availability:`` (a first paragraph "Availability: Windows, Linux."), and ``:foreign:``.
Members nested in a type are named ``Type.member``. Later signature lines of one directive
are the same object on other targets. They get no id and no entry.

Signatures become ``Desc*`` nodes: attributes and keywords as ``DescAnnotation``, the
package's short name as ``DescAddname`` (with ``add_module_names``), the name, parameters
as a ``DescParameterlist``, and named results as a second one after the text " -> ". Names in
type positions become ``Xref{odin:type}`` with the context attributes ``odin:package`` and
``odin:type``. An alias of ``:imports:`` is followed to its package in the reader. Built-in
types, keywords, polymorphic names (``$T`` and ``T`` after it, including a nesting type's),
array lengths, enumeration names, defaults, constants, tags, and where clauses stay text.
So nothing that cannot be an object produces a broken link. The Python domain likewise
leaves built-ins to its builtin resolver.


Ids, references, and the inventory
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


An object's id is Sphinx's ``make_id("odin-" + package + "." + name)``:
``odin-core-fmt.println``, ``odin-readers-sphinx.Config.docname``. A package's is
``odin-package-core-fmt``. ``make_id`` keeps case, dots, and underscores, as for Python ids,
and turns the import path's ``:`` and ``/`` into hyphens. So ids are readable in URLs, the same
in every build, and apart for packages that share a last segment. The alternative,
``<package>.<name>`` as for Python, would put ``:`` and ``/`` into ids.

Roles ``pkg``, ``proc``, ``type``, ``const``, ``var``, ``field``, ``enumerator``, and ``obj`` take the
Python domain's ``~`` and leading ``.``. A target is looked up as
``package.Type.name``, ``package.name``, the name as written, then by the end of an object's
name after a ``:`` or ``/``. So ``fmt.println`` finds ``core:fmt.println``, as Odin code names a
package by its last segment. A leading ``.`` searches any suffix, filtered by the role's
object types. Index entries read "println (procedure in core:fmt)". ``objects.inv``
lists every object as ``odin:<objtype>``, packages with priority 0.


Generating from source
~~~~~~~~~~~~~~~~~~~~~~


``gd-odindoc`` is a host library that uses only the standard library. It reads every
``.odin`` file of a package folder except ``*_test.odin`` and ``#+ignore`` files through the
host's confined reader, plugged in as a ``FileReader``. Its tokenizer and declaration-level
parser read every file-scope statement, expression and type as Odin's ``core:odin/parser``
does, with the same spans and the same comment grouping, and skip procedure bodies, which
declare nothing to document. Expressions are flat nodes indexed by ``NodeId``. It records
declarations in source order: kind, signature as written (the source span normalized to
one line, never a procedure's body), attributes, doc comment (the comment group directly
above, and a field's or enumerator's comment at the end of its line), privacy
(``@(private)``, ``@(private="file")``, ``#+private``), members, and targets.

``odin:autopackage``, ``odin:autoproc``, ``odin:autoprocgroup``, ``odin:autotype``,
``odin:autoconst``, and ``odin:autovar`` hand the request to the host, as autodoc's
directives do, and parse the answer where they stand. Options:
``:members:``, ``:undoc-members:``, ``:private-members:``, ``:exclude-members:``,
``:member-order:`` (source, alphabetical, groupwise), ``:no-index:``.

Doc comments are plain text and become reStructuredText that reads as written.
Paragraphs and lists stay. An indented block is ``code-block:: odin`` (``text`` after
"Output:"). "Inputs:" and "Returns:" items become ``:param name:`` and ``:result name:``
fields (a new ``result`` field of the domain) or ``:returns:``. "NOTE:" and "WARNING:" lines
are admonitions. Backquoted code is literal. Every other markup character is escaped.


Platforms
~~~~~~~~~


Files are read as Odin builds them for every target at once. File-name suffixes
(``_windows``, ``_linux_arm64``, ``_amd64``) and ``#+build`` tags (negations, the older
``//+build``) give a file's targets, a set of (OS, architecture) pairs Odin supports.
File-scope ``when`` blocks comparing ``ODIN_OS`` and ``ODIN_ARCH`` narrow them. Declarations of
one name and signature merge, with the union of their targets. One made for every target
of the package is documented once. One made for some says where ("Availability:
Windows"). One whose signature differs per target is documented once per signature, the
first indexed. A package built only for some targets says so with ``:platform:``. No
setting is needed.


API pages
~~~~~~~~~


``odin_autoapi_dirs`` lists folders, relative to the source folder. Each package below
them gets a page ``<odin_autoapi_root>/<path>`` holding ``odin:autopackage`` with
``odin_autoapi_options`` and ``odin_autoapi_member_order``. ``<root>/index`` lists the
packages with their synopses in a hidden toctree. The pages are virtual documents.
``discover`` adds them to the project's sources with their text, the build reads them as
it reads files, and they never appear in the source folder. They take part in every
builder, search, the general index, and the book. ``odin_autoapi_add_toctree_entry`` adds
the index to the root document's first toctree. ``odin_collections`` names folders for
import-style arguments (``core:strings``) and gets no pages. All settings are checked when
``conf.toml`` loads, with Elm-style diagnostics.


.. list-table::
   :header-rows: 1
   :widths: 1 1 1

   * - **Setting**
     - **Default**
     - **Meaning**
   * - ``odin_autoapi_dirs``
     - ``[]``
     - folders whose packages get pages
   * - ``odin_collections``
     - ``{}``
     - collection names to folders
   * - ``odin_autoapi_root``
     - ``"api"``
     - the pages' folder
   * - ``odin_autoapi_options``
     - ``["members", "undoc-members"]``
     - options of each page
   * - ``odin_autoapi_member_order``
     - ``"source"``
     - source, alphabetical, groupwise
   * - ``odin_autoapi_add_toctree_entry``
     - ``true``
     - the index joins the root toctree
   * - ``odin_autoapi_generate_api_docs``
     - ``true``
     - ``false``: directives only


Every Odin file read is a dependency of the page that read it, so
editing a package reads only its page again. The index's text holds the synopses, so its
digest changes when one does. A page whose package was not found is read again on every
build.


Rationale and alternatives
--------------------------


The Python domain's design was kept wherever Odin allows, so a Sphinx user needs to learn
nothing new: the same options, the same info fields, the same ``~`` and ``.``, the same
nesting of members, and the same inventory. Signatures are Odin's own syntax rather than
a C-like one, since Odin readers read declarations that way.

A parser that reads declarations as ``core:odin/parser`` does, with the same spans and
comment grouping, reads every syntax Odin accepts at file scope. Running ``odin doc`` would give types checked
by the compiler, but would run a compiler at build time, for one target at a time, and
could not see the other targets' files. Static analysis documents the whole
cross-platform API.

Virtual documents, rather than files written into the source folder as sphinx-autoapi
writes them, keep the source folder the author's and let incremental builds work on
dependencies as autodoc's pages do.


Security and operational considerations
---------------------------------------


Building never compiles or runs Odin code. Source is read only below
``odin_autoapi_dirs`` and ``odin_collections``, through ``gd_host::confined``, as includes are, and
within ``Limits::max_file_bytes`` (``odin.autodoc.memory``). The parser counts how deeply it
recurses and leaves a file nested past 160 levels out with ``odin.autodoc.nesting``. A
package that does not parse is reported with a warning, and what could be read is
documented.


Backwards compatibility
-----------------------


The domain adds directives and roles under ``odin:``, a name Sphinx does not use. Projects
that do not use it are unaffected, and their documents read as before. The environment's
version changed because documents may now be virtual.


Acceptance criteria and implementation status
---------------------------------------------


Implemented: the domain (``crates/guidedoc-sphinx/src/domains/odin.rs`` and its tests),
``gd-odindoc`` with its tests, and the project's generating directives, API pages,
resolution, inventory, and dependencies (``tests/integration/tests/tools/autodoc.rs`` and
``incremental.rs``). API page counts are build observations, not a fixed part of the
domain contract.


Open questions
--------------


- Private types that public declarations mention cannot be linked. A project lists them
  in ``nitpick_ignore``. Should the generator show them without a link?
- Whether ``A :: B`` declares a type or a constant is told from the names. The compiler
  would know.
- ``when`` conditions other than ``ODIN_OS`` and ``ODIN_ARCH`` comparisons are not evaluated.
- A package module index (``odin-modindex``), as the Python module index, is not written.


Review history
--------------


2026-09-30: drafted with the implementation.


References
----------


- GDS 0003, the Guidedoc engine.
- GDS 0006, Sphinx-style projects and recoverable publication: domains, autodoc,
  and the native project model.
- Sphinx 9.1, ``sphinx.domains.python`` and ``sphinx.ext.autodoc``; sphinx-autoapi.
- Odin's ``core:odin/parser``, ``core:odin/ast``, and its file-name and ``#+build`` rules.

.. rubric:: Lifecycle event

::

   2026-10-11: prediscussion -> discussion. Assigned a permanent number and opened
   for discussion.

.. rubric:: Lifecycle event

::

   2026-10-11: discussion -> accepted. Maintainer-requested implementation-status
   reconciliation; current supported contract reviewed and deferred scope stated
   explicitly.

.. rubric:: Lifecycle event

::

   2026-10-11: accepted -> committed. Current supported design implemented; beta
   review fixes and limitations are recorded. Library and integration
   suites; embedded-backend validation;
   docs/manual/evidence/beta-review-20261011.md. Deferred features are not
   claimed as implemented.
