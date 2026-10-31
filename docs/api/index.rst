Guidedog API reference
======================

This is the reference of Guidedog's Rust crates. The ``rust`` domain's API pages generate it from their source.
Each public module has a page with its structs, enums,
traits, functions, constants, and macros, and their doc comments. The page is laid out as rustdoc
lays it out. The :doc:`/api/index` lists the crates and their modules.

The crates are grouped by responsibility and publication boundary:

The engine (reusable)
   ``guidedoc`` (the AST, builder, diagnostics, registry, and pipeline), its readers
   ``guidedoc-rst`` (with ``guidedoc-rst-inline``), ``guidedoc-commonmark``, and
   ``guidedoc-sphinx`` (with ``guidedoc-cpp``), its renderers ``guidedoc-html``,
   ``guidedoc-typst``, ``guidedoc-text``, and ``guidedoc-rst-writer``, and
   ``guidedoc-defaults``, the built-in registry. The core engine crates use only the
   standard library (``guidedoc-defaults`` joins them with ``gd-highlight``).

Supporting libraries, API generators, and plugin SDK (reusable)
   ``gd-report`` (Elm-style reports), ``gd-toml``, ``gd-json``, ``gd-sha``, ``gd-flate``,
   ``gd-jinja``, ``gd-highlight``, ``gd-gettext``, ``gd-graphdog``, and ``gd-inventory``;
   the static source scanners and API generators ``gd-pyscan``, ``gd-autodoc``,
   ``gd-odindoc``, and ``gd-rustdoc``; and ``gd-plugins`` (the plugin SDK and process
   protocol). None of these crates depends on the engine.

Internal host, project builder, and command line (``publish = false``)
   ``gd-host`` (confined files, coarse budget, atomic publication, workers),
   ``gd-typst`` (the embedded and external Typst compiler driver), ``gd-discussion``
   (discussion catalogs and ``guidedog dsc``), ``gd-project`` (load and build a project),
   and ``gd-cli`` (the ``guidedog`` commands, and ``run_with`` for a binary with linked
   plugins). ``gd-host``, ``gd-discussion``, ``gd-project``, and ``gd-cli`` sit above the
   engine and remain internal to Guidedog.

Each crate's ``README.md`` in the repository is a short cheat sheet. The manual's
internals chapters describe the designs. Its ``rust`` chapter describes how these
pages are generated.

.. toctree::
   :maxdepth: 1
   :caption: Crates
