Custom directives and dynamic content
======================================

Guidedog publishes the same document tree to an interactive HTML website and a typeset PDF book.
Web readers benefit from interactive calculators, search filters, and embedded widgets. Book readers
benefit from balanced columns, callout cards, and vector typography.

This tutorial builds a project that demonstrates both without duplicating prose. We declare a custom
semantic directive in ``conf.toml``, add an interactive calculator for web readers, and design a
print-quality callout block for the PDF book.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Format-gated evaluation and custom directives across HTML and PDF pipelines.
   :alt: One source document evaluates object types for both outputs, sending raw HTML to the web and raw Typst to the PDF book.

   digraph dynamic_directives {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.32", ranksep="0.38"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];

     source [class="gd-input", shape=note, fillcolor="#fbfaf3",
             label=<<B>Source document</B><BR/><FONT POINT-SIZE="11">RST with directives &amp; format guards</FONT>>];

     conf [class="gd-input", shape=note, fillcolor="#fbfaf3",
           label=<<B>conf.toml</B><BR/><FONT POINT-SIZE="11">[[object_types]] declarations</FONT>>];

     parser [label=<<B>Guidedoc Parser</B><BR/><FONT POINT-SIZE="11">Resolve AST &amp; cross-references</FONT>>];

     obj_node [label=<<B>Custom object node</B><BR/><FONT POINT-SIZE="11">Index target &amp; signature</FONT>>];

     only_html [class="gd-plugin", style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e",
                label=<<B>.. only:: html</B><BR/><FONT POINT-SIZE="11">Active for web builder</FONT>>];

     only_typst [class="gd-plugin", style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e",
                 label=<<B>.. only:: typst or pdf</B><BR/><FONT POINT-SIZE="11">Active for book builder</FONT>>];

     html_out [class="gd-output", fillcolor="#eef4fa",
               label=<<B>HTML Website</B><BR/><FONT POINT-SIZE="11">Interactive widget &amp; DOM script</FONT>>];

     typst_out [class="gd-output", fillcolor="#eef4fa",
                label=<<B>Typst PDF Book</B><BR/><FONT POINT-SIZE="11">Typeset vector block &amp; index</FONT>>];

     source -> parser;
     conf -> parser;
     parser -> obj_node;
     parser -> only_html;
     parser -> only_typst;
     obj_node -> html_out [label="&lt;dl class=\"metric\"&gt;"];
     obj_node -> typst_out [label="Typst description"];
     only_html -> html_out [label="raw HTML markup"];
     only_typst -> typst_out [label="native Typst function"];
   }

1. Create the project
---------------------

Start in a clean directory and run:

.. code-block:: sh

   guidedog quickstart telemetry -q -p "Cluster Telemetry" -a "Operations Team" -v 1.0

Move into the new directory:

.. code-block:: sh

   cd telemetry

2. Declare custom domain directives in conf.toml
------------------------------------------------

A project often needs structured entities that standard domains (such as Python or C++) do not cover.
Rather than requiring a Python runtime or custom extension compiler, Guidedog lets us declare custom
directives and roles in ``conf.toml``.

Open ``conf.toml`` and append the declaration for a ``metric`` directive:

.. code-block:: toml

   [[object_types]]
   directive = "metric"
   role = "metric"
   index = "pair: %s; cluster telemetry"
   name = "first-word"
   display = "Metric: %s"

This single declaration establishes a complete syntax contract:
* ``.. metric:: p99_latency (milliseconds)`` describes the metric.
* ``:metric:`p99_latency``` links to the definition from any document.
* The index table automatically generates a pair index entry (``p99_latency; cluster telemetry``).
* In HTML, Guidedog generates a semantic definition list with CSS anchor hooks.
* In PDF, Guidedog typesets a styled description entry with back-references in the appendix.

3. Author the document with format guards
-----------------------------------------

Replace the contents of ``index.rst`` with the following document:

