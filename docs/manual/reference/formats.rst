Readers, renderers, and builders
================================

A reader understands a source language. A renderer writes one document tree.
A builder coordinates a project and publishes its output.
These are different responsibilities.

.. list-table:: Source routes
   :header-rows: 1
   :widths: 25 35 40

   * - Input
     - Reader
     - Route
   * - RST in a project
     - RST plus the built-in Sphinx layer
     - Project graph to HTML, Typst/PDF, text, or catalogs.
   * - Markdown in a project
     - MyST-compatible Markdown layer
     - The same project graph and builders.
   * - ``convert file.md``
     - Plain CommonMark
     - One document to HTML or Typst/PDF.
   * - ``convert file.typ``
     - Native Typst adapter
     - Typst compilation outside Guidedoc's semantic tree.

Each reader and renderer is its own crate, and ``guidedoc-defaults`` registers them.
Three crates read: ``guidedoc-rst`` (reStructuredText), ``guidedoc-commonmark`` (CommonMark 0.31.2 and
MyST), and ``guidedoc-sphinx`` (the Sphinx directives, roles and domains on both).
Three crates render: ``guidedoc-html``, ``guidedoc-typst``, and ``guidedoc-text``. A program can
register only the ones it needs; see :doc:`../howto/install`.

Builders
--------

``html``
   One HTML file for each document, with indexes, search, and an inventory.

``dirhtml``
   Directory-style document URLs.

``singlehtml``
   One main HTML page with document sections and local references.

``pdf``
   Typst books defined by the project. LaTeX builder aliases select this route.

``text``
   Plain text files for documents.

``gettext``
   Message templates for translation.

``dummy``
   Reads and resolves the project without publishing an artifact.

``coverage``
   Writes ``python.txt`` and ``c.txt``: the Python and C objects the documents do not describe.

Coverage is evidence
--------------------

``guidedog formats`` reports the executable's actual adapters.
The compatibility page records pinned suites and real-project checks.
Passing a CommonMark suite does not establish complete MyST coverage.
Reading RST does not establish every Python extension's behavior.
See :doc:`../compatibility`.
