Static websites and articles
========================================

We publish dated articles and standalone pages through the normal HTML builder.
We use ``guidedog ssg`` to edit publication state in source files::

   guidedog ssg new notes.rst --title "Release notes"
   guidedog ssg schedule notes.rst --at 2026-10-20T09:00:00Z
   guidedog ssg status
   guidedog build html --now=2026-10-20T09:00:00Z

We may use a ``.md`` path to create Markdown with YAML front matter.
For Markdown builds, we include ``.md`` in ``source_suffix`` and enable ``myst_parser`` in ``extensions`` as usual.
``new`` creates a draft and refuses to overwrite an existing file.
``publish`` clears draft, schedule and expiration, retaining an existing past date.
``draft`` withholds a source from subsequent release builds.
These commands leave version control to us.

We describe an article in its leading metadata:

.. code-block:: rst

   :date: 2026-10-16
   :author: Ada Lovelace
   :tags: rust, publishing
   :summary: A short abstract.
   :pdf: article

   Release notes
   =============

   Article body.

Date-only publication starts at midnight UTC.
``publish-at`` overrides that boundary with an RFC 3339 instant.
``expires-at`` ends publication at an exclusive instant.
Drafts, future articles and expired articles are excluded before navigation, search, feeds and source downloads are built.
Publication metadata is source configuration and is not translated by gettext catalogs.
We change metadata for a separate language edition in that edition's source files.
Invalid publication metadata stops the output commit.
A failed build retains the previous successful website.

We inspect unpublished pages with ``guidedog build html --preview``.
For separate public and preview artifacts, we supply separate output folders with the sphinx-build form::

   guidedog build -b html . preview-output --preview

We collect eligible articles in any page:

.. code-block:: rst

   .. postlist:: 10
      :path: notes/*
      :tags: rust
      :sort: -date
      :excerpts:

The path matches docnames without a source suffix.
Category, author and series filters match exactly, and every listed tag must match.
Articles are ordered by publication time, with docname breaking ties.
``:sort: date`` places the oldest first.
``:style: cards`` provides a CSS class for project styling.

We enable discovery files in ``conf.toml``:

.. code-block:: toml

   [site]
   base_url = "https://example.org/project"
   sitemap = true
   feeds = true

The HTML and dirhtml builders emit ``sitemap.xml``, ``rss.xml`` and ``atom.xml``.
The URL includes the hosting prefix.
We receive the same discovery bytes for the same eligible sources and metadata.

Dated articles use ``article.html`` by default.
Other pages use ``page.html``, which extends the existing ``layout.html``.
We select a project template with ``:template: landing.html`` or another name in ``templates_path``.
Templates receive dates, authors, tags, category, series, summary, draft state and PDF links through ``page.meta``.
``:nosidebar: true`` suppresses the built-in navigation sidebar.

``:pdf: article`` emits a standalone PDF next to the article's HTML page.
The layout includes the title, authors, publication date, abstract, and running headers after page one.
We override it with ``_templates/article.typ`` using the built-in template's ``book`` function signature.
The generated Typst source is retained under ``sources/`` in the output folder.
Article PDFs are supported by the HTML and dirhtml builders; singlehtml retains its existing single-page behavior.

Declarative mounts, automatic taxonomy pages, pagination and remote theme installation remain proposed work in GDS 0013.
The supported configuration rejects those keys until their contracts and implementations are ready.
