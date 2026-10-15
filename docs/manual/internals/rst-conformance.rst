reStructuredText reader: conformance with Docutils
==================================================

``crates/guidedoc-rst/tests/conformance.rs`` reads every source of ``crates/guidedoc-rst/tests/data/docutils/corpus.txt``
with Guidedoc and compares the document tree with the one Docutils 0.22 builds
(``tests/data/docutils/expected``, stored by the Odin project's ``tools/rstref``). Run it with

::

    cargo test -p guidedoc-rst --test conformance

The environment variable ``RST_ONLY=<source>`` checks one source; ``RST_DUMP=<dir>`` writes both
normalized trees of every source for diffing.

Result
------

.. list-table::
   :header-rows: 1

   * - Corpus part
     - Sources
     - Match
   * - Specification (``docs/ref/rst``: restructuredtext, directives, roles, introduction, definitions, history, mathematics) and ``docs/header2.rst``
     - 8
     - 8
   * - User documentation (``docs/user/rst``: quickstart, cheatsheet, demo)
     - 3
     - 3
   * - Docutils functional test inputs (``test/functional/input``, with ``data/``)
     - 66
     - 66
   * - Guidedoc's own sources for the settings (``extra/``)
     - 21
     - 21
   * - Total
     - 98
     - 98


Every source matches in structure, text, key attributes, reference targets, and the
number of warnings and errors, with the comparison rules below. One source compares
the total number of problems instead of warnings and errors separately (see
"Deliberate differences").

What is compared
----------------

Both trees are brought into one form whose node kinds are Guidedoc's kind names, then
compared line by line:

- nesting and node kinds;
- text: white space runs are one space, except in literal blocks, doctest blocks, math,
  raw content, and parsed literal blocks, which compare exactly;
- list style, bullet, delimiter, and start; table column count and cell spans;
  option-argument delimiters; footnote and citation labels (the number or symbol
  resolution gave); image URI, alt, width, height, scale, and align; figure width
  and align; raw formats; classes (as sets); bibliographic field keys;
- references: an external reference by URI, an internal one by what it leads to (the
  kind and first 40 characters of the text of the referenced element, the title for a
  section), unresolved ones by their source text;
- the number of warnings and of errors: Docutils' system messages of level 2, and of
  levels 3 and 4, against Guidedoc's reports of severity Warning and Error.

Identifiers, names, source positions, back-references, and table column widths are not
compared; anchors are the renderers' concern (``build_anchors``), and "Ids" below compares
them.

Ids
---

The ids of every corpus source's HTML page are compared with those Docutils 0.22.4's
html5 writer gives (``tests/data/docutils/ids.txt``, with Sphinx's
``auto_id_prefix = "id"``) by the HTML renderer's tests, since anchors are the
renderers' concern (``guidedoc::build_anchors``); the reader only gives the tree they are
computed from. All 98 sources have Docutils' ids, except the ids of system messages,
which Guidedoc does not write (see "Deliberate differences").

Rules for differences of representation
---------------------------------------

These are the same content represented differently; the comparison maps one to the
other.

.. list-table::
   :header-rows: 1

   * - Docutils
     - Guidedoc
     - Why
   * - ``problematic`` around text of failed markup
     - the source text, and a report
     - there is no problematic node kind
   * - ``system_message`` elements, the "Docutils System Messages" section
     - reports
     - counted, not compared as nodes
   * - substitution references replaced by the definition's content
     - SubstitutionReference resolved to the SubstitutionDefinition
     - renderers expand it; the comparison expands it, with Unicode ``ltrim``/``rtrim``
   * - ``topic`` "contents" holding the generated table of contents
     - Contents{Title, Local}
     - the host builds the table; title and locality are compared
   * - ``generated`` section numbers
     - Directive{sectnum}
     - renderers number sections; the presence of numbering is compared
   * - footnotes and references made by ``target-notes``
     - Directive{target-notes}
     - renderers make them, with the ids Docutils gives them (``Anchors::anchor_extras``)
   * - ``decoration`` > ``header``, ``footer``; ``meta`` moved to the top
     - Directive{header, footer, meta} where written
     - compared at the top in Docutils' order
   * - empty ``target`` after a named embedded URI or alias
     - the Link or Reference names the target
     - the name is on the reference
   * - chained target (``refid`` to a target with ``refuri``)
     - Target with the final URI
     - followed to the URI
   * - bibliographic element (``author``, ``date``, ...)
     - Field{Custom docinfo = key} with its FieldName kept
     - the field name is compared as the key
   * - ``authors`` > ``author`` (repeated)
     - Field{docinfo=authors} > FieldBody > one Paragraph per author
     - \
   * - ``address`` (preserved white space)
     - Paragraph turned Preformatted
     - \
   * - section ``subtitle``
     - Paragraph{Class section-subtitle} second in the Section
     - the core allows Subtitle only in Document and Sidebar; see "Core"
   * - ``literal_block`` with inline markup / without
     - Preformatted / CodeBlock
     - parsed literal blocks without markup compare as literal blocks
   * - ``literal`` class "code", literal_block classes "code <language>"
     - Code / CodeBlock with Language
     - the kind says it is code
   * - ``acronym``
     - Abbreviation
     - one kind for both
   * - ``reference`` > ``image`` at block level
     - Paragraph > Link or Reference > Image
     - a link is inline in Guidedoc
   * - ``figure`` without width for ``:figwidth: image``
     - Figure{Custom figwidth = image}
     - Docutils reads the image file for its size; renderers size the figure to the image
   * - a trailing line break in literal and raw text
     - CodeBlock and RawBlock text ends with "\\n"
     - the AST contract


Deliberate differences
----------------------

.. list-table::
   :header-rows: 1

   * - Difference
     - Reason
   * - PEP and RFC link URLs: Guidedoc's defaults are ``https://peps.python.org/pep-0008/`` and ``https://datatracker.ietf.org/doc/html/rfc2822``; Docutils' are ``https://peps.python.org/pep-0008`` and ``https://tools.ietf.org/html/rfc2822.html``
     - the project's link style; ``pep_base_url`` and ``rfc_base_url`` select Docutils-style URLs. The comparison reduces both to "pep:8" and "rfc:2822"
   * - Line breaks inside inline literals and ``code`` become spaces; math and raw keep them
     - inline code renders on one line in every target; LaTeX needs line breaks in math (comments end at them). The comparison turns breaks in inline code into spaces
   * - ``dangerous.rst`` (read with file insertion disabled): refused includes and ``:url:`` options are errors in Guidedoc, warnings in Docutils (corpus setting ``messages=total``)
     - content left out by policy is an error, so a conversion does not silently lose text; the number of problems is the same
   * - ``dangerous.rst`` is read without Docutils' ``raw_enabled = False``
     - Guidedoc's reader keeps raw content and renderers apply ``Policy::raw``, reporting what they refuse
   * - The specialized roles (``named-reference``, ``anonymous-reference``, ``footnote-reference``, ``citation-reference``, ``substitution-reference``, ``target``, ``uri-reference``) are implemented; Docutils reports them as not implemented
     - the specification describes them; no corpus source uses them. Tests: ``specialized_roles_name_their_constructs``
   * - System messages are not written into the HTML page, so their ids (listed apart in ``ids.txt``) are not on it; the ids are given all the same, so every later id is Docutils'
     - Guidedoc reports problems as diagnostics, not as page content, as Sphinx does (``keep_warnings = False`` removes them); the problematic text keeps its id. the HTML renderer's id test expects exactly the other ids
   * - Smart quotes: Guidedoc's quotation marks come from its own table (CLDR delimiters)
     - they agree with Docutils for the corpus languages (en, de, fr, ru, ja, it, and de alternative quotes); other languages may differ where Docutils' tables differ from CLDR


Core
----

Section subtitles would be represented as Subtitle if ``guidedoc``'s ``child_allowed``
allowed Subtitle as a child of Section; ``crates/guidedoc/AST.md`` should then
list Subtitle under Section. Until then the reader uses a Paragraph of class
``section-subtitle``, which renders sensibly without renderer changes.

