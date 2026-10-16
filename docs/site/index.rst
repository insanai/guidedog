.. meta::
   :description: Guidedog builds documentation websites and PDF books from
      reStructuredText and Markdown.

Guidedog
========

.. rst-class:: gd-tagline

Write once. Explain well. Publish on the web and on paper.

.. rst-class:: gd-lead

Guidedog builds a documentation website and a PDF book from the same
reStructuredText or Markdown files. We write the text once. Guidedog keeps the two
editions in step.

.. container:: gd-actions

   `Read the manual <manual_>`_
   `Download <download_>`_
   `Source code <source_>`_

.. container:: gd-flow

   .. raw:: html
      :file: flow.svg

   .. rst-class:: gd-flow-caption

   A plain text file goes in. Guidedog makes a web page and a book page from it.


What it does
------------

.. rst-class:: gd-features

* **A website.** Pages with navigation, search, highlighted code, and light and dark
  themes, from Jinja templates we can change as much as we like.
* **A PDF book.** Chapters, cross-references, figures, tables, footnotes, and an index,
  set by Typst with hyphenation for each language.
* **reStructuredText and Markdown.** Directives, roles, toctrees, and cross-references
  work as Sphinx users expect, and MyST Markdown can sit next to them.
* **Translations.** Gettext catalogs carry a project into other languages. Chinese,
  Japanese, Korean, and Hindi text is laid out correctly on screen and on paper.
* **Diagrams, math, and API pages.** Graphviz diagrams, LaTeX equations, and API
  reference for Rust, Python, and Odin, read from source code.
* **Clear errors.** A message says what is wrong, where, and how to fix it. A failed
  build leaves the last good site in place.


One program
-----------

The release provides one ``guidedog`` executable with PDF typesetting and diagram rendering included. We download the archive for our system, unpack it, and start a project. On Windows we keep the bundled runtime files beside the executable.

Configuration is a TOML file. Templates are plain files in our project, and we can edit them. The same command also publishes to GitHub Pages and keeps discussion records through its linked plugins.


Write it, see both
------------------

On the left is reStructuredText. On the right is what Guidedog made of it on this page.
The PDF builder sets the same lines as a book page.

.. container:: gd-example

   .. code-block:: rst

      .. note::

         Guidedog reads our text once.

      The *same* files become a **website**
      and a **book**:

      .. list-table::
         :header-rows: 1

         * - Builder
           - Result
         * - ``html``
           - A website
         * - ``pdf``
           - A PDF book

   .. container:: gd-rendered

      .. note::

         Guidedog reads our text once.

      The *same* files become a **website**
      and a **book**:

      .. list-table::
         :header-rows: 1

         * - Builder
           - Result
         * - ``html``
           - A website
         * - ``pdf``
           - A PDF book

.. rst-class:: gd-more

The `manual as a PDF book <manual-pdf_>`_ is an example of the result on paper. It is
built from the same files as the manual's website.


Get started
-----------

Download the archive for our system from the `latest release <download_>`_, put
``guidedog`` on the ``PATH``, and create a project:

.. code-block:: sh

   guidedog quickstart docs
   guidedog build html docs
   guidedog build pdf docs
   guidedog serve docs

``quickstart`` writes ``conf.toml``, ``index.rst``, and templates we can edit.
``serve`` rebuilds the site while we write. The `tutorial <manual-tutorial_>`_ walks through
a first project. To build Guidedog from source, install Rust and follow the
`instructions in the repository <build_>`_.


Design discussions
------------------

Guidedog's design decisions are written down as numbered discussions: the problem, the
alternatives, the decision, and the evidence. We can read each one on the web or
download it as a PDF.

Our own project can keep its discussions the same way, with ``guidedog dsc``. The `manual <manual-discussions_>`_ shows how.

.. container:: gd-actions gd-actions-quiet

   `Read the discussions <discussions_>`_

.. _download: https://github.com/insanai/guidedog/releases
.. _source: https://github.com/insanai/guidedog
.. _build: https://github.com/insanai/guidedog#build-from-source


Recent releases
---------------

.. postlist:: 3
   :path: releases/*
   :excerpts:
