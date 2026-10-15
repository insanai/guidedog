Write a plugin
==============

A plugin adds a service to Guidedog. A build plugin can transform source text,
transform a rendered HTML page, or supply static files. A publisher sends a
finished site to a destination. Both are ordinary Rust traits from the
``gd-plugins`` crate. We implement one once. The same value can be linked into
a custom ``guidedog`` binary or served as a separate program.

This chapter builds a product badge. It replaces ``{{product}}`` in source text,
adds a label to HTML pages, and supplies the label's CSS. GitHub Pages then shows
how the same plugin foundation supports a service with external effects.

1. Understand the boundary
--------------------------

A build plugin implements ``gd_plugins::BuildPlugin``. Its ``process_source``,
``process_html`` and ``collect_assets`` methods are optional. The default methods mean
the service is absent (text passes through, no assets). ``load`` reads the plugin's
configuration into the value itself. Dropping the value releases what it holds. The
host owns the build.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Build services precede publication.
   :alt: Source hooks precede parsing. HTML hooks and generated assets enter staging. Publication follows a successful build.

   digraph hooks {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     read [class="gd-input", label="Source text (RST / Markdown)", shape=note, fillcolor="#fbfaf3"];
     source [class="gd-plugin", label=<<B>process_source</B><BR/><FONT POINT-SIZE="11">Pre-parse text transformation</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e"];
     parse [label=<<B>Parse and resolve references</B><BR/><FONT POINT-SIZE="11">Build flat preorder doctrees</FONT>>, fillcolor="#edf5f2"];
     render [label=<<B>Render resolved documents</B><BR/><FONT POINT-SIZE="11">HTML or Typst PDF</FONT>>, fillcolor="#edf5f2"];
     html [class="gd-plugin", label=<<B>process_html</B><BR/><FONT POINT-SIZE="11">Transform complete HTML text</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e"];
     assets [class="gd-plugin", label=<<B>collect_assets</B><BR/><FONT POINT-SIZE="11">Custom files to _static/plugins/ID/</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e"];
     stage [label=<<B>Stage and verify complete output</B><BR/><FONT POINT-SIZE="11">Write the next generation</FONT>>, fillcolor="#edf5f2"];
     commit [label=<<B>Publish local generation</B><BR/><FONT POINT-SIZE="11">Journaled generation switch</FONT>>, shape=cylinder, fillcolor="#e0eee9"];
     publish [class="gd-input", label=<<B>Optional publisher service</B><BR/><FONT POINT-SIZE="11">(guidedog publish, guidedog-pages)</FONT>>, style="rounded,dashed,filled", fillcolor="#fbfaf3"];

     assets -> read [style=dashed, label="setup first"];
     read -> source -> parse -> render;
     render -> html -> stage;
     render -> stage [label="PDF / text"];
     assets -> stage;
     stage -> commit -> publish;
   }

Source hooks run before parsing each primary RST or Markdown document. They apply
to PDF and text builds too. Includes remain the engine's responsibility. HTML
hooks receive complete pages, including generated indexes and additional pages.
Generated assets belong to HTML builders and live below ``_static/plugins/ID/``.
The host collects them once during plugin setup, before reading documents.
They are fixed for that build. Source and HTML hooks cannot add later assets.

A ``Plugin`` is build hooks or a publisher, never both. Normal builds cannot invoke
a publishing service. A plugin reaches a build in one of two ways:

Linked
   A custom ``guidedog`` binary compiles the plugin crate in and passes its plugins to
   the command line with ``gd_cli::run_with``. Rust has no stable ABI, so there is no
   loading of dynamic libraries. Linked code is trusted like the rest of the binary.

Served
   The plugin crate's small binary calls ``gd_plugins::serve_plugin``. We register it
   with ``guidedog plugin register``, which pins its SHA-256. Then it runs as a child process
   per build, behind a bounded JSON Lines protocol. A crash or stall becomes a
   diagnostic. Registration is explicit, and it is not a sandbox: an installed plugin is
   a trusted program.

2. Compile the working example
------------------------------

The product badge lives in the Guidedog repository at
``crates/gd-plugins/examples/build_plugin``. ``badge.rs`` is the plugin, and ``main.rs`` is
the program that serves it. We start at the checkout root, with a Rust toolchain
installed. On macOS or Linux, run:

.. code-block:: sh

   cargo build --release -p guidedog-cmd
   cargo build --release -p gd-plugins --example build_plugin
   target/release/guidedog quickstart build/plugin-tutorial -q -p Orbit -a Team

On Windows, run these commands in PowerShell:

.. code-block:: powershell

   cargo build --release -p guidedog-cmd
   cargo build --release -p gd-plugins --example build_plugin
   target/release/guidedog.exe quickstart build/plugin-tutorial -q -p Orbit -a Team

The first two commands build Guidedog and the plugin program,
``target/release/examples/build_plugin``. The third creates the documentation
project. Registration follows after configuration. Users of a packaged plugin need no
Rust toolchain.

3. Select it for a project
--------------------------

Replace ``build/plugin-tutorial/conf.toml`` with:

.. code-block:: toml

   project = "Orbit"
   plugins = ["product-badge"]
   plugin_timeout = 60

   [plugin_configs]
   product-badge = "badge.toml"

Create ``build/plugin-tutorial/badge.toml``:

.. code-block:: toml

   product = "Orbit"

Replace ``build/plugin-tutorial/index.rst`` with:

.. code-block:: rst

   Welcome
   =======

   Hello {{product}}.

Configuration paths are relative to the project configuration directory. Plugins
run in the order listed in ``plugins``. Merely registering a program does not run it.

Register after writing the configuration. Registration checks the build plugin
in child processes before recording its checksum. On macOS or Linux:

.. code-block:: sh

   target/release/guidedog plugin register product-badge \
     "$PWD/target/release/examples/build_plugin" \
     --project=build/plugin-tutorial --config=badge.toml

On Windows, use one PowerShell command:

.. code-block:: powershell

   target/release/guidedog.exe plugin register product-badge "$PWD/target/release/examples/build_plugin.exe" --project=build/plugin-tutorial --config=badge.toml

4. Build and observe
--------------------

On macOS or Linux, run:

.. code-block:: sh

   target/release/guidedog build html build/plugin-tutorial -W
   target/release/guidedog serve build/plugin-tutorial

On Windows, run:

.. code-block:: powershell

   target/release/guidedog.exe build html build/plugin-tutorial -W
   target/release/guidedog.exe serve build/plugin-tutorial

Open the address printed by the server. The page says "Hello Orbit." and has an
Orbit badge. Its stylesheet is ``_static/plugins/product-badge/banner.css``.
Change the configured name to ``Comet`` and build again. Both the text and badge
change. A further unchanged build writes no output files.

5. Read the implementation
--------------------------

The complete plugin below is the source compiled in step 2. Download
:download:`badge.rs <_static/repository/build-plugin-badge.rs>` and
:download:`main.rs <_static/repository/build-plugin-main.rs>` to keep a local copy.
In our own crate, ``badge.rs`` becomes the library (``src/lib.rs``) and ``main.rs``
the binary, with ``gd-plugins`` and ``gd-toml`` as dependencies.

.. literalinclude:: _static/repository/build-plugin-badge.rs
   :language: rust

``plugin()`` is the public descriptor. ``Plugin::build`` keeps the unloaded ``Badge``
as a prototype and clones it for every session, so sessions never share state. ``info``
names the plugin and declares its hooks. ``load`` reads the configuration through
``LoadContext::read_config``. That call bounds its size, declares the file as a dependency
and fingerprints the bytes. ``load`` then checks the setting and keeps it in the value.

The source hook returns ``Cow::Borrowed`` when the text has no marker, so unchanged
text is never copied. It checks the expanded size against ``MAX_PLUGIN_TEXT`` before
allocating. It replaces every marker, including markers inside code blocks. This is a
text transformation, not a markup parser.

The HTML hook escapes the label and inserts it before the last closing body tag.
It uses ``assets_url`` supplied by the host, so nested pages link to the right CSS.
This small example expects literal lowercase closing head and body tags. A plugin
that rewrites arbitrary HTML should use a suitable parser and state its contract.

The assets hook returns ordinary owned values. There is nothing to release. Hook
results are plain ``Cow``, ``String`` and ``Vec`` values that the host takes over.

The program that serves the same plugin over standard streams is three lines:

.. literalinclude:: _static/repository/build-plugin-main.rs
   :language: rust

``serve_plugin`` handles negotiation and dispatch and returns the exit code. Keep
protocol output on stdout, and write human logs to stderr.

6. Link it into our own guidedog
--------------------------------

A plugin used by every build on a machine, or shipped with a team's tools, can be
compiled into a custom ``guidedog`` binary instead of being registered. We create a binary
crate that depends on ``gd-cli``, ``gd-plugins`` and our plugin crate (``badge`` here),
and give it this ``main``:

.. code-block:: rust

   use std::process::ExitCode;

   use gd_plugins::Registry;

   fn main() -> ExitCode {
       // Registry::new validates IDs, versions and hooks, so a bad list fails here.
       let linked = match Registry::new(vec![badge::plugin()]) {
           Ok(registry) => registry,
           Err(problem) => {
               let style = gd_report::Style::default();
               eprint!("{}", gd_report::render_text(&problem.report(), &style));
               return ExitCode::from(2);
           }
       };
       gd_cli::run_with(linked)
   }

The binary accepts every ``guidedog`` command and option. A build looks up each ID in
``plugins`` among the linked plugins first, then in the machine registry. So the
project's ``conf.toml`` is the same for both deployments and needs no registration
step. ``guidedog plugin list`` shows linked plugins beside registered ones.

A linked plugin's identity in the cache is its ID, its version and the host binary's
own build identity, because its code is part of the host. Rebuilding the binary
rebuilds the affected output. Linked code runs in the host process, so a panic or a
hang in a hook reaches the build. We serve a plugin as a program when it should be
isolated.

7. Keep state and caching explicit
----------------------------------

A plugin value *is* one build's session. The host creates a fresh session per
selected plugin per build, from the prototype. It drops the session when the build ends,
whether it succeeded or failed. ``Drop`` replaces an unload callback: we close files or
connections there.

Use ``LoadContext::read_config`` and ``read_file`` during load. They fingerprint the
bytes actually read and declare dependencies. Keep loaded state unchanged throughout
the build. If we read other files ourselves, we declare their paths with
``declare_dependency`` and add a fingerprint of the loaded values with
``add_cache_key``. Selected environment values go there too. The context only appends,
so the fingerprints the helpers recorded cannot be erased. A plugin must not depend on undeclared
changing data.

The host includes identity, configuration, dependencies, cache key, builder, language
and generated assets in cache invalidation. Hooks take ``&mut self``. When reading
workers share a session, the host serializes calls through a ``Mutex``, so plugins need
no synchronization. Installed programs have a deadline per whole request, including
pipe writes.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Build-plugin lifecycle. Asset collection belongs to setup.
   :alt: Guidedog loads a plugin and collects HTML assets before reading sources. Hooks run during reading and rendering. The session is dropped after success or failure.

   digraph plugin_sequence {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];
     start [label=<<B>Start the session</B><BR/><FONT POINT-SIZE="11">Clone the prototype, or start the program and verify it</FONT>>];
     load [class="gd-plugin", label=<<B>load</B><BR/><FONT POINT-SIZE="11">Read configuration; establish state</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6"];
     assets [class="gd-plugin", label=<<B>collect_assets</B><BR/><FONT POINT-SIZE="11">Once, for HTML builders only</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6"];
     source [class="gd-plugin", label=<<B>process_source</B><BR/><FONT POINT-SIZE="11">Before each primary source is parsed</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6"];
     core [label=<<B>Parse and resolve</B><BR/><FONT POINT-SIZE="11">Host owns the document graph</FONT>>];
     html [class="gd-plugin", label=<<B>Render; process_html</B><BR/><FONT POINT-SIZE="11">HTML hooks receive complete pages</FONT>>, style="rounded,dashed,filled", fillcolor="#fff9e6"];
     commit [label=<<B>Complete staging; commit</B><BR/><FONT POINT-SIZE="11">Only after successful validation</FONT>>];
     drop [class="gd-input", label=<<B>Drop the session</B><BR/><FONT POINT-SIZE="11">Also on failed builds</FONT>>, shape=oval, fillcolor="#fbfaf3"];
     start -> load -> assets -> source -> core -> html -> commit -> drop;
     core -> commit [label="PDF / text"];
   }

Text is limited to 8 MiB per hook and configuration to 1 MiB. Each plugin may supply
at most 256 assets and 16 MiB of asset bytes, and declare at most 256 dependencies.
Paths must be portable and relative. Duplicate names, case-only collisions, traversal
and conflicts with project files are refused. ``--untrusted`` refuses any build that
selects plugins. A child process's memory is outside the host's build budget.

8. Explain failures and test them
---------------------------------

Return a ``Problem`` with a code, title, affected path, explanation, concrete
correction and nonzero exit status: ``Problem::new(code, title, message)`` with
``.hint(...)``, ``.path(...)`` and ``.exit(...)``. For example, ``product = 42`` is
invalid. The badge explains that a string is required and suggests
``product = "Orbit"``. The host adds which plugin stopped and in which stage. Markup
diagnostics after expansion identify plugin-expanded input and show its actual
excerpt. They do not pretend expanded line numbers belong to the original.

Test the plugin value directly with ``PluginSession::load`` and its hook methods, and
through its program with ``process_plugin``. Cover changed settings, unchanged
rebuilds, nested links, invalid markup, asset collisions, cancellation and timeouts.
A failed build must preserve the previous site. The crate's own tests run the badge
both linked and served and require identical outputs.

9. Add a publishing service
---------------------------

A publisher implements ``gd_plugins::Publisher`` and is wrapped with
``Plugin::publisher``. Its methods are ``info``, ``load``, ``plan``, ``publish``, and
optional ``status`` and ``recover``. A session holds configuration and credentials. A
``Snapshot`` freezes the checked site files. A ``Plan`` compares that snapshot with a
target baseline. An ``Outcome`` reports the verified result. These are ordinary values,
and there is nothing to release.

Start with :download:`publisher.rs <_static/repository/publisher.rs>`
(``crates/gd-plugins/examples/publisher.rs``), a working publisher that replaces a local
folder whole. Then follow the GitHub Pages publisher in ``crates/gd-pages``.
Configuration and machine binding establish authority. Planning reads the target.
Publishing writes a durable journal before one conditional Git push. Status and
recovery settle the outcome. Nothing in a source or HTML hook grants publication
authority. ``guidedog publish --publisher=ID --config=FILE`` builds the project and
runs a publisher. ``--dry-run`` stops after the plan.

A remote acceptance is not a live deployment. ``Accepted`` and ``Pending`` return
6. An uncertain write returns ``Unknown`` and 7, with status or recovery directions.
Never retry an uncertain write blindly. See :doc:`howto/github-pages` for the
complete user workflow.

10. Version and distribute the contract
---------------------------------------

The build-plugin interface (``PLUGIN_API_VERSION``) is 1.0 and uses protocol
``guidedog-build-plugin`` 1.0. The publisher interface (``API_VERSION``) is 1.1 and
keeps ``guidedog-publisher`` protocol 1.0. These are separate contracts. The wire
format is the one that programs built with the Odin SDK speak, so registered programs from
earlier releases keep working. Product versions are separate from interface versions.
New hooks arrive as trait methods with "absent" defaults, and host-made inputs are
``#[non_exhaustive]``, so a new field does not break a plugin.

Register each rebuilt program deliberately, because its checksum changes. ``guidedog plugin
inspect ID`` shows its kind and declared hooks. The public catalog describes plugins.
It does not download, install or run them. Custom markup readers, renderers,
directives and roles are not part of this interface. They use the
``guidedoc::Registry`` API in a program that embeds the engine.

11. Check a plugin before registering it
----------------------------------------

``guidedog plugin check PATH --project=DIR --config=FILE`` exercises a build plugin
without changing the registry. Registration runs the same checks automatically:
two fresh loads, repeated source and HTML results, generated asset validation and
cleanup. ``--timeout=SECONDS`` defaults to 60 per process request. Supply a real
configuration when the plugin needs one. Relative configuration paths resolve
from ``--project``, whose default is the current directory.

Passing checks shows that the plugin works with Guidedog. It is not a proof of safety. Guidedog
keeps checking every runtime response and runs installed plugins in separate
processes. A crash, bounds failure or stalled process becomes a diagnostic and
cannot publish an incomplete generation. Linked plugins remain trusted code.

Generic checks never invoke a publisher. Qualify publishers with their own tests and
an explicitly configured dry run. The protocol and the SDK's design are recorded in
:doc:`internals/plugins`.
