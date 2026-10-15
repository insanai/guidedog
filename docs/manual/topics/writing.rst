Authoring documents
===================

Guidedog supports both reStructuredText (``.rst``) and MyST Markdown (``.md``).
Documents are parsed into a shared semantic document tree, which is then used to
generate both HTML websites and PDF books.

Choose a source language
------------------------

Use reStructuredText when you need full access to domains, complex tables, and
extensible directives. Use MyST Markdown when you prefer Markdown syntax while
retaining Sphinx-style directives and roles.

In a documentation project, Markdown documents are parsed using the MyST dialect.
Single-document conversions with ``guidedog convert`` read Markdown as plain
CommonMark 0.31.2: MyST's directives, roles, and settings belong to projects.

Headings and structure
----------------------

A document title identifies the page. Sections and subsections divide the text
logically. In reStructuredText, headings use underline adornments (and optional
matching overlines). The underline must be at least as long as the text.

Recommended adornment styles:

.. code-block:: rst

   Document Title
   ==============

   Section Heading
   ---------------

   Subsection
   ~~~~~~~~~~

   Sub-subsection
   ^^^^^^^^^^^^^^

   Paragraph Heading
   """""""""""""""""

Keep heading levels consistent across your project. In MyST Markdown, use standard
hashes (``#``, ``##``, ``###``).

Text formatting and inline markup
---------------------------------

Use inline markup to communicate meaning rather than decoration:

.. list-table:: Inline markup
   :header-rows: 1
   :widths: 30 35 35

   * - Style
     - reStructuredText
     - MyST Markdown
   * - **Strong / Bold**
     - ``**strong text**``
     - ``**strong text**``
   * - *Emphasis / Italic*
     - ``*emphasized text*``
     - ``*emphasized text*``
   * - ``Inline code``
     - ````inline code````
     - ```inline code```
   * - Subscript
     - ``:sub:`text```
     - ``{sub}`text```
   * - Superscript
     - ``:sup:`text```
     - ``{sup}`text```

To include literal asterisks or backticks within text, escape them with a backslash:
``\*not italic\*``.

Lists
-----

Guidedog supports four kinds of lists:

Bullet lists
~~~~~~~~~~~~

.. code-block:: rst

   * First item
   * Second item with multiple lines
     of continuing explanation.
   * Third item

Enumerated lists
~~~~~~~~~~~~~~~~

.. code-block:: rst

   1. Numbered item
   2. Second item
   #. Automatically numbered item
   #. Next auto-numbered item

Definition lists
~~~~~~~~~~~~~~~~

A definition list pairs terms with explanatory blocks. The term appears on a single
line, followed immediately by an indented definition:

.. code-block:: rst

   Workspace
      The reusable conversion context. It keeps its buffers between documents.

   Project
      A loaded configuration with its source folder, templates, and caches.

Field lists
~~~~~~~~~~~

Field lists provide structured metadata and parameter descriptions:

.. code-block:: rst

   :Authors: Jane Doe, John Smith
   :Version: 1.2
   :Status: Active

Code blocks
-----------

Use the ``code-block`` directive (or ``sourcecode``) to display syntax-highlighted
code. Guidedog highlights 35 programming languages, Rust and Odin among them, with
Pygments' names and styles and no Python runtime:

.. code-block:: rst

   .. code-block:: python
      :linenos:
      :caption: Server entry point
      :emphasize-lines: 2, 4-5

      def main():
          app = create_app()
          app.run(host="0.0.0.0", port=8080)

Options for code blocks:

* ``:linenos:``: Displays line numbers beside the code.
* ``:caption: Title``: Adds a caption above or below the code block.
* ``:emphasize-lines: 1, 3-5``: Highlights specific lines.
* ``:name: label``: Assigns a reference target to the code block.

To include external source code directly from a file, use ``literalinclude``:

.. code-block:: rst

   .. literalinclude:: ../src/server.py
      :language: python
      :lines: 1-25
      :linenos:

Admonitions
-----------

Admonitions highlight notes, warnings, and supplementary advice:

.. code-block:: rst

   .. note::
      Helpful background context or implementation detail.

   .. tip::
      Suggested best practices or workflow shortcuts.

   .. important::
      Essential requirements that must not be overlooked.

   .. warning::
      Conditions that could lead to unexpected behavior or lost work.

   .. caution::
      Potential pitfalls or sensitive operational steps.

   .. seealso::
      References to related chapters, specifications, or external guides.

Guidedog also supports ``danger``, ``error``, ``hint``, and ``attention``.
A generic admonition with a custom title uses the ``admonition`` directive:

.. code-block:: rst

   .. admonition:: Design Rationale

      Explains why a particular architecture was chosen.

Tables
------

Guidedog supports simple tables, grid tables, and directive-based tables.

Simple tables
~~~~~~~~~~~~~

Simple tables use horizontal dashes to define column spans:

.. code-block:: rst

   =====  =====  =======
   A      B      A and B
   =====  =====  =======
   False  False  False
   True   False  False
   True   True   True
   =====  =====  =======

Grid tables
~~~~~~~~~~~

Grid tables allow complex multi-line cells and arbitrary column/row spans:

.. code-block:: rst

   +------------------------+------------+----------+
   | Header row, column 1   | Column 2   | Column 3 |
   +========================+============+==========+
   | Cell with multiple     | Second     | Third    |
   | paragraphs of text.    | column     | column   |
   +------------------------+------------+----------+

List tables
~~~~~~~~~~~

The ``list-table`` directive creates tables from nested bullet lists, making wide
tables easy to read and maintain in source control:

.. code-block:: rst

   .. list-table:: Project configurations
      :widths: 25 25 50
      :header-rows: 1

      * - Target
        - Builder
        - Description
      * - Website
        - ``html``
        - Static HTML documentation site
      * - Book
        - ``pdf``
        - Typeset PDF book powered by Typst

CSV tables
~~~~~~~~~~

The ``csv-table`` directive constructs a table from comma-separated values:

.. code-block:: rst

   .. csv-table:: Comparative metrics
      :header: "Name", "Time (ms)", "Memory (MB)"
      :widths: 40, 30, 30

      "Cold build", 42, 12
      "Incremental", 3, 4

Images and figures
------------------

Include illustrations, screenshots, and diagrams with ``image`` and ``figure``:

.. code-block:: rst

   .. image:: /assets/architecture.png
      :width: 600px
      :align: center
      :alt: System architecture diagram

   .. figure:: /assets/flow.png
      :scale: 80%
      :align: center
      :alt: Execution flowchart

      Data flows sequentially through reader, resolver, and renderer.

A ``figure`` wraps the image with an explanatory caption and an optional reference target.

Document structure with toctrees
--------------------------------

The ``toctree`` directive creates the document hierarchy, navigation menus, and reading
order for both the HTML site and the PDF book:

.. code-block:: rst

   .. toctree::
      :maxdepth: 2
      :caption: User Guide
      :numbered:

      installation
      quickstart
      configuration

Common options for ``toctree``:

* ``:maxdepth: N``: Depth of heading levels to include in the table of contents.
* ``:caption: Title``: Category title displayed above the navigation entries.
* ``:numbered:``: Adds section numbers to chapters and headings.
* ``:titlesonly:``: Lists only the top-level document titles, ignoring inner sections.
* ``:hidden:``: Records the documents in the reading order without rendering an inline list.
* ``:glob:``: Allows wildcard patterns to match documents (e.g. ``tutorials/*``).

Cross-references and links
--------------------------

Explicit targets ensure links remain stable even when document files are renamed:

Target labels and ``:ref:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Place a target label immediately before a heading, table, or figure:

.. code-block:: rst

   .. _storage-model:

   Storage model
   -------------

   Every conversion runs within explicit limits.

Refer to this target from any document across the project:

.. code-block:: rst

   See :ref:`storage-model` for details.
   See :ref:`custom link text <storage-model>`.

Document references with ``:doc:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Link to another document by path (omitting the file extension):

.. code-block:: rst

   Read the :doc:`configuration guide <../reference/configuration>` for details.

Glossary terms with ``:term:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Define terms using the ``glossary`` directive:

.. code-block:: rst

   .. glossary::

      Workspace
         A reusable context whose buffers keep their capacity between conversions.

      Pass
         An in-memory transformation step operating on the document AST.

Link to a glossary term using the ``:term:`` role:

.. code-block:: rst

   Conversion runs within a reusable :term:`workspace`.

File downloads with ``:download:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Copy a file to the output's download directory and create a link:

.. code-block:: rst

   Download the :download:`starter template <files/starter.toml>`.

External links
~~~~~~~~~~~~~~

Link to external web addresses:

.. code-block:: rst

   Visit `Typst <https://typst.app>`_ for typography details.

Substitutions and includes
--------------------------

Define reusable phrases, symbols, or images:

.. code-block:: rst

   .. |version| replace:: 1.0.0
   .. |brand| replace:: **Field Notes**

   Welcome to |brand| version |version|.

To reuse common reStructuredText content across multiple files:

.. code-block:: rst

   .. include:: ../shared/warnings.rst

MyST Markdown authoring
-----------------------

MyST Markdown supports the same semantic directives and roles through fenced blocks:

.. code-block:: markdown

   # Project overview

   Here is a paragraph with **bold text**, *italic emphasis*, and `inline code`.

   ```{note}
   This note is rendered identically to a reStructuredText note directive.
   ```

   ```{code-block} python
   :caption: Example function
   :linenos:

   def add(a, b):
       return a + b
   ```

   ```{toctree}
   :maxdepth: 2
   :caption: Navigation

   first-chapter
   second-chapter
   ```

   See {ref}`storage-model` or consult the {doc}`../reference/configuration`.
