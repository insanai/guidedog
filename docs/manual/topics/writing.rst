Authoring documents
===================

Guidedog supports both reStructuredText (``.rst``) and MyST Markdown (``.md``).
Both are parsed into the same document tree. The HTML website and the PDF book
are made from that tree.

Choose a source language
------------------------

We use reStructuredText when we need full access to domains, complex tables, and
extensible directives. We use MyST Markdown when we prefer Markdown syntax but
still want Sphinx-style directives and roles.

In a documentation project, Markdown documents are parsed as MyST.
Single-document conversions with ``guidedog convert`` read Markdown as plain
CommonMark 0.31.2: MyST's directives, roles, and settings belong to projects.

Headings and structure
----------------------

A document title identifies the page. Sections and subsections divide the text.
In reStructuredText, headings use underline adornments (and optional
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

We keep heading levels consistent across the project. In MyST Markdown, we use standard
hashes (``#``, ``##``, ``###``).

Text formatting and inline markup
---------------------------------

Inline markup carries meaning, not decoration:

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

To write literal asterisks or backticks in text, we escape them with a backslash:
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

A definition list pairs terms with explanations. The term stands on one
line. An indented definition follows it directly:

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

The ``code-block`` directive (or ``sourcecode``) shows syntax-highlighted
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

To include source code from a file, we use ``literalinclude``:

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

The ``list-table`` directive creates tables from nested bullet lists. Wide
tables stay easy to read and maintain in source control:

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

We include illustrations, screenshots, and diagrams with ``image`` and ``figure``:

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

A ``figure`` wraps the image with a caption and an optional reference target.

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

Explicit targets keep links stable, even when document files are renamed.

Target labels and ``:ref:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~

We place a target label directly before a heading, table, or figure:

.. code-block:: rst

   .. _storage-model:

   Storage model
   -------------

   Every conversion runs within explicit limits.

Any document in the project can refer to this target:

.. code-block:: rst

   See :ref:`storage-model` for details.
   See :ref:`custom link text <storage-model>`.

Document references with ``:doc:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

We link to another document by its path, without the file extension:

.. code-block:: rst

   Read the :doc:`configuration guide <../reference/configuration>` for details.

Glossary terms with ``:term:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

We define terms with the ``glossary`` directive:

.. code-block:: rst

   .. glossary::

      Workspace
         A reusable context whose buffers keep their capacity between conversions.

      Pass
         An in-memory transformation step operating on the document AST.

We link to a glossary term with the ``:term:`` role:

.. code-block:: rst

   Conversion runs within a reusable :term:`workspace`.

File downloads with ``:download:``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

This copies a file to the output's download directory and links to it:

.. code-block:: rst

   Download the :download:`starter template <files/starter.toml>`.

External links
~~~~~~~~~~~~~~

We link to external web addresses like this:

.. code-block:: rst

   Visit `Typst <https://typst.app>`_ for typography details.

Substitutions and includes
--------------------------

We define reusable phrases, symbols, or images:

.. code-block:: rst

   .. |version| replace:: 1.0.0
   .. |brand| replace:: **Field Notes**

   Welcome to |brand| version |version|.

To reuse reStructuredText content in several files:

.. code-block:: rst

   .. include:: ../shared/warnings.rst

MyST Markdown authoring
-----------------------

MyST Markdown supports the same directives and roles through fenced blocks:

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
