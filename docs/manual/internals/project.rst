The project builder: crates and seams
=====================================

The Odin ``lib/project`` package built a documentation project in one package: load
``conf.toml``, discover and read the documents, resolve the project tables, write one
builder's output, and publish it. The Rust port splits it into crates with one
responsibility each, so that builders, readers and the command line can be written,
tested and reviewed apart. This page records the split and the seams between the
crates; it is kept up to date as the crates land. Each crate's ``README.md`` is its API
cheat sheet.

The crates
----------

Dependencies point one way, from the configuration up to the command line::

   gd-conf ──► gd-migrate
      │
      ▼
   gd-env ──► gd-intl ──► gd-read
      │
      ▼
   gd-outputs ──► gd-resolve ──► gd-link ──► gd-site, gd-book
                                     all ──► gd-project ──► gd-cli ──► cmd/guidedog

``gd-serve`` (the preview server) needs only the standard library and gd-host;
``gd-i18n`` (translating documents) only the engine and gd-gettext.

gd-conf
   ``conf.toml``: the ``Config`` model with Sphinx's names and defaults, loading
   (``load_config``, ``find_project``), ``-D`` overrides (``apply_override``), the
   checks Sphinx makes when it starts (``check_config``), the settings table gd-migrate
   converts ``conf.py`` with (``SETTINGS``, ``find_setting``, ``suggestion``), trust
   (``readable_roots``, ``confine_untrusted``), and ``Digest``.
gd-migrate
   ``conf.py`` to ``conf.toml``, and ``guidedog quickstart``.
gd-env
   The foundation: build vocabulary (``BuilderKind``, ``BuildOptions``,
   ``BuildResult``, ``OutputFormat``, ``Asset``, ``Rendered``, ``UiText``), discovery
   (``discover``, ``SourceFile``), the environment (``Environment``, ``DocInfo``), the
   object store (``ObjectStore``), lookups and traces (``Lookup``, ``Trace``), the
   settings digests, and the build plugins of one build (``PluginRun``).
gd-outputs
   One ``Generation`` per build and builder: output units, the manifest, reuse, the
   commit journal and publication, recovery of interrupted commits, ``clean``.
gd-resolve
   The project tables (``resolve``), inventories (``load_intersphinx``), cross-reference
   resolution without the document tree (``resolve_xref``), the answers that verify a
   page's trace (``lookup_answer``), the indexes' data, ``BuildView`` and the
   ``OutputBuilder`` trait.
gd-link
   The link pass: rewrites a document's tree for one output (references, images,
   numbers, ``only``, todo lists, graphs) and renders the page.
gd-read
   Incremental reading: parses changed documents, collects their symbols, and hosts
   autodoc, odindoc and rustdoc.
gd-site, gd-book
   The builders: html, dirhtml and singlehtml (templates, search index, static files);
   pdf (Typst) and text.
gd-intl, gd-i18n
   Catalogs, the interface translator, the gettext builder; the translation pass.
gd-project
   The coordinator and public surface: ``Project``, ``LoadOptions``, ``load``,
   ``build``, ``clean``, ``watch_roots``, ``build_limits`` and the trusted and
   untrusted policies.
gd-cli
   The command line, and ``run_with(linked)`` for custom binaries.

Seams
-----

S1: gd-conf
~~~~~~~~~~~

``load_config(dir, &LoadOptions) -> Result<Loaded, Problem>`` with
``LoadOptions { conf_dir, no_config, overrides, source, output }`` and
``Loaded { config, warnings, settings }``. ``Config`` keeps the general settings flat
(``project``, ``root_doc``, ``exclude_patterns``, ``numfig``, ...) and groups the rest:
``mathjax``, ``intersphinx``, ``autodoc``, ``odin_api``, ``rust_api``, ``plugins``,
``html``, ``pdf``, ``graphviz``, ``myst``, ``intl``. The conf.toml keys stay flat.
``ObjectTypeDecl``, ``AutodocDefaultOption`` and ``IntersphinxProject`` are gd-conf's
own types; the crates that need them convert. Failures are ``gd_host::Problem``
(``config.invalid``, ``cli.define``, ``project.missing``, exit 2) and warnings
``gd_report::Report``.

S2: gd-env
~~~~~~~~~~

``DocId(u32)`` indexes ``Environment::docs`` (sorted by docname). ``Digest`` is
gd-conf's, re-exported. ``SourceFile { docname, path, relative, kind, generated }``;
a generated page (an API page) carries its text and the files it was made from.
``DocInfo`` is Odin's ``Doc_Info`` with ``Option`` for every "none" and a
``CachedFile { path, digest }`` for the object and the search words.
``load_environment(path, settings, fresh, &budget)``, ``ENVIRONMENT_VERSION`` 26.
``ObjectStore`` writes ``objects/<docname>.<digest16>.gdo`` and loads it into a
``guidedoc::Workspace``. ``Lookup { kind: LookupKind, key }`` and ``Trace`` (each
lookup once). ``Rendered { body, assets, trace, reports, failed }``.
``trait UiText: Sync { gettext, ngettext }``.

S3: gd-outputs
~~~~~~~~~~~~~~

