PDF theme review
=================

On 16 October 2026 we reviewed Guidedog's default PDF templates for books and discussion records. The layout examples supplied for the review were visual references. Guidedog's templates retain its own metadata contract, vocabulary, palette and project configuration. They have no dependency on another project's files, branding or document claims.

A shared theme
---------------

``theme.typ`` defines Guidedog teal, dark ink, neutral panels, font defaults and a language-aware content wrapper. ``book.typ`` and ``discussion.typ`` import that module with ordinary Typst imports. A wrapper receives the content it styles; a helper that merely executes local ``set`` rules cannot style its caller's later content.

Books use centered covers, teal chapter openings and balanced margins. The manual keeps its localized titles and regional CJK and Devanagari fonts. Discussion records use the catalog name and prefix from metadata, a panel of readable fields, state badges, a short status note and compact contents. They do not assume a particular catalog lifecycle or invent an intended status. Unknown fields and custom state names remain visible. The source title and section anchors remain part of the document.

The binary embeds all three modules. Quickstart copies all three, while discussion initialization copies the discussion layout and shared theme. Built-in fallback templates inline the shared module, so they need no project files. Untrusted builds cannot execute a project's theme through a built-in fallback. Existing copied templates remain project-owned; the :doc:`../templates` guide explains how we adopt updated defaults without overwriting customizations.

Imported modules and cached PDFs
---------------------------------

The review found that a PDF's input set previously recorded its entry template but not the modules that template imported. A cached PDF could therefore survive a change to a shared module. This was a dependency-tracking problem, independent of the visual layout.

The builder now records readable files and directory membership beneath ``_templates/``. Editing, adding or removing a nested module invalidates the PDF. Canonical folder identities stop link cycles, confinement keeps discovery inside readable roots, and a limit of 16,384 inputs bounds the scan. An unreadable template folder or an exceeded limit fails composition with a diagnostic. Untrusted builds skip project template discovery.

Regression checks build an actual PDF with an imported nested module, keep the unchanged PDF, rebuild it after a same-length module edit, and reject a missing module instead of reusing stale output. Separate checks edit the shared theme, install new catalogs, exercise copied and embedded layouts, and refuse project code in untrusted builds. Long captioned tables and listings retain all 120 entries across pages, with repeated table headers.

Visual review
--------------

We rendered the Guidedog manual and the complete discussion catalog. We inspected the book cover, chapter opening, discussion panel, state badge, contents, running page furniture and section bookmarks. We also inspected later pages with code and tables. The layout uses the bundled Libertinus families and requires no remote template package or project-specific asset.
