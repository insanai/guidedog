:gds-number: 0013
:gds-title: Static Website and Article Publishing in Guidedog
:gds-state: discussion
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-10-16
:gds-updated: 2026-10-16
:gds-discussion: Local review; no external discussion URL assigned
:gds-labels: ["architecture", "build", "theme", "typst", "cli"]

Static Website and Article Publishing in Guidedog
================================================

Abstract
========

We publish pages and dated articles through Guidedog's existing document pipeline.
We read publication metadata from the first root field list, apply a build-clock gate before resolution, and render eligible documents through project-selected HTML layouts.
We express collections with ``.. postlist::`` and compile optional standalone article PDFs with the shared Typst theme.
The ``guidedog ssg`` commands edit publication state in source files.

This record separates the implemented publication foundation from proposed website orchestration.
Version 1.0.0-beta.6 introduces the foundation described below.
Declarative mounts, generated taxonomies, pagination, remote theme packs and distinct starter presets remain proposed work.
They are not supported configuration in this release.

Review of the initial proposal
==============================

We retain the publishing model and correct several assumptions about the implementation.

* Guidedog is a Rust workspace with ``conf.toml`` projects.
  Introducing a competing ``guidedog.toml`` format would duplicate project discovery, overrides, migration and validation.
  We extend ``conf.toml`` instead.
* ``gd-jinja`` is Guidedog's own template engine.
  We extend its existing loader and renderer rather than introducing MiniJinja.
* ``gd-env`` stores sorted ``Vec<DocInfo>`` values and writes explicit JSON records with ``gd-json``.
  We preserve that representation and bump its cache format from 27 to 28.
  We do not introduce serde derives or a parallel document map.
* A plain body edit already leaves unrelated pages cached when navigation does not change.
  ``document_settings`` hashes titles and navigation, not every source body.
  We retain conservative navigation invalidation because templates can call ``hasdoc`` and ``toctree`` dynamically.
  Removing those dependencies without tracing the template functions would produce stale pages.
* File size and modification time do not establish content equality.
  An editor, restored archive or copied file can preserve both while changing the bytes.
  We retain content hashing.
* A complete build still discovers and verifies inputs across the project.
  Parsing and HTML rendering can be limited to one changed document, but total build work is not unconditionally :math:`O(1)`.
  We specify observable rebuild counts instead of an unmeasured 50 ms claim.
* MyST front matter exposes values through the reader's field-list contract.
  Boolean values appear as ``True`` and ``False`` and sequence values appear as JSON text.
  Publication metadata accepts those representations as well as RST scalar fields.
  MyST does not currently give every YAML key an exact source span.
* The proposed Typst listing used palette names, font return types and function arguments absent from ``theme.typ``.
  The implemented article template uses the actual shared palette and ``configure-document`` wrapper.
* A single source update needs one atomic replacement.
  A multi-file journal is appropriate for a catalog-wide transaction, but unnecessary for replacing one article's metadata.
* Existing builds may fetch configured inventories or Typst packages.
  This change adds no build-time theme fetching, but does not claim that all existing build modes are offline or that custom templates cannot read permitted project files.

Authoring
=========

We create, schedule, publish and withhold articles with ordinary source files::

   guidedog ssg new notes.rst --title "Release notes"
   guidedog ssg schedule notes.rst --at 2026-10-20T09:00:00Z
   guidedog ssg status
   guidedog build html --now=2026-10-20T09:00:00Z
   guidedog ssg draft notes.rst
   guidedog build html --preview

We may use ``.md`` instead of ``.rst``.
New Markdown articles use YAML front matter.
The authoring clock honors ``SOURCE_DATE_EPOCH`` and may be fixed with ``--now``.
All generated authoring dates use UTC.
Builds retain the existing ``SOURCE_DATE_EPOCH`` behavior and accept an explicit RFC 3339 ``--now`` override.

``new`` refuses an existing destination.
``draft`` sets the draft flag without discarding publication history.
``schedule`` clears the draft flag, sets ``publish-at``, and clears an old expiration.
``publish`` clears draft, schedule and expiration.
We preserve an existing past publication date unless ``--date`` replaces it.
We replace a missing or future date with the current UTC date so that publishing takes effect immediately.
An explicit future publish date is rejected; we use ``schedule`` for that case.
The commands never commit, tag or move source files.

We validate metadata with the real readers before writing.
Edits retain the body bytes, UTF-8 BOM, line endings and file permissions.
Duplicate publication fields and unclosed front matter are refused.
A directory lock serializes publication commands.
A synced temporary file and atomic rename replace an existing source.
New sources use an exclusive hard link from the completed temporary file so an existing destination cannot be overwritten.
A crash can leave an unused temporary file, but cannot expose a half-written source.

