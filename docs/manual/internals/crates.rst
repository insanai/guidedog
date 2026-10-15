Crates: reusable and internal
=============================

Some of Guidedog's crates are useful beyond Guidedog. We publish those to crates.io and keep the rest internal to the program.

Take one library
----------------

When another Rust program only needs to read TOML, we add one line to its ``Cargo.toml``:

.. code-block:: toml

   [dependencies]
   gd-toml = "0.7"

.. code-block:: rust

   fn main() {
       let text = "[site]\ntitle = \"Guide\"\n";
       match gd_toml::parse(text) {
           Ok(table) => {
               let title = table.path(&["site", "title"]).and_then(|v| v.as_str());
               println!("{title:?}");
           }
           Err(err) => eprintln!("{err}"),
       }
   }

The program receives ``gd-toml`` and ``gd-report``, nothing else: no engine, no Typst, no command line. Every reusable crate is like this. It needs only other reusable crates and the few external crates the project chose, so taking one never pulls in the program around it.

To convert documents, we take the engine instead. ``guidedoc-defaults`` registers every reader and renderer, so one dependency and one call convert MyST to HTML:

.. code-block:: rust

   fn main() -> Result<(), guidedoc::Fail> {
       let registry = guidedoc_defaults::registry();
       let mut ws = guidedoc::Workspace::new();
       let html = guidedoc::convert_text(&mut ws, &registry, "myst", "html", "# Hello\n")?;
       println!("{html}");
       Ok(())
   }

A program that needs only CommonMark and HTML registers ``guidedoc_commonmark::READER`` and ``guidedoc_html::RENDERER`` itself and leaves the other readers out. Each crate's README starts with such a complete program.

Two groups
----------

The **reusable** crates are the engine, the supporting libraries, the API generators and the plugin SDK. Each is published, has a README written for a reader who has never seen Guidedog, and keeps its API for other projects as well as for ours.

The **internal** crates are the parts of the Guidedog program: configuration, the project builder, the site and book builders, translation, publication and the command line. They set ``publish = false``. Their APIs follow the program's needs and change with it, so we promise nothing about them to anyone else.

The engine
~~~~~~~~~~

The engine crates use only the standard library, so they embed wherever Rust compiles.

``guidedoc``
   The document engine: the AST, the builder and its validation, the workspace that owns and reuses the document tables, limits, diagnostics, the registry of readers and renderers, and the one-call ``convert_text`` beside the staged pipeline.
``guidedoc-rst``
   The reStructuredText reader: block structure, explicit markup and every standard Docutils directive, with the extension points the Sphinx layer uses.
``guidedoc-rst-inline``
   reStructuredText inline markup and reference resolution, shared by the rst reader and the Sphinx layer.
``guidedoc-commonmark``
   CommonMark 0.31.2 (reader id ``commonmark``) and MyST Markdown (``myst``).
``guidedoc-sphinx``
   Sphinx's directives, roles and domains (py, c, cpp, js, rst, std, math, odin, rust) for both the rst and the MyST reader.
``guidedoc-cpp``
   C++ declarations parsed as Sphinx's C++ domain parses them, with every id version. It knows nothing of documents; ``guidedoc-sphinx`` turns its data into nodes.
``guidedoc-html``, ``guidedoc-typst``, ``guidedoc-text``, ``guidedoc-rst-writer``
   The renderers: HTML, Typst source (compiled to PDF elsewhere), plain text as Sphinx's text builder writes it, and reStructuredText that reads back into the same tree.
``guidedoc-defaults``
   One registry with every reader and renderer and the code highlighter: ``guidedoc_defaults::registry()``.