``output_paths(&config, kind, output_dir) -> OutputPaths { out_dir, next, old, legacy,
build_dir, doctrees, manifest, journal }``. ``Generation::begin(paths, options,
&mut reports)`` settles interrupted commits; ``reuse(key, inputs, answer)`` is the cutoff
point; ``unit(key) -> Unit`` (``emit``, ``emit_text``, ``emit_copy``, ``emit_tree``,
``stage_path``, ``finish(UnitRecord)``, ``fail``; dropped unfinished counts as failed);
``commit(CommitRecords { environment, store }) -> CommitOutcome``; dropping a generation
abandons it. ``clean(&config)``.

S4: gd-resolve
~~~~~~~~~~~~~~

``resolve(&config, &env, ui, &mut reports) -> Tables``. ``BuildView<'b>`` borrows the
configuration, options, builder, environment, tables, inventories, object store,
registry, policy, limits, interface words, dates, plugin run, budget, readable roots and
output settings. ``resolve_xref(&view, &XrefQuery, &mut Trace) -> Result<Resolved,
Box<Unresolved>>``; ``Unresolved`` carries whether to warn, the code and category, a
hint and a "Did you mean". ``lookup_answer(&view, page, &Lookup) -> Digest``.
``trait OutputBuilder { kind(); write(&view, &mut Generation, &mut reports) }`` is
implemented by gd-site, gd-book and gd-intl; gd-project maps each ``BuilderKind`` to one
in a single ``match``.

S5: plugins
~~~~~~~~~~~

``PluginRun::start(PluginStart { config, linked, kind, untrusted })`` looks each
selected ID up in the linked registry first, then in the machine registry; an
``--untrusted`` build refuses any (``plugins.untrusted``); at most 64. One
``Mutex<PluginSession>`` per plugin per build; ``process_source`` is called by gd-read,
``process_html`` and ``assets`` by gd-site (HTML builders only). ``digest()`` (identity,
cache key, configuration and dependency files, assets) joins the reading and output
settings. Sessions stop when the run is dropped. ``gd_project::LoadOptions`` owns the
linked ``gd_plugins::Registry``.

S6, S7
~~~~~~

gd-read: ``read_documents(&ReadContext, previous: Environment, &[SourceFile]) ->
Result<ReadOutcome, Problem>``, which fails only when the registry has no reader for a
document (``build.reader``); a document that cannot be read is a failed ``DocInfo`` in
the outcome. ``api_sources`` makes the generated Odin and Rust API pages and
``reread_document`` reads a document again whose object is stale. The autodoc, odindoc
and rustdoc hosts implement guidedoc-sphinx's provider trait. gd-site: templates through ``gd_jinja::Environment`` with a custom
loader (``templates_path``, then the built-in theme).

S8: gd-project
~~~~~~~~~~~~~~

``load(dir, &LoadOptions { config, plugins, registry }) -> Result<Project, Problem>``
checks the engine registry (``core.registry.invalid``; ``None`` is
``guidedoc_defaults::registry()``) and reads ``conf.toml``. ``Project { config,
warnings, settings, registry, plugins }`` is plain data. ``build(&project, kind,
&options) -> BuildResult`` never fails outright; it runs, in order:

1. ``confine_untrusted`` on its own copy of the configuration (``--untrusted``);
   ``output_paths`` and ``Generation::begin``, which settles an interrupted commit
   first; ``build_limits`` (``build.stack``); ``PluginRun::start``.
2. ``discover``, ``contents_root``, ``api_sources`` and ``with_generated``: the sources.
3. ``load_intl`` and, except for gettext, ``localize_config``; ``build_dates``;
   ``load_intersphinx`` (no network when untrusted). The build stage becomes Reading.
4. ``build_settings`` (reading settings, translations, trust, limits, plugins);
   ``load_environment`` and ``read_documents``; the documents' reports join the
   build's.
5. ``resolve``; the stage becomes Writing. A ``BuildView`` with
   ``output_settings``, and one ``match`` on ``BuilderKind``: ``SiteBuilder``
   (html, dirhtml, singlehtml), ``PdfBuilder``, ``TextBuilder``, ``DummyBuilder``,
   ``GettextBuilder`` (with ``reread_document`` as its ``Reread``).
6. ``suppress_warnings``; then a build with an unread document (``build.read``) or,
   with ``-W``, any warning or error (``build.warnings``) abandons its generation;
   any other commits it. ``published`` and ``unsettled`` come from the commit.

``BuildResult::written`` and ``unchanged`` are the generation's units written and kept
besides the site unit (``Generation::written_count``, ``reused_count``); a builder whose
whole output is the site unit (singlehtml, gettext) counts it as one output.

Ownership
---------

Odin's project and build arenas are gone. ``load`` returns an owned ``Project``; a build
takes ``&Project`` and clones the configuration, so the type system enforces that a
build never changes its project (an ``--untrusted`` build confines only its own copy).
Only the coarse ``gd_host::Budget`` remains, for the files and sessions a build holds,
with ``--budget`` hints; ``BuildOptions::on_budget`` decides at the budget (a terminal
asks whether to raise it). Pages render on ``gd_host::run_workers``: each worker owns a
``guidedoc::Workspace`` and returns a ``Rendered``; the main thread emits units in
docname order.