Metadata
========

A leading RST field list or Markdown front matter block carries page metadata:

.. code-block:: rst

   :date: 2026-10-16
   :publish-at: 2026-10-20T09:00:00Z
   :draft: false
   :author: Ada Lovelace
   :tags: architecture, rust
   :category: engineering
   :summary: A short abstract for collections and feeds.
   :pdf: article

   Publishing a document
   =====================

   Article body.

The recognized fields are:

=================== ==============================================================
Field               Meaning
=================== ==============================================================
``template``        HTML layout override.
``date``            Publication date or RFC 3339 instant.
``updated``         Significant revision date or RFC 3339 instant.
``publish-at``      Scheduled release instant, overriding ``date``.
``expires-at``      Exclusive end of publication, as an RFC 3339 instant.
``draft``           Boolean publication flag, false by default.
``author``          One author; repeated fields preserve source order.
``authors``         Comma-separated authors or a Markdown YAML sequence.
``tags``            Comma-separated tags or a Markdown YAML sequence.
``category``        Primary category.
``series``          Series name.
``summary``         Plain-text abstract.
``pdf``             ``article`` enables the standalone PDF; absent or ``none`` disables it.
``nosidebar``       Boolean flag hiding the built-in sidebar.
=================== ==============================================================

Unknown metadata fields remain available to other document consumers.
Only the first root field list supplies publication metadata.
Later examples and nested field lists cannot change publication state.
A dated document is implicitly an orphan for toctree validation.

We validate Gregorian dates, time zones and fractional seconds instead of comparing date strings.
Date-only publication values mean midnight UTC.
Instants accept numeric offsets and up to nine fractional digits.
We reject leap seconds and the unknown offset ``-00:00``.
Invalid publication metadata prevents an output commit rather than silently publishing a page.

Publication model
=================

Let :math:`r(x)` be ``publish-at``, falling back to ``date`` when no schedule exists.
Let :math:`e(x)` be the optional expiration instant.
For build clock :math:`t`, release eligibility is

.. math::

   \operatorname{eligible}(x,t) =
   \neg\operatorname{draft}(x)
   \land (r(x)=\bot \lor r(x)\le t)
   \land (e(x)=\bot \lor t<e(x)).

Preview builds bypass this gate.
A preview output directory can therefore contain unpublished content.
We use a separate output directory when retaining both public and preview artifacts.

.. graphviz::
   :caption: Publication filtering precedes every public index and renderer.
   :alt: Source files are read into a private cache, filtered by the build clock, and then resolved and rendered.
   :class: graphviz-adaptive

   digraph publication {
     rankdir=LR;
     source [label="Sources"];
     cache [label="Private environment cache"];
     gate [label="Publication gate"];
     output [label="Resolution, HTML, PDF, search, feeds"];
     source -> cache -> gate -> output;
   }

We retain withheld ``DocInfo`` records in ``Environment::ineligible_docs`` for reuse.
We merge them into the previous reading before verifying source digests.
After reading, we partition the environment before ``gd-resolve`` assigns document ids or builds symbol tables.
Only ``env.docs`` reaches navigation, search, inventory, source downloads, collections, feeds and renderers.
Cache commit and sweeping retain both eligible and withheld objects.
A subsequent clock-only build can publish a scheduled article without parsing it again.
A successful rebuild removes artifacts no longer owned by an eligible output unit, including old article PDFs.
A failed build retains the previous successful output under the existing transaction contract.

A reference to a withheld document or label reports ``link.unpublished_target``.
The diagnostic does not repeat the unpublished title or summary.
The gate does not inspect arbitrary raw HTML, manually copied extra files or content an author deliberately includes in a public document.
Those remain explicit publication inputs.

Collections and discovery
========================

We insert a chronological collection in any RST or MyST document:

