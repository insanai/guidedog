Diagrams that explain
=====================

Choose a diagram for a question. A flow chart explains a choice.
A block diagram explains responsibility. A sequence explains ordering.
A graph explains relationships. Use prose when it answers the question more directly.

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

Light, dark and print
---------------------

A transparent canvas does not change a graph's ink. Dark text can disappear on
a dark page. Ordinary graphs therefore keep their chosen colors and receive a
light panel in the default dark theme.

For the manual's adaptive palette, add ``:class: graphviz-adaptive`` to the
directive. This is available in Guidedog 0.6.3 and later. Guidedog adds a fixed
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
