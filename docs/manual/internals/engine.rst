The Guidedoc engine
===================

Guidedoc is the conversion core: the ``guidedoc`` crate and the reader and renderer
crates it registers. It reads a source, builds a validated semantic tree, runs passes,
and renders bytes. It does not open files, run compilers, or publish artifacts; the
host does that. The engine crates use only the standard library, so another program can
embed them.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Effects belong to the host; document semantics belong to Guidedoc.
   :alt: The CLI calls the project host, which owns workspaces and runs the conversion core; the host also drives Graphviz, Typst and tree-sitter.

   digraph layers {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     cli [class="gd-input", label=<<B>gd-cli</B><BR/><FONT POINT-SIZE="11">Arguments and terminal presentation</FONT>>, fillcolor="#fbfaf3"];
     project [label=<<B>gd-project and builders</B><BR/><FONT POINT-SIZE="11">Configuration, cache, files, publication</FONT>>, fillcolor="#edf5f2"];
     core [label=<<B>Guidedoc engine</B><BR/><FONT POINT-SIZE="11">Readers, tree, passes, renderers</FONT>>, fillcolor="#e0eee9"];
     native [class="gd-input", label=<<B>Native integrations</B><BR/><FONT POINT-SIZE="11">Graphviz, Typst, tree-sitter</FONT>>, fillcolor="#fbfaf3"];

     cli -> project [label="invokes"];
     project -> core [label="owns workspaces"];
     project -> native [label="orchestrates"];
   }

Crates and their direction
--------------------------

Dependencies point one way: the engine knows nothing of the crates above it.

``guidedoc``
   The AST, the builder and validator, diagnostics, the adapter registry, symbol
   tables, the object format, and the conversion pipeline.

Readers and renderers
   ``guidedoc-rst`` (with ``guidedoc-rst-inline``), ``guidedoc-commonmark`` (CommonMark
   and MyST), ``guidedoc-sphinx`` (Sphinx directives, roles and domains, including the
   ``odin`` and ``rust`` domains), ``guidedoc-cpp``; ``guidedoc-html``,
   ``guidedoc-typst``, and ``guidedoc-text``.

``guidedoc-defaults``
   The built-in registry, and the adapter that joins ``gd-highlight`` to the engine's
   ``Highlighter`` trait.

The host
   ``gd-host`` meets the operating system: sessions, confined files, atomic
   publication, the coarse budget, worker threads, crash reports. The project builder
   (``gd-conf``, ``gd-env``, ``gd-read``, ``gd-resolve``, ``gd-link``, ``gd-outputs``,
   ``gd-site``, ``gd-book``, ``gd-intl``, ``gd-project``) and ``gd-cli`` sit above it,
   and ``cmd/guidedog`` is the program.

Supporting libraries (``gd-toml``, ``gd-json``, ``gd-jinja``, ``gd-highlight``,
``gd-gettext``, ``gd-graphdog``, ``gd-typst``, ...) never depend on ``guidedoc``; only
adapters join them to the engine. Typst lives only inside ``gd-typst``.

Conversion data flow
--------------------

A document passes through distinct stages. Each stage works in the caller's
``Workspace``, which keeps its buffers between conversions:

.. graphviz::
   :class: graphviz-adaptive
   :caption: Conversion data flow within the Guidedoc engine.
   :alt: Source text becomes a flat tree. Passes transform it. One rendering run writes into a bounded emitter.

   digraph core_pipeline {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.35", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     source [class="gd-input", label=<<B>Source text</B><BR/><FONT POINT-SIZE="11">(&amp;str, UTF-8 by type)</FONT>>, shape=note, fillcolor="#fbfaf3"];
     reader [label=<<B>Reader</B><BR/><FONT POINT-SIZE="11">(RST, CommonMark or MyST)</FONT>>, fillcolor="#edf5f2"];
     tree [label=<<B>Semantic tree</B><BR/><FONT POINT-SIZE="11">(flat preorder Vec, validated)</FONT>>, fillcolor="#edf5f2"];
     passes [label=<<B>Passes</B><BR/><FONT POINT-SIZE="11">(Unchanged or Rebuilt)</FONT>>, fillcolor="#edf5f2"];
     render [label=<<B>Renderer</B><BR/><FONT POINT-SIZE="11">(one run, bounded emitter)</FONT>>, fillcolor="#e0eee9"];
     output [label=<<B>Output</B><BR/><FONT POINT-SIZE="11">(borrowed from the workspace)</FONT>>, shape=cylinder, fillcolor="#eef6f2"];

     source -> reader -> tree -> passes -> render -> output;
   }

The staged calls are ``read_document``, ``transform_document``, ``check_render`` and
``render_document``; ``convert`` runs them for one request, and ``convert_text`` is the
one-call path with a default request:

.. code-block:: rust

   use guidedoc::{convert_text, Workspace};

   let registry = guidedoc_defaults::registry();   // the built-in readers and renderers
   let mut ws = Workspace::new();                  // reuse it: buffers are kept
   match convert_text(&mut ws, &registry, "commonmark", "html", "# Hello\n") {
       Ok(html) => println!("{html}"),             // borrows ws until the next convert
       Err(fail) => eprintln!("{fail}"),           // title, message, and hint
   }

Every failure is a ``Fail { status, need, diagnostic }``: the status is the class
(``InvalidInput``, ``Unsupported``, ``Policy``, ``Capacity``, ``Limit``, ``Cancelled``,
``Internal``), and the diagnostic has a stable code, a title, a message, and a hint.

The boundary
------------

The workspace owns its tables as ``Vec`` and ``String`` values and clears them between
conversions, keeping their capacity, so the engine's own work in steady state allocates
nothing; a test with a counting allocator checks it. Readers and renderers still build
small scratch vectors per conversion; a second counting-allocator test bounds them per
reader and renderer and checks that they do not grow (see :doc:`testing`). Growth driven by the input uses
``try_reserve`` and stays within ``Limits`` (nodes, text bytes, output bytes, reports,
depth, work, stack, includes), so running out is a ``Capacity`` or ``Limit`` result,
never a panic. A third-party compiler has its own allocation rules; the guarantee names
the conversion boundary, not the whole process. See :doc:`memory`.

The tree
--------

Nodes have newtype ids (``NodeId(u32)``). Hot columns hold the kind and the subtree
range; cold tables hold the payloads particular kinds need. A flat preorder ``Vec`` lets
a traversal skip a whole subtree without following pointers.

For node :math:`i`, let :math:`e_i` be its exclusive subtree end. The subtree occupies
:math:`[i,e_i)`. A leaf has :math:`e_i=i+1`. A parent closes after its descendants. See
:doc:`algorithms` for the interval proof and :doc:`ast` for the normative node shapes.

The API
-------

The API has few concepts and plain data. Requests are public-field structs built from
``default_request`` with struct-update syntax; registries are immutable values holding
reader, pass, and renderer descriptors. There is no runtime plugin discovery and no
global registration. Readers are ``Reader`` constants whose ``read`` function builds
nodes with the builder; renderers walk the document and write to the emitter; a pass is
a closure that returns ``Unchanged`` at no cost or copies what it keeps into a rebuilt
tree.

Ownership is the key rule, and the compiler enforces it. A ``Document<'ws>`` borrows
its workspace, so a document or an output cannot outlive the next conversion, and a
stale or foreign handle does not compile. The Odin engine checked generations and
revisions at run time instead; :doc:`engine-api-map` maps its names to the Rust ones.
A result the host keeps must own its text. The crate's ``README.md`` is the API cheat
sheet, the generated reference is :rust:mod:`guidedoc`, and the design record is GDS
0003 in ``docs/gds``.