.. code-block:: rst

   .. postlist:: 10
      :path: notes/*
      :category: engineering
      :tags: rust
      :author: Ada Lovelace
      :series: compiler
      :sort: -date
      :style: cards
      :excerpts:

The optional argument limits the result count; zero or absent means unlimited.
Path filters use Guidedog's existing docname glob rules.
Category, author and series filters match exactly.
Every requested tag must match.
We sort by the parsed publication instant, newest first by default, with docname as the stable tie breaker.
``:sort: date`` selects oldest first.
``list`` and ``cards`` use the same semantic list structure with different CSS classes.
``:excerpts:`` adds plain-text summaries.
The link pass produces ordinary list and link nodes, so collection output works through the existing renderers.

We enable deterministic discovery files in ``conf.toml``:

.. code-block:: toml

   [site]
   base_url = "https://example.org/project"
   sitemap = true
   feeds = true

The HTML and dirhtml builders emit ``sitemap.xml``, ``atom.xml`` and ``rss.xml`` in the site unit.
The base URL includes any hosting prefix.
Feeds contain eligible dated documents, use source metadata instead of the wall clock, and escape text and URLs.
Sitemaps contain eligible document pages.
The feature is opt-in, so projects without ``[site]`` acquire no discovery files.

Templates and article PDFs
=========================

The default HTML template is ``page.html``, which extends the existing ``layout.html``.
Dated documents select ``article.html``.
A source can select ``landing.html`` or any project template with ``:template:``.
Project overrides remain in the existing ``templates_path`` directories.
Built-in names retain the ``!`` override escape used by Sphinx-compatible layouts.
Templates receive publication values in ``page.meta``.
Values are ordinary escaped text and typed booleans or lists, not executable markup.

An eligible ``:pdf: article`` document also produces a PDF beside its HTML page.
For ``notes.rst`` with the HTML builder, the files are ``notes.html`` and ``notes.pdf``.
For dirhtml, they are ``notes/index.html`` and ``notes/index.pdf``.
The generated Typst remains available under ``sources/`` for inspection.
The article template uses continuous prose, a title and author block, an abstract, and running headers and page numbers after page one.
It shares typography and directive support with the book and discussion layouts.
Project ``_templates/article.typ`` overrides use the existing ``book`` function convention with article metadata arguments.
Untrusted builds use the embedded article template.

Incremental correctness
=======================

``LookupKind`` remains a small enum; each lookup has a separate string key.
We add ``Template`` and ``Postlist`` kinds instead of embedding owned strings in the enum.
A postlist key contains its complete encoded query, avoiding query-hash collisions.
Its answer hashes the ordered selected docnames, titles and page metadata.
A body edit therefore does not invalidate a summary-only collection.
A membership, ordering, title or metadata change does.

Template dependency tracing happens inside ``gd-jinja::Environment::load_compiled``.
It records cached requests, transitive inheritance, imports, dynamic includes and unsuccessful optional includes.
We compare effective sources through the same confined loader before reusing a page.
Adding a project override or creating an optional include therefore invalidates the affected pages.
Changing an unused template does not invalidate document pages.
We retain the global navigation digest until template function calls and every navigation context value have their own complete dependency model.

Acceptance evidence
===================

We require tests for both RST and MyST authoring, exact release and expiration boundaries, offset and fractional timestamps, invalid metadata, cache round trips, and output cleanup.
Clock-only changes must reuse parsed objects.
A plain body edit must parse one document and emit only its changed HTML page when navigation and metadata are unchanged.
Changing an unused template must emit no document pages.
A standalone article must compile to a real PDF through the embedded Typst compiler.
The existing workspace tests, Clippy, source limits and full website build remain release gates.

Remaining proposals
===================

The following work needs separate implementation and acceptance evidence before this record can be considered complete:

* Declarative mounts and language editions, including path overlap checks, cross-mount links, shared assets, atomic whole-site publication and translation overrides.
* Replacement of repository-specific website assembly in ``tools/xtask/src/site.rs`` after those contracts can express the existing seven-language website.
* Generated tag, category and year archives with a collision policy for author-written pages.
* Collection pagination with stable output ownership and navigation.
* Per-collection feed destinations and filters.
* Remote theme installation with a defined archive format, canonical tree checksum, symlink policy, lockfile and offline verification.
* Distinct project, journal and research presets whose output has been reviewed.
* Reading time, chronological neighbor links and taxonomy URLs in template context.
* Fine-grained navigation tracing and a measured large-project benchmark.

We do not install incomplete command stubs or silently accept configuration for those features.
Version-control actions remain outside publication commands, and per-article PDFs remain opt-in.

References
==========

* ``docs/manual/howto/static-publishing.rst`` documents the supported authoring flow.
* ``docs/manual/internals/incremental.rst`` specifies cache inputs and output ownership.
* ``crates/gd-env/src/page.rs`` and ``page_fields.rs`` define publication metadata.
* ``crates/gd-jinja/src/environment.rs`` records template requests during rendering.
* ``crates/gd-project/tests/ssg.rs`` exercises clock changes, cache reuse and artifact cleanup.

Review history
==============

* 2026-10-16: Initial discussion draft.
* 2026-10-16: Reviewed against the Rust workspace and revised around existing configuration, cache records, template tracing, source transactions and measured acceptance evidence.

.. rubric:: Lifecycle event

::

   2026-10-16: prediscussion -> discussion. Assigned a permanent number and opened for discussion.
