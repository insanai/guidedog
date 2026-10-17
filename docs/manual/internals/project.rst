The project builder: crates and seams
=====================================

Building a documentation project means: load ``conf.toml``, discover and read the documents, resolve the project tables, write one builder's output, and publish it. The project builder splits this work into crates with one responsibility each, so that builders, readers and the command line can be written, tested and reviewed apart. This page records the split and the seams between the crates. Each crate's ``README.md`` is its API
cheat sheet.

The crates
----------

Dependencies point one way, from the configuration up to the command line.

.. graphviz::
   :class: graphviz-adaptive
   :caption: The project crates and build flow. Each arrow points from a crate or step to one that builds on it.
   :alt: gd-conf and gd-theme feed gd-migrate, gd-env, gd-site, gd-book and gd-ssg. gd-read feeds filter_publication, which feeds gd-resolve, then gd-link, then gd-site (collections and feeds) and gd-book (books and article PDFs), then gd-project, gd-cli and cmd/guidedog.

   digraph project_crates {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.15,0.08"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     conf [class="gd-input", fillcolor="#fbfaf3", label=<<B>gd-conf</B><BR/><FONT POINT-SIZE="11">conf.toml &amp; [site]</FONT>>];
     theme [class="gd-input", fillcolor="#fbfaf3", label=<<B>gd-theme</B><BR/><FONT POINT-SIZE="11">Built-in layouts &amp; .gdt pack</FONT>>];
     migrate [label=<<B>gd-migrate</B><BR/><FONT POINT-SIZE="11">conf.py &amp; quickstart</FONT>>];
     env [label=<<B>gd-env</B><BR/><FONT POINT-SIZE="11">Environment, PageMeta, Trace</FONT>>];
     intl [label=<<B>gd-intl</B><BR/><FONT POINT-SIZE="11">Catalogs &amp; gettext</FONT>>];
     read [label=<<B>gd-read</B><BR/><FONT POINT-SIZE="11">Incremental reading &amp; API</FONT>>];
     filter_publication [class="gd-choice", shape=diamond, fillcolor="#fff4e6", color="#ca8a04", fontcolor="#854d0e", label=<<B>filter_publication</B><BR/><FONT POINT-SIZE="11">Clock &amp; --preview gate</FONT>>];
     outputs [label=<<B>gd-outputs</B><BR/><FONT POINT-SIZE="11">Generation &amp; manifest</FONT>>];
     resolve [label=<<B>gd-resolve</B><BR/><FONT POINT-SIZE="11">Tables, xrefs, lookup_answer</FONT>>];
     link [label=<<B>gd-link</B><BR/><FONT POINT-SIZE="11">Link pass &amp; page rendering</FONT>>];
     site [label=<<B>gd-site</B><BR/><FONT POINT-SIZE="11">HTML, collections, feeds, sitemap</FONT>>];
     book [label=<<B>gd-book</B><BR/><FONT POINT-SIZE="11">PDF books, article PDFs, text</FONT>>];
     project [label=<<B>gd-project</B><BR/><FONT POINT-SIZE="11">Coordinator &amp; policies</FONT>>];
     ssg [class="gd-plugin", fillcolor="#fff9e6", color="#ca8a04", fontcolor="#854d0e", label=<<B>gd-ssg</B><BR/><FONT POINT-SIZE="11">Authoring, presets, mounts</FONT>>];
     cli [label=<<B>gd-cli</B><BR/><FONT POINT-SIZE="11">Command-line dispatch</FONT>>];
     cmd [class="gd-output", fillcolor="#eef4fa", label=<<B>cmd/guidedog</B><BR/><FONT POINT-SIZE="11">Linked executable</FONT>>];

     {conf theme} -> migrate;
     conf -> env;
     env -> intl -> read -> filter_publication;
     env -> outputs -> resolve;
     filter_publication -> resolve [label="eligible docs"];
     resolve -> link;
     {link theme} -> site;
     {link theme} -> book;
     {read site book migrate} -> project;
     {conf env theme} -> ssg;
     project -> cli;
     {cli ssg} -> cmd;
   }

``gd-serve`` (the preview server) needs only the standard library and gd-host;
``gd-i18n`` (translating documents) only the engine and gd-gettext.

