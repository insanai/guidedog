Compatibility with Sphinx
=========================

Guidedog follows Sphinx where that helps an author move a project.
It measures compatibility feature by feature.
It does not execute Python configuration or arbitrary Python extensions.
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

The builders ``epub``, ``man``, ``texinfo``, ``linkcheck``, ``doctest``,
``coverage``, and ``changes`` are unavailable. Their names receive a correction
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
Section numbering follows reading order. A document is numbered once;
a second numbered toctree reports the conflict. See the reference tests for
format substitution and identifier details.

The general index, module index, and search are built in.
Search matches word prefixes; it does not copy Sphinx's English stemming.

Declarative object types replace a useful class of Python extension registrations.
``object_types``, ``crossref_types``, and ``directive_aliases`` describe their data.
A custom Python ``parse_node`` becomes declarative name, display, and program rules.
Migration reports code it cannot express. See :doc:`object-types`.

Built-in extension behavior
---------------------------

.. list-table:: Extension boundary
   :header-rows: 1
   :widths: 36 64

   * - Behavior
     - Status
   * - ``todo``, ``ifconfig``, ``extlinks``, ``autosectionlabel``
     - Built in. Hardcoded extlinks can receive a suggested role.
   * - ``graphviz``
     - Linked Graphviz produces static drawings. Resource confinement applies.
   * - ``intersphinx``
     - Local and fetched inventories. Fetching runs the system's ``curl`` program.
   * - ``githubpages``
     - Built-in publication files for a static site.
   * - ``mathjax`` and ``imgmath``
     - HTML mathematics uses MathJax. PDF translates its supported LaTeX subset to Typst.
   * - ``myst_parser``
     - The implemented project Markdown layer.
   * - ``doctest`` directives
     - Displayed. The project builder does not execute their tests.
   * - ``autodoc``
     - Python source is read without importing or executing the package.

Source-based autodoc cannot discover every object created at runtime.
Compiled modules, external decorators, computed values, and extension event hooks
have explicit limits. Python 3.13 and the pinned Sphinx documenter tests provide
the reference behavior. See :doc:`autodoc` for the full settings and exceptions.

``autosummary``, ``napoleon``, ``viewcode``, and arbitrary Python extensions remain
outside this route. An unsupported extension named in configuration is reported.
An unknown directive or role is reported at its source location.
**Its omitted content must be reviewed before publication.**
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
reuses one workspace; the retained project graph and catalogs still consume memory
as the project grows. Reading workers coordinate admission.
A refused request reports the failing operation and a budget that fits.
``GUIDEDOG_AVAILABLE_MIB`` can supply known machine headroom. The Odin
implementation's disk-backed working memory (``--memory=disk``, ``--disk-budget``)
is not part of the Rust implementation.

``--max-depth``, ``--max-nodes``, and ``--stack-kib`` bound document work.
Changing a limit invalidates cached reading and output.
Depth and stack capacity must agree. Native Graphviz, tree-sitter, and Typst
allocations are outside the host budget. Use operating-system limits when those
need a hard bound. See :doc:`errors` and :doc:`internals/memory`.

Verification and real projects
------------------------------

The Odin implementation's ``tools/sphinxdiff`` compared 42 pinned Sphinx test roots.
It checked pages, identifiers, body links, inventories, numbering, and diagnostics,
and its stored results explained deliberate differences: precise source locations,
stable reading-order numbering, and explicit reports for omitted target-specific raw
content, for example. The Rust crates keep the reader conformance tests
(:doc:`internals/rst-conformance`, :doc:`internals/commonmark-conformance`) and their
expected outputs.

The remote validation of the Odin implementation on 1 October 2026 ran clean and
unchanged HTML and PDF builds for CPython, Django, and Flask. All twelve builds
succeeded and retained the baseline diagnostics and link results. It also passed 1,039 Linux tests
and 94 targeted AddressSanitizer tests.

Those projects contain unsupported Python extension constructs.
Their diagnostics remain part of the evidence. A zero exit status does not mean
all their private directives were reproduced. Run with ``-W`` when publishing
requires zero diagnostics.

The historical measurements and source revisions remain in
``docs/manual/evidence/manuals.rst``. They were made with the Odin implementation
(release 0.6); user-facing behavior carries over to the Rust implementation, but its
timings and memory use must be measured again. These measurements describe the
recorded workloads. They are not universal speed or memory bounds.
