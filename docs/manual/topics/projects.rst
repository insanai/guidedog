How a project works
===================

The project root is the directory containing ``conf.toml``.
``root_doc`` names the document that starts the reading order.
Guidedog discovers source documents, reads them, resolves their relations,
and publishes a complete output generation.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Guidedog's end-to-end documentation and static site compilation pipeline.
   :alt: Pipeline traverses discovery, source plugins, parallel reading, publication gate, reference resolution, rendering, HTML plugins, and atomic staging.

   digraph project_pipeline {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.38"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.22,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     setup [class="gd-plugin", label=<<B>Plugin setup</B><BR/><FONT POINT-SIZE="11">Load sessions; collect assets once</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e"];
     discover [class="gd-input", shape=note, label=<<B>1. Discovery</B><BR/><FONT POINT-SIZE="11">Find sources; apply exclude_patterns</FONT>>, fillcolor="#fbfaf3"];
     source_hook [class="gd-plugin", label=<<B>2. Source Plugins</B><BR/><FONT POINT-SIZE="11">Transform primary source text</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e"];
     parse [label=<<B>3. Parallel Reading</B><BR/><FONT POINT-SIZE="11">RST and Markdown to flat preorder trees</FONT>>, fillcolor="#edf5f2"];
     gate [class="gd-choice", shape=diamond, label=<<B>4. Publication Gate</B><BR/><FONT POINT-SIZE="11">Filter drafts, future publish-at,<BR/>and expired pages against clock</FONT>>, fillcolor="#fff4e6", color="#ca8a04", fontcolor="#854d0e"];
     resolve [label=<<B>5. Resolve References</B><BR/><FONT POINT-SIZE="11">Toctrees, postlists, labels, domains</FONT>>, fillcolor="#edf5f2"];
     render [label=<<B>6. Rendering</B><BR/><FONT POINT-SIZE="11">HTML, collections &amp; feeds via Jinja;<BR/>books &amp; article PDFs via Typst</FONT>>, fillcolor="#edf5f2"];
     html_hook [class="gd-plugin", label=<<B>7. HTML Plugins</B><BR/><FONT POINT-SIZE="11">Transform complete HTML pages</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e"];
     stage [class="gd-output", label=<<B>8. Complete and Commit</B><BR/><FONT POINT-SIZE="11">Complete staging; journal; switch<BR/>generation</FONT>>, shape=cylinder, fillcolor="#eef4fa"];

     setup -> discover -> source_hook -> parse -> gate -> resolve -> render;
     render -> html_hook -> stage;
     render -> stage [label="PDF / text"];
     setup -> stage [style=dashed, label="assets", constraint=false];
   }

Documents and resources
-----------------------

A source suffix selects a reader. RST and project Markdown share the project graph.
Images and downloads are resources. Guidedog copies resources that the documents use.
Theme assets are listed in ``html_static_path``.
An include must remain within the source directory or the configured ``include_roots``.

Separate projects and mounted sites
-----------------------------------

We keep a manual and design records in separate projects. They have different readers and different reading orders. Each project has its own configuration, templates, cache, and output. The Guidedog repository uses ``docs/manual`` and ``docs/gds`` this way, and mounts them into the root website in ``docs/conf.toml`` via ``[[site.mounts]]``.

Incremental work
----------------

Guidedog records the inputs and lookups used by each output. A changed source is reread. A changed dependency invalidates its users. A changed template invalidates the pages whose template chain loaded it. An unchanged document may be loaded from its saved object, including withheld drafts and scheduled posts that become eligible on a later build clock.

The cache is evidence about a previous build. It is not the source of truth.
``-E`` discards the saved environment for the next build.
``-a`` writes all outputs. They solve different problems.

.. math::

   \text{reuse} = \text{same inputs} \land \text{same lookups} \land \text{output exists}

An explicit timestamp in a template is also an input to the rendered page.
A page that prints the current minute can change even when its prose does not.
We use ``SOURCE_DATE_EPOCH`` when we need a fixed build time.

Publication
-----------

A build writes changes to a sibling staging directory.
It completes that directory, flushes the files and records, and then switches generations.
Readers see a complete previous or new publication.
An interrupted commit is settled on the next build.
The design and its proof obligations are in :doc:`../internals/publication`.