gd-conf
   ``conf.toml``: the ``Config`` model with Sphinx's names and defaults plus ``[site]``
   (``SiteConfig``), loading (``load_config``, ``find_project``), ``-D`` overrides
   (``apply_override``), the checks Sphinx makes when it starts (``check_config``), the
   settings table gd-migrate converts ``conf.py`` with (``SETTINGS``, ``find_setting``,
   ``suggestion``), trust (``readable_roots``, ``confine_untrusted``), and ``Digest``.
gd-theme
   Built-in HTML and Typst templates (``layout.html``, ``article.html``, ``landing.html``,
   ``book.typ``, ``discussion.typ``, ``article.typ``, ``theme.typ``), CSS/JS assets, and
   deterministic ``.gdt`` theme archive packing and ``guidedog-theme.lock`` verification.
gd-migrate
   ``conf.py`` to ``conf.toml``, and ``guidedog quickstart``.
gd-env
   The foundation: build vocabulary (``BuilderKind``, ``BuildOptions``,
   ``BuildResult``, ``OutputFormat``, ``Asset``, ``Rendered``, ``UiText``), discovery
   (``discover``, ``SourceFile``), the environment (``Environment``, ``DocInfo``,
   ``ineligible_docs``), publication metadata and queries (``PageMeta``, ``PostlistSpec``),
   the object store (``ObjectStore``), lookups and traces (``Lookup``, ``LookupKind``,
   ``Trace``), the settings digests, and the build plugins of one build (``PluginRun``).
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
   numbers, ``only``, todo lists, ``postlist``, graphs) and renders the page.
gd-read
   Incremental reading: parses changed documents, extracts leading ``PageMeta`` fields
   and symbols, and hosts autodoc, odindoc and rustdoc.
gd-site, gd-book
   The builders: html, dirhtml and singlehtml (templates, taxonomy and collection
   archives, RSS/Atom feeds, ``sitemap.xml``, search index, static files); pdf (Typst
   books and standalone ``:pdf: article`` article PDFs) and text.
gd-intl, gd-i18n
   Catalogs, the interface translator, the gettext builder; the translation pass.
gd-project
   The coordinator and public surface: ``Project``, ``LoadOptions``, ``load``,
   ``build``, ``clean``, ``watch_roots``, ``build_limits`` and the trusted and
   untrusted policies.
gd-ssg
   The linked static publishing plugin (``guidedog ssg``): site presets, surgical
   metadata transitions, pinned ``.gdt`` theme installation, and mounted multi-project
   site assembly.
gd-cli
   The command line, and ``run_with(linked)`` for custom binaries.

Seams
-----

S1: gd-conf
~~~~~~~~~~~

``load_config(dir, &LoadOptions) -> Result<Loaded, Problem>`` with ``LoadOptions { conf_dir, no_config, overrides, source, output }`` and ``Loaded { config, warnings, settings }``. ``Config`` keeps the general settings flat (``project``, ``root_doc``, ``exclude_patterns``, ``numfig``, ...) and groups the rest: ``mathjax``, ``intersphinx``, ``autodoc``, ``odin_api``, ``rust_api``, ``plugins``, ``html``, ``pdf``, ``graphviz``, ``myst``, ``intl``, and ``site`` (``SiteConfig``: ``base_url``, ``sitemap``, ``feeds``, ``taxonomies``, ``page_size``, ``collections``, ``mounts``, ``assets``, ``theme``). The Sphinx-compatible conf.toml keys stay flat while static publishing lives under ``[site]``. ``ObjectTypeDecl``, ``AutodocDefaultOption`` and ``IntersphinxProject`` are gd-conf's own types; the crates that need them convert. Failures are ``gd_host::Problem`` (``config.invalid``, ``cli.define``, ``project.missing``, exit 2) and warnings ``gd_report::Report``.

S2: gd-env
~~~~~~~~~~

``DocId(u32)`` indexes ``Environment::docs`` (sorted by docname). ``Environment`` also persists ``ineligible_docs`` so documents withheld by ``:draft:``, ``:publish-at:`` or ``:expires-at:`` retain their cached ``DocInfo`` across clock-only builds without polluting active resolution tables. ``Digest`` is gd-conf's, re-exported. ``SourceFile { docname, path, relative, kind, generated }``; a generated page (an API page) carries its text and the files it was made from. ``DocInfo`` holds what the environment knows of a document, including ``page: PageMeta`` (extracted from the first root ``FieldList`` by ``page_metadata``), with ``Option`` for every "none" and a ``CachedFile { path, digest }`` for the object and the search words. ``PostlistSpec`` encodes and evaluates chronological article queries for ``postlist`` directives and collections. ``load_environment(path, settings, fresh, &budget)``, ``ENVIRONMENT_VERSION`` 29. ``ObjectStore`` writes ``objects/<docname>.<digest16>.gdo`` and loads it into a ``guidedoc::Workspace``. ``Lookup { kind: LookupKind, key }`` and ``Trace`` (each lookup once, including ``LookupKind::Template``, ``LookupKind::Postlist``, ``LookupKind::Navigation``, ``LookupKind::Excerpt``, and ``LookupKind::Mount``). ``Rendered { body, assets, trace, reports, failed }``. ``trait UiText: Sync { gettext, ngettext }``.

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

