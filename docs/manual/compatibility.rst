Compatibility with Sphinx
=========================

Guidedog supports all of reStructuredText and all of Sphinx's built-in extensions.
It supports them in its own way: the output serves the same purpose, but it is not identical to Sphinx's.
Guidedog never runs Python, neither the configuration nor an extension.
It measures compatibility feature by feature.
**A successful build is evidence of a build, not proof of complete equivalence.**

Projects and publication
------------------------

.. list-table:: Project operations
   :header-rows: 1
   :widths: 36 64

   * - Operation
     - Implemented behavior
   * - Project layout and build flags
     - Sphinx-style source and output directories; ``-a -E -W -n -D -t -j -b -M -c -C``.
   * - Configuration
     - TOML data with familiar Sphinx names. Migration converts literals and reports computed values.
   * - HTML builders
     - ``html``, ``dirhtml``, and ``singlehtml``. Single-page anchors include document names.
   * - Other builders
     - ``text``, ``gettext``, and ``dummy``. ``pdf`` uses Typst; ``latex`` and ``latexpdf`` select that route.
   * - Templates
     - Guidedog's Jinja implementation, typed ``html_context``, layout inheritance, and additional pages.
   * - Incremental work
     - Sources, dependencies, templates, configuration, and reference lookups invalidate affected output.
   * - Publication
     - One complete generation. A failed build keeps the previous published generation.

``-D`` accepts ``true`` and ``false``, or ``1`` and ``0``, for booleans.
A project with ``contents`` and no ``index`` can use ``contents`` as its root,
with a warning. Additional template pages use filenames at the site root,
including under ``dirhtml``. See :doc:`templates` for the exact contract.

An unchanged page reports its diagnostics again. Therefore ``-W`` has the same
meaning on a clean build and an unchanged build. A deleted document's page is removed.
Publication uses a recoverable journal. The next build settles an interrupted commit.
See :doc:`internals/publication` for the invariant and its platform boundary.

``coverage`` comes with the ``coverage`` extension, described below.
The builders ``epub``, ``man``, ``texinfo``, ``linkcheck``, ``doctest``, and ``changes`` are unavailable. Their names receive a correction
message. The repository's ``tools/linkcheck`` is a separate verification tool.

Source languages
----------------

.. list-table:: Pinned conformance evidence
   :header-rows: 1
   :widths: 34 66

   * - Reader
     - Evidence and limits
   * - reStructuredText
     - 98 corpus sources match the compared Docutils trees and identifiers. The comparison excludes source positions and several bookkeeping attributes.
   * - CommonMark 0.31.2
     - All 652 specification examples pass.
   * - MyST 0.16.1 reference
     - 215 of 218 reader fixtures and 8 of 10 Sphinx fixture builds match. Remaining differences are recorded.

These are pinned tests. They do not imply compatibility with every later release.
Run ``guidedog formats`` for the executable's reader and renderer evidence.
The reader directories contain their conformance notes.

``rst_prolog`` follows leading bibliographic fields. ``rst_epilog`` follows the
source. Neither is inserted into Markdown. ``only`` and ``ifconfig`` keep their
sections when the condition holds. Their placement can differ from Sphinx when
a conditional section changes the surrounding section level.
Section titles inside MyST ``eval-rst`` are errors.

References and domains
----------------------

Guidedog implements toctrees, labels, glossaries, numbered sections and figures,
``ref``, ``doc``, ``numref``, ``term``, ``download``, ``any``, ``keyword``,
``option``, ``envvar``, and ``token`` references.
Objects can appear in toctrees, sidebars, and the page outline.

The standard, Python, C, C++, JavaScript, reStructuredText, and math domains are
implemented. The Odin and Rust domains are Guidedog's own extensions.
They describe packages or crates, declarations, signatures, and generated API pages.
See :doc:`odin` and :doc:`rust` for their source-only discovery and limits.

Parameter, return, exception, and variable fields form structured descriptions.
Canonical names become aliases for references and inventories.
Python defaults and annotations retain their source spelling.
Sphinx may print them again through Python's unparser.
C++ declarations publish Sphinx-compatible identifier versions and symbol inventories.
Nesting limits still apply. Nested parentheses are parsed in linear time.

``numref`` substitutes one number slot and reports an unnumbered target.
Section numbering follows reading order. A document is numbered once.
A second numbered toctree reports the conflict. See the reference tests for
format substitution and identifier details.

The general index, module index, and search are built in.
Search matches word prefixes. It does not copy Sphinx's English stemming.

Declarative object types replace a useful class of Python extension registrations.
``object_types``, ``crossref_types``, and ``directive_aliases`` describe their data.
A custom Python ``parse_node`` becomes declarative name, display, and program rules.
Migration reports code it cannot express. See :doc:`object-types`.

Built-in extensions
-------------------

Every extension in Sphinx's ``sphinx.ext`` package is built in. A project lists it by its Sphinx name in ``extensions``, and its directives, roles, settings and builders are there. The settings keep Sphinx's names and defaults, so ``guidedog migrate`` carries them over from ``conf.py``. Where Sphinx imports Python, Guidedog reads the source instead. Where Sphinx calls a program, Guidedog uses what it has built in. GDS 0011 records the design of each one.

