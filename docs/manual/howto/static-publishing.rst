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
Recognized publication fields supply layout data and are omitted from body rendering and search text.
Unknown metadata fields and ordinary later field lists remain content.
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

We generate archives and a paginated journal with the same publication gate:

.. code-block:: toml

   [site]
   base_url = "https://example.org/project"
   sitemap = true
   taxonomies = ["tags", "category", "year", "authors", "series"]
   page_size = 10

   [[site.collections]]
   output = "journal"
   title = "Journal"
   tags = ["rust"]
   page_size = 5
   feeds = true

The journal starts at ``journal/index.html``.
Later pages use ``journal/page/2/index.html``, with previous and next links.
A zero page size keeps the collection on one page.
The collection accepts ``path``, ``category``, ``tags``, ``author``, ``series`` and ``year`` filters.
Its feeds live at ``journal/rss.xml`` and ``journal/atom.xml`` and contain the entire selected collection.
Its HTML layout defaults to ``collection.html``; ``template`` selects an override.
Templates receive the current page number, page count and selected posts in ``collection``.
``collection.previous_url`` and ``collection.next_url`` are relative pagination links or ``None``.
Each post supplies ``title``, ``docname``, ``date``, ``url``, ``summary`` and typed ``meta`` values, including its optional ``pdf_url``.

Taxonomy archives use paths such as ``tags/rust/index.html`` and ``year/2026/index.html``.
Labels retain their spelling in headings.
Paths escape uppercase, punctuation and non-ASCII bytes, so distinct labels remain distinct on case-insensitive filesystems.
Generated outputs cannot replace an author-written page or another output.
When publication state or collection membership shrinks, the output transaction removes obsolete archives and pagination pages.

Article templates receive ``page.older`` and ``page.newer`` in publication order, with stable docname ties.
``page.taxonomies`` maps each enabled taxonomy to entries with ``name`` and relative ``url`` fields.
``page.word_count`` and ``page.reading_minutes`` estimate visible reading length using ``myst_words_per_minute``; CJK characters count individually.

Starter sites
-------------

We create a new site with one of three complete presets::

   guidedog ssg init project --preset project --title "Our project"
   guidedog ssg init journal --preset journal --title "Our journal"
   guidedog ssg init research --preset research --title "Our papers"
   guidedog ssg build journal

The project preset starts with a landing page and a documentation guide.
The journal preset starts with a dated article, chronological cards, feeds and taxonomy archives.
The research preset starts with a paper containing an abstract and mathematics, and builds its standalone PDF.
All presets copy editable HTML, CSS and Typst theme files.
Initialization refuses an existing destination.
We replace the example public URL in ``conf.toml`` before publishing feeds or sitemaps.

``ssg build`` builds a single project when no mounts are configured.
Its default output is ``_build/site``.
``--out DIR`` selects another destination, ``--now RFC3339`` fixes the publication clock, and ``--preview`` includes withheld documents.

Mounted projects and editions
-----------------------------

We assemble several existing projects with a site-level ``conf.toml``:

.. code-block:: toml

   project = "Our website"

   [site]
   base_url = "https://example.org/project"

   [[site.mounts]]
   name = "home"
   source = "pages"
   output = ""

   [[site.mounts]]
   name = "manual"
   source = "manual"
   output = "manual"
   builder = "html"
   overrides = ['html_extra_path=[]']
   editions = [
     {language = "en", output = ""},
     {language = "de", output = "de"},
     {language = "ja", output = "ja", source = "manual-ja"},
   ]

   [[site.assets]]
   source = "shared"
   output = "assets"

Each source is a project directory beneath the site configuration's directory, after resolving symbolic links.
An edition uses gettext in the mount's source project unless ``source`` selects a separate translated project.
Edition ``output`` is relative to the mount's public output directory.
An empty output mounts at the website root.
Named mounts and edition output directories must be distinct when compared without ASCII case.
Different mounts may share a directory when their actual files do not overlap.
The assembly rejects file collisions, file/directory overlaps and symlinks in copied output.
It publishes every mount and shared asset in one output transaction.
A later project failure preserves the preceding complete website.

Mount builders are ``html``, ``dirhtml``, ``pdf`` and ``discussion-pdf``.
PDF mounts copy PDF outputs; ``discussion-pdf`` builds the active records of a discussion catalog, with ``prefix`` selecting its identity.
``exclude`` supplies globs over a mount's relative output files.
``overrides`` accepts setting assignments as ``-D`` does; an edition's overrides follow the mount's overrides.
We select languages through ``editions.language`` rather than a ``language`` override, so routing and translated output cannot disagree.
Values can contain ``{language}``, ``{edition}``, ``{site_root}`` and ``{base_url}`` placeholders.
The root configuration's nonempty ``version`` and ``release`` values apply to every mount.
The public base URL gains each mount and edition's prefix.
One publication clock applies to all projects in the build.

An explicit link addresses a file in another named mount:

.. code-block:: rst

   `The manual <mount://manual/index.html>`_

The link selects the target mount's matching language edition, then its default edition.
HTML receives a relative link, including when the whole website lives below a hosting prefix.
PDFs use the public site URL when one is configured.
The target file must belong to the current assembled output; a stale file cannot satisfy a link to a withheld document.

``--no-pdf`` skips PDF mounts for an HTML preview and removes cross-mount PDF download links.
Custom templates can use ``ssg_no_pdf`` to suppress their own PDF controls.
The repository's ``docs/conf.toml`` uses these contracts for the landing pages, seven manual editions, seven PDF books and discussion PDFs.
``cargo xtask build-site`` delegates to this same command.

Pinned themes
-------------

We package a directory containing ``_templates/`` and ``_static/``::

   guidedog ssg theme pack theme-source --out theme.gdt
   sha256sum theme.gdt
   guidedog ssg theme install https://example.org/theme.gdt --sha256 HASH --project journal
   guidedog ssg theme verify journal

We replace ``HASH`` with the archive's actual SHA-256.
Installation requires that explicit pin and verifies it before writing theme files.
We enable the installed theme with ``theme = true`` in the project's ``[site]`` table.
Project template directories take precedence over the installed theme.
Project static files can override its static files.

The version-one archive is UTF-8 JSON with ``format`` set to ``1`` and a ``files`` object mapping relative paths to base64 file bytes.
Only portable files beneath ``_templates/`` and ``_static/`` are accepted.
Traversal, case aliases, reserved platform names, symlinks and file/directory overlaps are refused.
An archive contains at most 4,096 files and 64 MiB of expanded data.
Its directory depth is at most 32 levels.
It carries no executable installation hooks.

``guidedog-theme.lock`` records the source, archive SHA-256 and canonical tree SHA-256.
The tree checksum hashes the versioned domain tag, sorted filenames and raw file bytes, with lengths delimiting names and contents.
The installed tree lives beneath ``_themes/TREE_SHA256``.
Installation finishes and syncs that tree before atomically replacing the lock.
A failed pin leaves the preceding selected theme intact.

Builds never download themes.
They verify the locked tree offline and fail if a file is missing, changed, added or replaced by a symlink.
We explicitly reinstall a pinned archive to restore a missing installation or select another version.
