Make a small book
=================

A website is an interconnected web of pages that readers explore non-linearly. A book,
by contrast, requires a deliberate, sequential reading order with numbered chapters,
running headers, and formal front matter.

In Guidedog, the table of contents directive (``toctree``) provides the bridge between
both formats. A single ``toctree`` organizes your documents into a hierarchical sidebar
tree for the web and a linear chapter progression for the PDF book.

.. graphviz::
   :class: graphviz-adaptive
   :caption: How a single toctree shapes both web navigation and sequential book chapters.
   :alt: The root document toctree maps into an interactive sidebar and breadcrumbs on the web, and numbered chapters in print.

   digraph toctree_dual_role {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];
     root [class="gd-input", shape=note, fillcolor="#fbfaf3",
           label=<<B>index.rst</B><BR/><FONT POINT-SIZE="11">toctree: measurement</FONT>>];
     chapter [class="gd-input", shape=note, fillcolor="#fbfaf3",
              label=<<B>measurement.rst</B><BR/><FONT POINT-SIZE="11">One source chapter</FONT>>];
     web [class="gd-output", fillcolor="#eef4fa",
          label=<<B>Web navigation</B><BR/><FONT POINT-SIZE="11">Sidebar tree<BR/>Linked chapter page</FONT>>];
     book [class="gd-output", fillcolor="#eef4fa",
           label=<<B>Book reading order</B><BR/><FONT POINT-SIZE="11">Contents<BR/>Numbered chapter</FONT>>];
     root -> chapter;
     chapter -> web;
     chapter -> book;
   }

1. Add a new chapter
--------------------

In your ``notes`` project, create a new document named ``measurement.rst``:

.. code-block:: rst
   :caption: notes/measurement.rst

   Measurement
   ===========

   A scientific measurement consists of a numerical magnitude and an explicit unit.
   Always report both. A bare number leaves the reader guessing.

   .. _measurement-units:

   Units of measurement
   --------------------

   Maintain a single, consistent unit system within every comparison.

   When working with the International System of Units (SI):

   * Length is expressed in meters (m).
   * Mass is expressed in kilograms (kg).
   * Time is expressed in seconds (s).

   Prefixes scale units by powers of ten, simplifying very large or small values.

Notice the section label ``.. _measurement-units:`` placed directly above the heading.
Labels establish cross-reference targets that work identically on the web and in print.

2. Structure the reading order
------------------------------

Open ``notes/index.rst`` and declare the table of contents:

.. code-block:: rst
   :caption: notes/index.rst

   Field Notes
   ===========

   These notes explain how to perform reliable physical measurements and document
   them clearly.

   .. toctree::
      :maxdepth: 2
      :numbered:

      measurement

   Next steps:

   * Begin with :doc:`measurement`.
   * Review the standard unit conventions in :ref:`measurement-units`.

Understanding the toctree options:

* ``:maxdepth: 2`` sets the depth of nested subheadings displayed in the table of contents.
* ``:numbered:`` activates automatic, hierarchical chapter numbering (for example, "1. Measurement" and "1.1 Units of measurement").
* ``measurement`` references the document file name without the ``.rst`` extension.

Cross-referencing syntax:

* ``:doc:`measurement``` links to the chapter by document path.
* ``:ref:`measurement-units``` links to the labeled section. PDF uses the section
  title. A book template can also add the page number through ``page-ref``; see
  :doc:`a-custom-theme` for a working example.

3. Add a diagram with Graphviz
------------------------------

Visual diagrams clarify complex relationships. Guidedog embeds the Graphviz engine
natively, compiling vector diagrams directly into SVG for web pages and PDF drawings for
print without requiring an external browser renderer.

Add a diagram to ``notes/measurement.rst``:

.. code-block:: rst

   Comparison workflow
   -------------------

   Before comparing two experimental values, convert both to a shared base unit:

   .. graphviz::
      :class: graphviz-adaptive
      :caption: Converting values to a common base unit before comparison.
      :alt: Raw experimental values are converted to a shared unit before comparison.

      digraph comparison {
        graph [rankdir=LR, bgcolor="transparent", pad="0.2"];
        node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
              color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12];
        edge [color="#216553"];

        "Raw values" -> "Unit conversion" -> "Common unit" -> "Comparison";
      }

The ``:caption:`` option supplies a caption. To number figures, add
``numfig = true`` to ``notes/conf.toml``. The ``:alt:`` option provides alternative
text for screen readers.

4. Build the book and inspect results
-------------------------------------

Rebuild your project:

.. code-block:: sh

   guidedog build html notes -W
   guidedog build pdf notes -W

Inspect the results in both formats:

* **HTML website:** Open ``notes/_build/html/index.html``. The sidebar now shows the
  numbered chapter hierarchy. Clicking "1. Measurement" opens the new page, complete
  with local breadcrumbs and the rendered SVG diagram.
* **PDF book:** Open ``notes/_build/pdf/field-notes-en.pdf``. The contents lists the
  chapters and their pages. The diagram remains vector artwork, and document
  cross-references link to their targets.

5. Next steps
-------------

Now that you understand the dual output model, explore the topics section to learn more:

* Learn rich markup, lists, and tables in :doc:`../topics/writing`.
* Learn how to render formulas and equations in :doc:`../topics/mathematics`.
* Learn how to configure custom styles and book fonts in :doc:`../topics/typography`.
