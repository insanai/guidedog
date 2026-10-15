Guidedog API reference
======================

This is the reference of Guidedog's Rust crates. The ``rust`` domain's API pages generate it from their source.
Each public module has a page with its structs, enums,
traits, functions, constants, and macros, and their doc comments. The page is laid out as rustdoc
lays it out. The :doc:`/api/index` lists the crates and their modules.

The crates are grouped by responsibility:

The engine
   ``guidedoc`` (the AST, builder, diagnostics, registry, and pipeline), its readers
   ``guidedoc-rst``, ``guidedoc-commonmark`` and ``guidedoc-sphinx``, its renderers
   ``guidedoc-html``, ``guidedoc-typst`` and ``guidedoc-text``, and
   ``guidedoc-defaults``, the built-in registry. These use only the standard library.

Host and supporting libraries
   ``gd-report`` (Elm-style reports), ``gd-host`` (files, budget, publication,
   workers), ``gd-jinja``, ``gd-highlight``, ``gd-gettext``, ``gd-toml``, ``gd-json``,
   ``gd-typst``, ``gd-graphdog``, ``gd-rustdoc``, ``gd-plugins`` (the plugin SDK), and
   ``gd-discussion``. None of them depends on the engine.

Project and command line
   ``gd-project`` (load and build a project) and ``gd-cli`` (the ``guidedog``
   commands, and ``run_with`` for a binary with linked plugins).

Each crate's ``README.md`` in the repository is a short cheat sheet. The manual's
internals chapters describe the designs. Its ``rust`` chapter describes how these
pages are generated.

.. toctree::
   :maxdepth: 1
   :caption: Crates
