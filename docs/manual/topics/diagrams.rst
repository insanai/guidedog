Diagrams with Graphviz and Typst
================================

Choose a diagram for a question. A flow chart explains a choice.
A block diagram explains responsibility. A sequence explains ordering.
A graph explains relationships. Use prose when it answers the question more directly.

We keep diagram source beside the explanation and let Guidedog build the image for HTML and PDF.
Graphviz and Typst are included in the executable.
We need no separate ``dot`` or ``typst`` program.

For musical scores, we use the native notation workflow in :doc:`music`.

Choose the drawing tool
-----------------------

.. list-table:: Drawing tools
   :header-rows: 1
   :widths: 25 45 30

   * - Tool
     - Useful when
     - Source
   * - Graphviz
     - We describe nodes and edges and want the layout engine to place them.
     - DOT inside ``graphviz``, ``graph`` or ``digraph``.
   * - Typst with CeTZ
     - We need explicit geometry, custom shapes, plots or sequence layouts.
     - Typst inside ``typst-diagram``.
   * - Typst with Fletcher
     - We need positioned nodes, mathematical labels and precise arrows.
     - Typst inside ``typst-diagram`` with a pinned Fletcher import.

We choose SVG for scalable drawings in both the browser and book.
Typst diagrams can also produce PNG at 144 pixels per inch.
Existing SVG and PNG files still work with ordinary image and figure directives.

A flow chart
------------

.. graphviz::
   :class: graphviz-adaptive
   :caption: A conversion checks its limits before a buffer grows.
   :alt: If the request fits the limits, the workspace buffer grows and conversion continues; otherwise the conversion ends with a Limit result.

   digraph capacity {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     need [class="gd-input", label="Input needs more room", fillcolor="#fbfaf3"];
     fits [class="gd-choice", label="Within the limits?", shape=diamond, fillcolor="#fff4e6", color="#ca8a04", fontcolor="#854d0e"];
     grow [label=<<B>Workspace buffer grows</B><BR/><FONT POINT-SIZE="11">capacity is kept for the next document</FONT>>, fillcolor="#eef6f2"];
     stop [label=<<B>Limit result</B><BR/><FONT POINT-SIZE="11">reported with a hint; no panic</FONT>>, fillcolor="#edf5f2"];

     need -> fits;
     fits -> grow [label="yes"];
     fits -> stop [label="no"];
     grow -> need [label="continue", style=dashed];
   }

The drawing names the decision at the growth step.
It does not suggest that running out of room aborts the program:
a bound produces a result the caller can report.
This distinction is part of the contract.

Author a graph
--------------

.. code-block:: rst

   .. graphviz::
      :class: graphviz-adaptive
      :caption: Parsing precedes resolution.
      :alt: The source becomes a tree, then references are resolved.

      digraph pipeline {
        graph [rankdir=LR, bgcolor="transparent", pad="0.3"];
        node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
              color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12];
        edge [color="#216553", fontname="Helvetica", fontsize=10];
        "source" -> "tree" -> "resolved tree";
      }

``graphviz``, ``graph``, and ``digraph`` are built in.
Guidedog renders them through Graphviz. HTML receives an SVG asset.
PDF embeds the rendered drawing. Both use the same graph source.

In MyST Markdown, we use a directive fence with the same options.

.. code-block:: markdown

   ```{graphviz}
   :caption: Parsing precedes resolution
   :alt: Source becomes a tree, then references are resolved

   digraph pipeline { source -> tree -> resolved; }
   ```

A ``graphviz`` file argument reads a DOT file instead of an inline body.
``graph`` and ``digraph`` take the graph name as their argument and wrap the body's edges in the corresponding DOT graph.
``:layout:`` (or Sphinx's ``:graphviz_dot:`` alias) selects the Graphviz layout engine: ``dot`` (default), ``neato``, ``fdp``, ``sfdp``, ``circo``, ``twopi``, ``osage`` or ``patchwork``.
A plain ``dot`` or ``graphviz`` Markdown fence also draws a graph; we use the directive form when we need options.

Draw with Typst
---------------

The ``typst-diagram`` directive compiles its body to an image.
The page fits the drawing automatically.
We can start with Typst's built-in shapes before adding a package.

.. code-block:: rst

   .. typst-diagram::
      :caption: A drawing from Typst source
      :alt: A teal circle
      :name: typst-circle

      #circle(radius: 12pt, fill: rgb("#216553"), stroke: none)

.. typst-diagram::
   :caption: A drawing from Typst source.
   :alt: A teal circle
   :name: chapter-typst-circle

   #circle(radius: 12pt, fill: rgb("#216553"), stroke: none)

The same drawing in MyST uses ``{typst-diagram}`` as the fence language.
An ordinary ``typst`` code fence displays source without compiling it.

.. code-block:: markdown

   ```{typst-diagram}
   :caption: A drawing from Typst source
   :alt: A teal circle
   :format: png

   #circle(radius: 12pt, fill: rgb("#216553"), stroke: none)
   ```

CeTZ supplies a drawing canvas and geometric operations.
We import a specific package version so the source states its dependency.

.. code-block:: rst

   .. typst-diagram::
      :caption: Two connected nodes
      :alt: Two circles joined by a line

      #import "@preview/cetz:0.3.4"
      #cetz.canvas({
        import cetz.draw: *
        circle((0, 0), radius: 0.5)
        circle((2, 0), radius: 0.5)
        line((0.5, 0), (1.5, 0))
      })

