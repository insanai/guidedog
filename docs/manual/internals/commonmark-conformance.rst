Markdown reader: conformance
============================

CommonMark
----------

``crates/guidedoc-commonmark/tests/spec.rs`` runs all 652 examples of the CommonMark
0.31.2 specification (``tests/data/commonmark-spec-0.31.2.txt``) through the
``commonmark`` reader and the HTML renderer in the reference profile, and compares the
HTML exactly. Result: **652 / 652**.

MyST (myst-parser 0.16.1)
-------------------------

``crates/guidedoc-commonmark/tests/myst_conformance`` reads myst-parser 0.16.1's own test
data (``crates/guidedoc-sphinx/tests/data/myst``, MIT license, used as data only; both harnesses read this one copy) with the ``myst`` reader, as a
project reads ``.md`` documents, and compares the document trees. ``MYST_ONLY=<file.md>``
(or ``<directory>/<document>``) checks one fixture and prints its differences;
``MYST_DUMP=<dir>`` writes both normalized trees of every case for diffing.

Most fixtures use directives and roles, which the Sphinx layer provides through the
``Extension`` interface. The reader crate cannot depend on the Sphinx layer, so its copy
of the harness reads without handlers and only reports. The Sphinx layer runs the same
harness with its extension, where every difference must be documented below.

.. list-table::
   :header-rows: 1

   * - Part
     - Cases
     - With the Sphinx layer
     - Reader alone (no handlers)
   * - Renderer fixtures (syntax elements, tables, directive options, Docutils and
       Sphinx directives and roles, amsmath, containers, eval-rst, definition lists)
     - 197
     - 195 match, 2 documented
     - 79 of all 218 fixture cases match; the rest need handlers
   * - Warnings (``reporter_warnings.md``, ``mock_include_errors.md``)
     - 16
     - 15 match, 1 documented
     - (included above)
   * - Includes (``mock_include.md``)
     - 5
     - 5 match
     - (included above)
   * - Sphinx builds (basic, extended syntaxes, field lists, footnotes, includes,
       references, substitutions, commonmark_only, heading_slug_func)
     - 10
     - 8 match, 2 documented
     - 3 match, 2 documented

Without handlers every case outside the documented differences that does not match
differs only where a directive or role would run (languages ``default``/``none`` given
with an extension, roles, admonitions, eval-rst, includes).

What is compared
----------------

Both trees become one form, named by Guidedog's node kinds, compared line by line:
nesting and kinds; text, with white space trimmed at both ends outside verbatim nodes
(``pformat`` drops trailing spaces); the language of code; the kind of admonitions; link
destinations; cross-reference targets; image URIs; raw formats; list styles.
Identifiers, names, classes, spans, and Sphinx's bookkeeping attributes are not
compared. For the warning fixtures, the numbers of warnings and of errors are compared.

Rules for differences of representation
---------------------------------------

.. list-table::
   :header-rows: 1

   * - myst-parser / Sphinx
     - Guidedog
     - Why
   * - hard break as raw ``<br />`` and raw ``\\``
     - HardBreak
     - a node renderers already know
   * - ``target`` for ``equation-*`` and ``index-*`` ids, and after a named link
     - an Id on the equation, index, or the link's name
     - as for reStructuredText
   * - ``index`` nodes before an object description
     - Index inside the Desc
     - :doc:`ast`, Sphinx layer
   * - generated paragraph of ``versionmodified``
     - none; renderers write it
     - as for reStructuredText
   * - ``centered``
     - Paragraph{Class centered}
     - as for reStructuredText
   * - ``hlist`` > ``hlistcol`` > lists
     - Hlist > one List
     - renderers split the columns
   * - ``topic`` "contents"
     - Contents
     - as for reStructuredText
   * - ``literal_strong``, ``literal`` of roles file/samp/kbd, ``inline``
     - Role{text = name}
     - :doc:`ast`, Sphinx layer
   * - ``download_reference``
     - Xref{reftype download}
     - the project host resolves it
   * - ``pending_xref`` target ``path.md#anchor``
     - Xref{doc}{name = path, Custom anchor}; ``#anchor`` of this document:
       Link{Internal}
     - the anchor is resolved in the reader
   * - footnote reference and footnote labels
     - FootnoteReference / Footnote with value = number
     - renderers write the numbers
   * - ``field_body`` holding text
     - FieldBody > Paragraph
     - a FieldBody holds blocks
   * - literal block of Pygments tokens
     - CodeBlock text (white space not compared)
     - highlighting is the renderer's
   * - ``literal_block`` with markup
     - Preformatted
     - as for reStructuredText
   * - ``math_block`` inside a paragraph (``dmath_double_inline``)
     - Math
     - a paragraph holds inlines
   * - math text with the list's indentation
     - white space runs compared as one space
     - LaTeX ignores them
   * - PEP and RFC URLs
     - compared as ``pep:N``, ``rfc:N``
     - the base URLs differ by design
   * - percent-encoded URIs
     - decoded
     - destinations are stored unencoded
   * - a transition that ends a section
     - after the section
     - Docutils' transform, which resolution applies

Documented differences
----------------------

.. list-table::
   :header-rows: 1

   * - Case
     - Difference
     - Reason
   * - ``docutil_directives.md``: class
     - ``{class}`` gives a Python class description
     - with Sphinx, ``class`` is the Python domain's directive (hence ``rst-class``)
   * - ``reporter_warnings.md``: Missing Reference
     - no warning while reading ``[a](b)``
     - it is a cross-reference, which the project host reports when unresolved
   * - ``sphinx_roles.md``: js:class
     - ``:js:class:`` shows no "()"
     - Sphinx 9.1 no longer adds them (sphinx-doc/sphinx#13217)
   * - ``basic/content``
     - wordcount 52 where myst-parser counts 53
     - the count follows the Markdown source of paragraphs, headings, and cells
   * - ``substitutions/index``
     - a directive substitution used inline stays inline text
     - a Paragraph cannot hold an admonition
