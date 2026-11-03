Draw diagrams with Typst
========================

We can put Typst drawing code directly in an RST or MyST document.
``typst-diagram`` compiles the code with Guidedog's embedded Typst compiler and includes the resulting image in HTML and PDF.
We need no separate figure task or installed Typst program.

A first drawing
---------------

We write the source as the directive's body.

.. code-block:: rst

   .. typst-diagram::
      :caption: A circle
      :alt: A teal circle
      :name: example-circle

      #circle(radius: 12pt, fill: rgb("#216553"), stroke: none)

.. typst-diagram::
   :caption: A circle compiled from the directive body.
   :alt: A teal circle

   #circle(radius: 12pt, fill: rgb("#216553"), stroke: none)

We use a MyST directive fence in Markdown.
An ordinary ``typst`` code fence remains a source example.

.. code-block:: markdown

   ```{typst-diagram}
   :caption: A circle
   :alt: A teal circle

   #circle(radius: 12pt, fill: rgb("#216553"), stroke: none)
   ```

The image defaults to SVG, so the same vector drawing scales in the website and book.
``:format: png`` selects a PNG at 144 pixels per inch.
We can also set ``:class:`` and ``:align: left|center|right``.
With ``numfig = true``, a captioned diagram is a numbered figure and ``:numref:`` can refer to its ``:name:``.
We provide meaningful ``:alt:`` text because SVG text is drawn as outlines.

.. list-table:: ``typst-diagram`` options
   :header-rows: 1
   :widths: 24 76

   * - Option
     - Meaning
   * - ``:alt: text``
     - Alternative text describing the diagram's content.
   * - ``:caption: text``
     - Wraps the drawing in a figure with this caption (numbered when ``numfig = true``).
   * - ``:name: label``
     - Cross-reference anchor for ``:ref:`` and ``:numref:``.
   * - ``:format: svg|png``
     - Output image format: ``svg`` (default) or ``png`` (144 PPI).
   * - ``:align: left|center|right``
     - Horizontal placement of the image or figure.
   * - ``:class: names``
     - Extra CSS class names added alongside ``typst-diagram``.

CeTZ and Fletcher
-----------------

We import packages explicitly with pinned versions.
This CeTZ example uses the version shared by Fletcher 0.5.8.

.. code-block:: rst

   .. typst-diagram::
      :alt: Two joined circles

      #import "@preview/cetz:0.3.4"
      #cetz.canvas({
        import cetz.draw: *
        circle((0, 0), radius: 0.5)
        circle((2, 0), radius: 0.5)
        line((0.5, 0), (1.5, 0))
      })

Fletcher supplies nodes and arrows.

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

The ``pdf_packages``, ``pdf_fonts`` and ``pdf_font_paths`` settings apply to diagrams in both HTML and PDF.
``pdf_packages = "offline"`` uses only packages already installed in Typst's package directories.
The default permits downloads of missing packages.
For repeatable typography we set ``pdf_fonts = "embedded"`` and add any project font directories with ``pdf_font_paths``.
After installing a previously missing package or changing font files, we run a fresh ``guidedog build html -a`` command to redraw cached pages and reload the compiler's font store.

Shared helpers and existing files
---------------------------------

We can move common colors and drawing functions into a local ``.typ`` file.
An inline diagram imports paths relative to the RST or Markdown file containing it.
A directive inside an included document uses that document's folder.

.. code-block:: rst

   .. typst-diagram::
      :alt: Project architecture

      #import "figures/helpers.typ": architecture
      #architecture()

We can also retain a complete figure source file and let Guidedog build it.
We give either a file argument or an inline body.
Imports in the file resolve beside that file.

.. code-block:: rst

   .. typst-diagram:: figures/architecture.typ
      :caption: Project architecture
      :alt: The project's components and connections

Imported helpers and data files become build dependencies.
Changing one redraws the affected HTML pages without changing their markup.
We keep project imports below the source directory; symlinks leading outside it are refused.
Absolute Typst imports start at that source directory.

Page size and failures
----------------------

Guidedog prepends ``#set page(width: auto, height: auto, margin: 0pt)`` so the page fits the drawing.
We write ordinary Typst content, without an ``html.frame`` wrapper.
A diagram must produce exactly one finite page.
The directive body is limited to 1 MiB; PNG export permits at most 16 million pixels and 16,384 pixels on either side.
The adapter tracks at most 4,096 distinct file lookups per diagram.
Typst layout uses the compiler's own memory; these limits do not impose a compiler timeout or a total memory ceiling.

If compilation fails, we see a ``build.typst-diagram`` warning at the directive and the source stays visible as code.
The message includes the position within the diagram or imported Typst file.
Giving neither or both of a file argument and an inline body reports ``sphinx.typst.source``.
A ``guidedog`` built without the ``embedded`` Typst feature reports ``typst.embedded``, and exceeding the host memory budget reports ``host.budget``.
``-W`` prevents a failed build from replacing the published site or book.
``--untrusted`` draws diagrams too: the embedded compiler reads only the source folder there and downloads no package, so a diagram that imports an uncached package fails with a warning.
Text builds retain source without compiling it.

Migrate existing figure files
-----------------------------

When a project previously ran ``typst compile`` by hand or in a Makefile to generate prebuilt ``.svg`` or ``.png`` files:

1. Remove any explicit ``#set page(...)`` or ``html.frame(...)`` wrapper that existed only to crop the figure; Guidedog prepends ``#set page(width: auto, height: auto, margin: 0pt)`` automatically.
2. Replace ``.. figure:: figures/arch.svg`` (and its separate build script) with ``.. typst-diagram:: figures/arch.typ``, keeping ``:caption:``, ``:alt:``, ``:name:`` and ``:align:`` on the directive.
3. Delete the committed generated ``.svg`` or ``.png`` artifacts once ``guidedog build html -W`` and ``guidedog build pdf -W`` succeed.
