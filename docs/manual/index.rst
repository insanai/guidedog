Guidedog Manual
===============

**Write once. Explain well. Publish on the web and on paper.**

Guidedog builds a documentation website and a PDF book from the same reStructuredText or Markdown files. We keep one set of source files and publish it for readers on the web and on paper. The release provides one ``guidedog`` executable, with PDF typesetting and diagram rendering included.

A small project needs three things: a source document, a table of contents,
and ``conf.toml``. Start with :doc:`tutorial/first-project`.
For a particular task, use :doc:`howto/index`.
For an exact contract, use :doc:`reference/index`.

.. graphviz::
   :class: graphviz-adaptive
   :caption: One source project, two publications.
   :alt: RST and Markdown enter Guidedoc; HTML goes to a browser and Typst produces PDF.

   digraph publication {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];
     source [class="gd-input", shape=note, fillcolor="#fbfaf3",
             label=<<B>One source project</B><BR/><FONT POINT-SIZE="11">RST / Markdown + assets</FONT>>];
     engine [label=<<B>Guidedog + Guidedoc</B><BR/><FONT POINT-SIZE="11">Read, resolve, render</FONT>>];
     html [class="gd-output", fillcolor="#eef4fa",
           label=<<B>HTML website</B><BR/><FONT POINT-SIZE="11">Navigation and search</FONT>>];
     pdf [class="gd-output", fillcolor="#eef4fa",
          label=<<B>PDF book</B><BR/><FONT POINT-SIZE="11">Typeset by Typst</FONT>>];
     source -> engine;
     engine -> html;
     engine -> pdf;
   }

.. toctree::
   :maxdepth: 2
   :caption: Learn

   tutorial/index
   topics/index

.. toctree::
   :maxdepth: 2
   :caption: Work

   howto/index
   reference/index

.. toctree::
   :maxdepth: 2
   :caption: Document code

   autodoc
   sources-and-diagrams
   rust
   odin
   object-types

.. toctree::
   :maxdepth: 2
   :caption: Understand

   internals/index
   plugins
   about
   repository
   evidence/index

.. toctree::
   :maxdepth: 1
   :caption: API reference

   api/index
