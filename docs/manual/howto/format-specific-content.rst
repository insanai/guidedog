Deliver format-specific content and custom directives
======================================================

A single source tree can publish rich, dynamic HTML pages on the web while typesetting elegant,
print-quality PDF books. This guide covers how to route content to specific output formats, declare
custom domain directives, and invoke Typst typesetting functions.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Content routing and directive handling for HTML and PDF outputs.
   :alt: Content passes through format guards and object type tables to produce tailored web and print deliverables.

   digraph format_routing {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];

     rst [class="gd-input", shape=note, fillcolor="#fbfaf3",
          label=<<B>Document source</B><BR/><FONT POINT-SIZE="11">RST / Markdown markup</FONT>>];

     router [label=<<B>Target evaluation</B><BR/><FONT POINT-SIZE="11">Evaluate .. only:: guards</FONT>>];

     html_branch [class="gd-plugin", style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e",
                  label=<<B>HTML builder path</B><BR/><FONT POINT-SIZE="11">Raw HTML + Jinja layouts</FONT>>];

     pdf_branch [class="gd-plugin", style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e",
                 label=<<B>PDF builder path</B><BR/><FONT POINT-SIZE="11">Raw Typst + theme.typ functions</FONT>>];

     html_doc [class="gd-output", fillcolor="#eef4fa",
               label=<<B>HTML webpage</B><BR/><FONT POINT-SIZE="11">Interactive web elements</FONT>>];

     pdf_doc [class="gd-output", fillcolor="#eef4fa",
              label=<<B>PDF book</B><BR/><FONT POINT-SIZE="11">Typeset vector document</FONT>>];

     rst -> router;
     router -> html_branch [label="target: html"];
     router -> pdf_branch [label="target: typst / pdf"];
     html_branch -> html_doc;
     pdf_branch -> pdf_doc;
   }

1. Embed dynamic HTML and client scripts
----------------------------------------

Use ``.. only:: html`` paired with a ``.. raw:: html`` block to inject web-specific markup, interactive
calculators, or embedded audio/video components:

.. code-block:: rst

   .. only:: html

      .. raw:: html

         <div class="interactive-panel" data-endpoint="/api/v1/status">
           <button class="btn-refresh" onclick="checkHealth()">Check Health</button>
           <span id="health-status">Ready</span>
         </div>

To supply the supporting CSS and JavaScript:
1. Place your script and stylesheet in ``_static/`` (e.g. ``_static/interactive.js`` and ``_static/interactive.css``).
2. Register them in ``conf.toml``:

.. code-block:: toml

   html_css_files = ["interactive.css"]
   html_js_files = ["interactive.js"]

The HTML builder copies these files into the build directory and injects ``<link>`` and ``<script>`` tags
into every generated page.

2. Typeset custom Typst formatting for PDF
------------------------------------------

To introduce vector shapes, tailored callout boxes, or custom typography for printed pages, wrap the markup
in ``.. only:: pdf``:

.. code-block:: rst

   .. only:: pdf

      .. raw:: typst

         #rect(
           fill: rgb("#edf5f2"),
           stroke: (left: 3pt + rgb("#216553")),
           inset: (x: 10pt, y: 8pt),
           radius: (right: 3pt),
           width: 100%
         )[
           #text(weight: "bold", fill: rgb("#216553"))[Security Advisory] \
           Always verify token checksums before deploying cryptographic credentials.
         ]

Guidedog emits this block directly into the Typst document before invoking the Typst compiler.
The HTML builder automatically omits this block without emitting warnings.

.. note::

   **Why Typst and not LaTeX?**

   Guidedog generates PDF books through native `Typst <https://typst.app/>`_, not TeX or LaTeX.
   Raw blocks are passed verbatim to the target rendering engine:

   * **Raw Typst** (``.. raw:: typst``): Rendered directly by the Typst compiler in PDF builds.
   * **Raw LaTeX** (``.. raw:: latex``): Not supported in PDF builds. Guidedog reports a
     ``typst.raw.foreign`` warning and omits the content because Typst cannot execute LaTeX commands.
   * **LaTeX Math** (``.. math::`` and ``:math:``): Fully supported. Guidedog includes a built-in
     mathematical translator that converts standard LaTeX equation syntax into native Typst math
     expressions at compile time.
   * **The ``latex`` tag in ``.. only::``**: For backward compatibility with legacy Sphinx projects,
     Guidedog recognizes ``latex`` as an alias for PDF builds in ``.. only::`` expressions. In
     native Guidedog projects, prefer ``.. only:: pdf``.