gd-read: ``read_documents(&ReadContext, previous: Environment, &[SourceFile]) -> Result<ReadOutcome, Problem>``, which fails only when the registry has no reader for a document (``build.reader``); a document that cannot be read is a failed ``DocInfo`` in the outcome. ``api_sources`` makes the generated Odin and Rust API pages and ``reread_document`` reads a document again whose object is stale. The autodoc, odindoc and rustdoc hosts implement guidedoc-sphinx's provider trait. gd-site: loads templates through ``gd_jinja::Environment`` with a confined loader (``templates_path``, an offline-verified ``_themes/<tree-sha256>/`` when ``site.theme = true``, then the built-in ``gd-theme`` assets), records fine-grained ``Template``, ``Navigation`` and ``Postlist`` lookups per page, and emits taxonomy and ``[[site.collections]]`` archive units, RSS 2.0 and Atom 1.0 feeds, and ``sitemap.xml``. gd-book: composes Typst books from ``pdf_documents`` and standalone article PDFs for eligible documents with ``:pdf: article`` (using ``_templates/article.typ`` or ``gd_theme::article_template``).

S8: gd-project
~~~~~~~~~~~~~~

``load(dir, &LoadOptions { config, plugins, registry }) -> Result<Project, Problem>``
checks the engine registry (``core.registry.invalid``; ``None`` is
``guidedoc_defaults::registry()``) and reads ``conf.toml``. ``Project { config,
warnings, settings, registry, plugins }`` is plain data. ``build(&project, kind,
&options) -> BuildResult`` never fails outright; it runs, in order:

1. ``confine_untrusted`` on its own copy of the configuration (``--untrusted``); ``output_paths`` and ``Generation::begin``, which settles an interrupted commit first; ``build_limits`` (``build.stack``); ``PluginRun::start``.
2. ``discover``, ``contents_root``, ``api_sources`` and ``with_generated``: the sources.
3. ``load_intl`` and, except for gettext, ``localize_config``; ``build_dates`` (from ``options.now`` or the clock); ``load_intersphinx`` (no network when untrusted). The build stage becomes Reading.
4. ``build_settings`` (reading settings, translations, trust, limits, plugins); ``load_environment`` and ``read_documents``; the documents' reports join the build's. Unless ``options.preview`` is set, the publication clock filter partitions ``env.docs`` into eligible ``docs`` and withheld ``ineligible_docs`` against ``now``.
5. ``resolve``; the stage becomes Writing. A ``BuildView`` with ``output_settings``, and one ``match`` on ``BuilderKind``: ``SiteBuilder`` (html, dirhtml, singlehtml), ``PdfBuilder``, ``TextBuilder``, ``DummyBuilder``, ``GettextBuilder`` (with ``reread_document`` as its ``Reread``).
6. ``suppress_warnings``; then a build with an unread document (``build.read``) or, with ``-W``, any warning or error (``build.warnings``) abandons its generation; any other commits it. ``published`` and ``unsettled`` come from the commit.

``BuildResult::written`` and ``unchanged`` are the generation's units written and kept
besides the site unit (``Generation::written_count``, ``reused_count``); a builder whose
whole output is the site unit (singlehtml, gettext) counts it as one output.

Ownership
---------

No arena holds a project or a build. ``load`` returns an owned ``Project``; a build
takes ``&Project`` and clones the configuration, so the type system enforces that a
build never changes its project (an ``--untrusted`` build confines only its own copy).
Only the coarse ``gd_host::Budget`` remains, for the files and sessions a build holds,
with ``--budget`` hints; ``BuildOptions::on_budget`` decides at the budget (a terminal
asks whether to raise it). Pages render on ``gd_host::run_workers``: each worker owns a
``guidedoc::Workspace`` and returns a ``Rendered``; the main thread emits units in
docname order.