Fletcher builds node-and-arrow diagrams on CeTZ.
Its explicit coordinates are useful when position carries meaning.

.. code-block:: rst

   .. typst-diagram::
      :caption: Source becomes an image
      :alt: Source points to an image

      #import "@preview/fletcher:0.5.8": diagram, node, edge
      #diagram(
        node((0, 0), [Source]),
        node((1, 0), [Image]),
        edge((0, 0), (1, 0), "->"),
      )

Share source and refer to figures
---------------------------------

Both directives accept ``:caption:``, ``:alt:``, ``:name:``, ``:class:`` and ``:align:``.
We enable ``numfig = true`` in ``conf.toml`` and refer to a named captioned drawing with ``:numref:`` in RST or ``{numref}`` in MyST.
For example, :numref:`chapter-typst-circle` is a numbered figure in this book.
Alternative text explains the information conveyed by the drawing.
It is especially useful for Typst SVGs, which draw letters as outlines.

For a larger drawing we keep Typst source in a file.
Guidedog compiles it during the ordinary build.

.. code-block:: rst

   .. typst-diagram:: figures/architecture.typ
      :caption: Project architecture
      :alt: Components and their connections

An inline diagram's imports resolve beside the document containing it.
A file diagram's imports resolve beside its Typst file.
We can therefore share a local helper library for colors, nodes and sequence diagrams.
Changing an imported helper or data file redraws the affected HTML pages.
Identical generated images keep their timestamps so unchanged PDF books remain cached.

Packages, limits and failures
-----------------------------

The ``pdf_packages``, ``pdf_fonts`` and ``pdf_font_paths`` settings govern Typst diagrams in both outputs.
With ``pdf_packages = "offline"``, the compiler uses only packages already installed in Typst's package directories.
The default downloads missing packages.
After installing a previously missing package or editing fonts, we run a fresh ``guidedog build html -a`` command.
Project imports stay below the source directory, including through symlinks.
Diagrams must produce one finite page; PNG export is limited to 16 million pixels and 16,384 pixels on either side.

A failed drawing remains visible as source with a warning (``build.graphviz`` or ``build.typst-diagram``; missing or duplicate directive sources report ``sphinx.graphviz.source`` or ``sphinx.typst.source``).
We build with ``-W`` when a missing diagram must prevent publication.
``--untrusted`` refuses Typst compilation and external Graphviz resources and retains the source.
Text builds show source without compiling a drawing.
The :doc:`../howto/typst-diagrams` guide lists every option, source limit and migration step for existing figure files.

Light, dark and print
---------------------

Typst diagrams keep their authored colors and receive the same light panel as ordinary Graphviz images in the default dark theme. The adaptive Graphviz palette does not recolor Typst drawings.

A transparent canvas does not change a graph's ink. Dark text can disappear on
a dark page. Ordinary graphs therefore keep their chosen colors and receive a
light panel in the default dark theme.

For the manual's adaptive palette, add ``:class: graphviz-adaptive`` to the
directive. Guidedog adds a fixed
stylesheet inside the SVG image. Its dark media query follows the embedding
page's ``color-scheme``, including the theme control's explicit selection.
No script, external stylesheet or inline page SVG is needed.

Use short labels, rounded boxes for steps, notes for input and cylinders for
durable stores. Node classes distinguish roles in the dark palette:

.. list-table:: Semantic node classes
   :header-rows: 1
   :widths: 30 70

   * - DOT class
     - Role
   * - ``gd-input``
     - Source, configuration or supporting material; warm neutral.
   * - ``gd-output``
     - A produced artifact; blue.
   * - ``gd-plugin``
     - An optional extension point; amber, with a dashed border.
   * - ``gd-choice``
     - A decision; amber, with a diamond shape.
   * - ``gd-memory``
     - A separate ownership or allocation boundary; violet.

For example, ``source [class="gd-input", shape=note, fillcolor="#fbfaf3"]``
sets both the light presentation and its dark role. Classes do not choose shapes
or add labels. We write those explicitly, so the drawing also works in monochrome.
Unclassified steps use green. This palette is opt-in. We do not apply it to artwork
whose exact colors carry data.

The light DOT attributes remain the PDF master. The book receives an SVG without
the adaptive stylesheet. Printed HTML selects the light scheme too. Keep both
palettes legible and test the actual book. Transparency alone proves neither.
The inheritance rule is defined by the
`CSS color-scheme specification <https://www.w3.org/TR/css-color-adjust-1/#color-scheme-prop>`_.

Make the result legible
-----------------------

Keep node labels short. Explain a longer rule below the drawing.
Prefer one direction for the main path. Label a return edge with its condition.
Use shape and text as well as color. A print reader may have no color at all.

Supply alternative text and a caption. Test narrow browser windows and the PDF page.
If a diagram needs tiny type to fit, split it into two drawings.
See :doc:`../media` for image and figure sizing.

On narrow screens, captioned diagrams scroll horizontally to keep labels readable.
Scroll inside the figure. The page itself keeps its width.