.. graphviz::
   :class: graphviz-adaptive
   :caption: The engine crates and what each one needs.
   :alt: Every reader and renderer needs guidedoc. guidedoc-rst also needs guidedoc-rst-inline. guidedoc-sphinx needs the rst, inline, CommonMark and C++ crates. guidedoc-defaults joins all readers and renderers with gd-highlight.

   digraph engine {
     graph [rankdir=BT, bgcolor="transparent", pad="0.3", nodesep="0.3", ranksep="0.45"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.15,0.08"];
     edge [color="#216553", arrowsize=0.7];

     defaults [label=<<B>guidedoc-defaults</B><BR/><FONT POINT-SIZE="11">every reader and renderer</FONT>>];
     sphinx [label="guidedoc-sphinx"];
     rst [label="guidedoc-rst"];
     inline [label="guidedoc-rst-inline"];
     commonmark [label="guidedoc-commonmark"];
     cpp [label="guidedoc-cpp"];
     renderers [label=<<B>renderers</B><BR/><FONT POINT-SIZE="11">html, typst, text, rst-writer</FONT>>];
     highlight [class="gd-input", label="gd-highlight", fillcolor="#fbfaf3"];
     guidedoc [label=<<B>guidedoc</B><BR/><FONT POINT-SIZE="11">AST, workspace, registry</FONT>>];

     defaults -> {sphinx rst commonmark renderers highlight};
     sphinx -> {rst inline commonmark cpp};
     rst -> inline;
     {sphinx rst inline commonmark renderers} -> guidedoc;
   }

``guidedoc-defaults`` also points at ``guidedoc`` directly; the drawing leaves that edge out. ``guidedoc-cpp`` needs nothing at all: it is a parser with plain data, so the Sphinx layer, not the parser, decides what a declaration becomes in a document.

Libraries
~~~~~~~~~

The libraries never depend on the engine. That is why the engine can use them through adapters while another program uses them alone.

``gd-report``
   Elm-style diagnostic reports (title, location, excerpt with carets, hint) rendered as text or JSON. The other libraries' errors become these reports.
``gd-toml``
   A TOML 1.0 parser and writer; every value knows its line and byte offset.
``gd-json``
   JSON over ``serde_json``: duplicate-key and depth checks, HTML-safe output.
``gd-sha``, ``gd-flate``
   SHA-256, SHA-1 and FNV-1a; DEFLATE and zlib with output limits.
``gd-jinja``
   A Jinja 3.x template engine.
``gd-highlight``
   Syntax highlighting with Pygments' token classes and styles for 35 languages.
``gd-gettext``
   GNU gettext PO and MO catalogs, plural forms, merging and statistics.
``gd-graphdog``
   Graphviz DOT to SVG, PNG or DOT with the Graphviz C library in process. It is the one crate that links a system library (``libgvc`` and ``libcgraph``).
``gd-inventory``
   Sphinx object inventories (``objects.inv``) for intersphinx.

API generators
~~~~~~~~~~~~~~

The generators read source code without compiling or running it and write reStructuredText for the Sphinx domains.

``gd-pyscan``
   Static analysis of Python modules with tree-sitter.
``gd-autodoc``
   Sphinx autodoc's output from ``gd-pyscan``'s analysis.
``gd-odindoc``
   Odin packages, read for every build target at once, for the ``odin`` domain.
``gd-rustdoc``
   Rust crates, read with tree-sitter, for the ``rust`` domain.

Plugin SDK
~~~~~~~~~~

``gd-plugins``
   Typed build plugins and publishers, linked into a program or served as programs over the JSON Lines protocol (see :doc:`plugins`).

.. graphviz::
   :class: graphviz-adaptive
   :caption: The libraries, generators and SDK that need another crate.
   :alt: gd-toml, gd-json and gd-graphdog need gd-report. gd-inventory needs gd-flate and gd-report. gd-plugins needs gd-report, gd-json, gd-toml and gd-sha. gd-autodoc needs gd-pyscan. gd-json uses serde_json; gd-pyscan and gd-rustdoc use tree-sitter.

   digraph libraries {
     graph [rankdir=BT, bgcolor="transparent", pad="0.3", nodesep="0.3", ranksep="0.45"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.15,0.08"];
     edge [color="#216553", arrowsize=0.7];

     plugins [label=<<B>gd-plugins</B><BR/><FONT POINT-SIZE="11">plugin SDK</FONT>>];
     toml [label="gd-toml"];
     json [label="gd-json"];
     sha [label="gd-sha"];
     graphdog [label="gd-graphdog"];
     inventory [label="gd-inventory"];
     flate [label="gd-flate"];
     report [label=<<B>gd-report</B><BR/><FONT POINT-SIZE="11">Elm-style reports</FONT>>];
     autodoc [label="gd-autodoc"];
     pyscan [label="gd-pyscan"];
     rustdoc [label="gd-rustdoc"];
     serde [class="gd-input", shape=note, label="serde_json, serde", fillcolor="#fbfaf3"];
     treesitter [class="gd-input", shape=note, label="tree-sitter + grammars", fillcolor="#fbfaf3"];

     plugins -> {toml json sha report};
     {toml json graphdog} -> report;
     inventory -> {flate report};
     json -> serde;
     autodoc -> pyscan;
     {pyscan rustdoc} -> treesitter;
   }

The notes are the only external crates the reusable crates use. ``guidedoc``, ``guidedoc-cpp``, ``gd-report``, ``gd-sha``, ``gd-flate``, ``gd-jinja``, ``gd-highlight``, ``gd-gettext`` and ``gd-odindoc`` need no other crate.

Internal crates
---------------

Every other package is internal:

- the host and project layers: ``gd-host``, ``gd-conf``, ``gd-env``, ``gd-outputs``, ``gd-serve``, ``gd-i18n``, ``gd-read``, ``gd-resolve``, ``gd-link``, ``gd-site``, ``gd-book``, ``gd-intl``, ``gd-migrate``, ``gd-pages`` and ``gd-project``;
- the command line and its services: ``gd-cli``, ``gd-theme``, ``gd-discussion``, ``gd-fetch``, ``gd-git``, ``gd-typst`` and ``gd-clock``;
- the programs under ``cmd/``, the tools under ``tools/``, the examples under ``examples/`` and the end-to-end tests under ``tests/integration``.

``gd-typst`` embeds the Typst compiler. Keeping it internal keeps Typst's large dependency tree out of every reusable crate: ``guidedoc-typst`` writes Typst source as text, and only the program compiles it.

``gd-fetch`` reads URLs and files and sends HTTP requests for remote intersphinx inventories and the Pages publisher. It uses ``ureq``, a small blocking HTTP client, with ``rustls`` for TLS and Mozilla's root certificates built in (``webpki-roots``). Guidedog therefore needs no ``curl`` program, OpenSSL or system certificate store, and request headers never appear on a command line. The crate stays internal, so the reusable crates do not carry an HTTP stack.

``gd-clock`` gives the program one answer to "what time is it?". A build stamps its output with ``SOURCE_DATE_EPOCH`` when reproducible builds set it, and with the clock otherwise. The crate turns Unix seconds into civil dates with plain arithmetic and asks the operating system only for the local time zone's offset from UTC. That offset is the crate's one piece of unsafe code (``localtime_r`` on Unix, ``SystemTimeToTzSpecificLocalTime`` on Windows), so the rest of the program needs none for dates. The plugin SDK ``gd-plugins`` is reusable and cannot depend on it; it keeps its own few lines of date arithmetic.

Rules the gate checks
---------------------

The dependency gate in ``tests/integration`` (``tests/gates/dependencies.rs``) reads ``cargo metadata`` and checks three rules for publication:

- every reusable crate is publishable, and every internal one sets ``publish = false``;
- a reusable crate needs only other reusable crates (dev-dependencies are not published, so they are free);
- every path dependency of a reusable crate asks for the workspace version.

The last rule exists because crates.io needs a version beside each path. The root ``[workspace.dependencies]`` names it once per reusable crate, and we bump it together with ``[workspace.package] version`` and ``VERSION``; the gate fails if we forget one.

What a package holds
--------------------

A package holds its ``src`` folder, its README and, for ``gd-plugins``, ``gd-odindoc`` and ``guidedoc-rst-writer``, its examples. Tests and their fixtures stay in the repository: the conformance corpora would make the engine packages several times larger, and a user of the crate does not run them.

We see what a crate would publish, and package several crates at once, with:

.. code-block:: sh

   cargo package --list -p gd-toml
   cargo package --no-verify --allow-dirty -p gd-report -p gd-toml

Packaging several crates in one call lets each resolve the others before they are on crates.io.

To make an internal crate reusable, we first move its dependencies on internal crates out, then add it to ``REUSABLE`` in the gate, give it the package metadata the others have (description, keywords, categories, ``include``, ``readme``), and list it on this page.