3. Declare custom domain directives in conf.toml
------------------------------------------------

When you need custom structured blocks with signature parsing, cross-references, and index entries,
declare them in ``conf.toml`` under ``[[object_types]]``:

.. code-block:: toml

   [[object_types]]
   directive = "service"
   role = "service"
   index = "pair: %s; infrastructure service"
   name = "first-word"
   display = "Service: %s"

Now authors can describe services across all documents:

.. code-block:: rst

   .. service:: auth-proxy (port 8443)

      Terminates mutual TLS connections and validates client bearer tokens.

Other documents can link directly to this service using the declared role:

.. code-block:: rst

   Requests pass through :service:`auth-proxy` before reaching backend nodes.

In HTML, Guidedog produces semantic definition tags (``<dl class="service">``) with automatic anchor links.
In PDF, Guidedog typesets a styled description entry and records an index entry in the book's index.

4. Alias existing directives
----------------------------

If you are migrating documentation or want friendlier names for built-in directives, declare
aliases in ``[directive_aliases]`` in ``conf.toml``:

.. code-block:: toml

   [directive_aliases]
   callout = "admonition"
   endpoint = "py:function"

Authors can then use ``.. callout::`` or ``.. endpoint::`` using the options and rules of the target directive.

5. Call reusable Typst functions from templates
-----------------------------------------------

Instead of writing verbose Typst code inline across multiple documents, define a reusable function in
``_templates/theme.typ``:

.. code-block:: typst

   #let danger-box(body) = block(
     fill: rgb("#fde8e8"),
     stroke: 0.5pt + rgb("#9b1c1c"),
     inset: 10pt,
     radius: 4pt,
     width: 100%
   )[
     #text(weight: "bold", fill: rgb("#9b1c1c"))[CAUTION] \
     #v(2pt)
     #body
   ]

Call it concisely from reStructuredText:

.. code-block:: rst

   .. only:: typst

      .. raw:: typst

         #danger-box[Rotating keys immediately invalidates active sessions.]

6. Advanced: Preprocessing directives with a build plugin
---------------------------------------------------------

When you want to invent a completely custom directive syntax and expand it dynamically during build time,
implement a ``gd_plugins::BuildPlugin`` in Rust.

The plugin implements ``process_source``, intercepting the raw document text before parsing:

.. code-block:: rust

   use std::borrow::Cow;
   use gd_plugins::{BuildPlugin, PluginInfo, SourceInput, Problem};

   pub struct ChartPlugin;

   impl BuildPlugin for ChartPlugin {
       fn info(&self) -> PluginInfo {
           PluginInfo::new("chart", "1.0.0")
       }

       fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
           if !input.text.contains(".. chart::") {
               return Ok(Cow::Borrowed(input.text));
           }
           // Replace `.. chart::` with dual-target `.. only:: html` and `.. only:: typst`
           let transformed = input.text.replace(
               ".. chart:: latency",
               concat!(
                   ".. only:: html\n\n",
                   "   .. raw:: html\n\n",
                   "      <div class=\"chart\" data-type=\"latency\"></div>\n\n",
                   ".. only:: typst\n\n",
                   "   .. raw:: typst\n\n",
                   "      #rect(stroke: 0.5pt)[*Latency Chart*]\n"
               )
           );
           Ok(Cow::Owned(transformed))
       }
   }

Register the compiled plugin using:

.. code-block:: sh

   guidedog plugin register ./target/release/my-plugin

Guidedog verifies the plugin's SHA-256 fingerprint, starts it per build, and forwards document sources
through its pipeline.