.. code-block:: rst

   Cluster Telemetry
   =================

   We monitor service level indicators across all ingress gateways.
   Refer to :metric:`p99_latency` for service level objectives.

   .. metric:: p99_latency (milliseconds)

      The 99th percentile response duration measured over a 60-second sliding window.

   Interactive estimator
   ---------------------

   .. only:: html

      .. raw:: html

         <div class="telemetry-widget" style="background: #f4f8f6; border: 1px solid #216553; border-radius: 6pt; padding: 14pt; margin: 16pt 0;">
           <label for="rps-slider" style="font-weight: bold; color: #16382f; display: block; margin-bottom: 6pt;">Simulate Request Rate (req/s):</label>
           <input type="range" id="rps-slider" min="500" max="10000" step="100" value="2500" style="width: 100%; max-width: 400pt;"
                  oninput="document.getElementById('est-p99').textContent = (0.35 + (this.value / 1000) * 0.42).toFixed(2) + ' ms'">
           <p style="margin-top: 10pt; color: #16382f;">Estimated P99 Latency: <strong id="est-p99" style="color: #216553; font-size: 1.15em;">1.40 ms</strong></p>
         </div>

   .. only:: pdf

      .. raw:: typst

         #rect(
           fill: rgb("#edf5f2"),
           stroke: 0.75pt + rgb("#216553"),
           radius: 4pt,
           inset: (x: 12pt, y: 10pt),
           width: 100%
         )[
           #text(weight: "bold", fill: rgb("#216553"))[Telemetry SLA Specification] \
           #v(3pt)
           P99 latency must remain strictly below 2.50 ms under sustained 5,000 req/s load.
           An interactive calculator is available in the web edition.
         ]

.. note::

   **Typst and LaTeX in Guidedog**: Guidedog compiles PDF books using native Typst.
   Always write ``.. raw:: typst`` for print callouts and vector blocks. Guidedog does not
   execute verbatim ``.. raw:: latex`` (which is omitted with a diagnostic). However, standard
   LaTeX math written in ``.. math::`` and ``:math:`` is automatically translated into Typst
   formulas by Guidedog's built-in math compiler.

4. Build and inspect both publications
--------------------------------------

Build the HTML website:

.. code-block:: sh

   guidedog build -b html . _build/html -W

Open ``_build/html/index.html`` in a browser or run ``guidedog serve``. Notice that:
* The ``:metric:`p99_latency``` reference resolves directly to the definition anchor.
* The interactive calculator slider is rendered and active in the web DOM.
* The PDF callout block is omitted entirely, without emitting warnings.

Now build the PDF book:

.. code-block:: sh

   guidedog build -b pdf . _build/pdf -W

Open ``_build/pdf/telemetry-en.pdf`` (or the inspection source at ``_build/pdf/book-en.typ``). Notice that:
* The slider and raw HTML are completely absent.
* The Typst ``#rect(...)`` vector box is typeset with native margins, borders, and typography.
* The appendix includes an index entry under ``p99_latency``.

5. Custom Typst functions in theme templates
--------------------------------------------

When you have formatting patterns used across many chapters, define a reusable Typst function in
``_templates/theme.typ``:

.. code-block:: typst

   #let status-badge(status, text-content) = {
     let bg = if status == "ok" { rgb("#edf5f2") } else { rgb("#fff9e6") }
     let fg = if status == "ok" { rgb("#216553") } else { rgb("#854d0e") }
     box(fill: bg, inset: (x: 6pt, y: 3pt), radius: 3pt, baseline: 10%)[
       #text(fill: fg, size: 0.85em, weight: "bold")[#text-content]
     ]
   }

In any document, invoke the function inside a raw Typst block:

.. code-block:: rst

   .. only:: typst

      .. raw:: typst

         #status-badge("ok")[SLO COMPLIANT]

This keeps your reStructuredText prose focused on documentation while granting full access to Typst's
typesetting engine for printed publications.