.. list-table:: Built-in extensions
   :header-rows: 1
   :widths: 30 70

   * - Extension
     - Guidedog's way
   * - ``todo``, ``ifconfig``, ``extlinks``, ``autosectionlabel``
     - As in Sphinx. Hardcoded extlinks can receive a suggested role.
   * - ``graphviz``
     - Linked Graphviz produces static drawings. Resource confinement applies.
   * - ``intersphinx``
     - Local and fetched inventories. Fetching uses the built-in HTTP client.
   * - ``githubpages``
     - Built-in publication files for a static site.
   * - ``mathjax`` and ``imgmath``
     - HTML mathematics uses MathJax. PDF translates its supported LaTeX subset to Typst.
   * - ``doctest``
     - The tests are shown, highlighted. Guidedog never runs Python, so it never runs them.
   * - ``autodoc``
     - Python source is read without importing or executing the package.
   * - ``napoleon``
     - Google and NumPy docstring sections become the fields autodoc shows (:doc:`autodoc`).
   * - ``autosummary`` and ``apidoc``
     - Summary tables and API pages are generated from the source, in memory. Nothing is written into the source folder (:doc:`autodoc`).
   * - ``viewcode``
     - Highlighted source pages under ``_modules/``, linked to the descriptions and back (:doc:`sources-and-diagrams`).
   * - ``linkcode``
     - A URL template, ``linkcode_url``, replaces the Python function ``linkcode_resolve`` (:doc:`sources-and-diagrams`).
   * - ``inheritance_diagram``
     - Class graphs from the source, drawn by the linked Graphviz (:doc:`sources-and-diagrams`).
   * - ``coverage``
     - ``guidedog build coverage`` writes ``python.txt`` from the source and the documents (:doc:`autodoc`).
   * - ``duration``
     - The slowest documents to read, in the build summary and a JSON file (:doc:`sources-and-diagrams`).
   * - ``imgconverter``
     - Accepted, and nothing runs: Typst reads SVG, PNG, JPEG and GIF itself. Other formats are reported (:doc:`sources-and-diagrams`).
   * - ``myst_parser``
     - The implemented project Markdown layer.

Reading source has limits that importing does not. Compiled modules, objects created at run time, external decorators, computed values and extension event hooks cannot be seen. Python 3.13 and the pinned Sphinx documenter tests provide the reference behavior. See :doc:`autodoc` for the full settings and exceptions.

A third-party extension, such as ``sphinx_design``, is not built in. Configuration that names one is refused, and ``guidedog migrate`` leaves it out with a note. An unknown directive or role is reported at its source location.
**Its omitted content must be reviewed before publication.**
Declared object types (:doc:`object-types`) and build plugins (:doc:`plugins`) cover many of them.
Other theme names are accepted using Guidedog's theme implementation.

Trust and resource limits
-------------------------

A normal build accepts the project's raw HTML and Typst, templates, and network
inventories. Local reads stay within the source directory and explicitly shared
roots. A symlink does not grant access outside that boundary.
Templates, static files, includes, images, fonts, and API sources obey the same rule.

Graphviz image resources are resolved relative to their document and confined.
Supported image bytes are embedded in the drawing. ``imagepath`` and ``fontpath``
do not expand the boundary. Typst runs with a private root containing the readable
folders. This keeps book templates and preambles within the declared resources.

``--untrusted`` narrows the read boundary to the source folder.
It omits raw content, unsafe URLs, outside resources, project Typst templates,
and preambles, with diagnostics. It fetches no inventories or Typst packages.
Graphs that ask to read resources are shown as code with a warning.
This mode does not bound the native compiler's memory or execution time.

The host budget defaults to 1 GiB. It is coarse: it counts the sources, included
files, and PDF output the host holds, reserved before they are read. Each worker
reuses one workspace. The retained project graph and catalogs still consume memory
as the project grows. Reading workers coordinate admission.
A refused request reports the failing operation and a budget that fits.
``GUIDEDOG_AVAILABLE_MIB`` can supply known machine headroom. Working memory always stays in memory. There is no disk-backed mode, and ``--memory=disk`` and ``--disk-budget`` are refused.

``--max-depth``, ``--max-nodes``, and ``--stack-kib`` bound document work.
Changing a limit invalidates cached reading and output.
Depth and stack capacity must agree. Native Graphviz, tree-sitter, and Typst
allocations are outside the host budget. We use operating-system limits when those
need a hard bound. See :doc:`errors` and :doc:`internals/memory`.

Verification and real projects
------------------------------

``cargo xtask sphinxdiff-roots`` compares Guidedog with a pinned Sphinx 9.1.0 over 42 of Sphinx's own test roots. It checks pages, identifiers, body links, inventories, numbering, and diagnostics. The stored results explain each difference, for example precise source locations, stable reading-order numbering, and explicit reports for omitted target-specific raw content (:doc:`internals/sphinx-comparison`). The reader conformance tests (:doc:`internals/rst-conformance`, :doc:`internals/commonmark-conformance`) keep their expected outputs, and ``cargo xtask verify-docutils`` checks those against a pinned Docutils 0.22.4. All 98 trees match.

On 15 October 2026 we built the manuals of CPython, Django and Flask as HTML and as PDF books, clean and unchanged (:doc:`evidence/manuals`). Every HTML build succeeded, and the link check found no broken internal link besides two inert ones in CPython. The Django and Flask books built; the CPython book needs more memory than the 12 GB machine had.

Those projects contain unsupported Python extension constructs. Their diagnostics remain part of the evidence. A zero exit status does not mean all their private directives were reproduced. We run with ``-W`` when publishing requires zero diagnostics.

The measurements describe the recorded workloads on the recorded machine. They are not universal speed or memory bounds.
