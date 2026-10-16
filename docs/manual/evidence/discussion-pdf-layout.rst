Discussion PDF layout review
============================

The PDF composer previously selected ``book.typ`` for every root document. Discussion records therefore acquired a cover, colophon, contents and chapter-opening pages. Their metadata appeared as an ordinary field list in the body.

Standalone records now select a dedicated RFC-style layout. The composer and link pass share the same record recognition: a leading metadata list must contain a number and title in its catalog's namespace. The template receives quoted metadata, keeps the source title and its anchor, and starts the discussion on the first page. Ordinary books and discussion chapters inside a larger book retain their book behavior.

We built all 12 active records in Guidedog's GDS catalog and visually reviewed the process record and GDS 0012. The latter changes from seven pages in the previous site output to two pages with the same source. Its first page contains the identifier, title, readable metadata and abstract. Later pages carry the catalog and record identifiers, with page numbers and totals. PDF bookmarks follow the visible section numbering.

Regression checks inspect actual laid-out pages. A short record fits on one page with its metadata and abstract. Separate checks cover copied discussion templates, existing book templates, cache invalidation, untrusted fallback, plain books and partial metadata. Long captioned tables and listings preserve all 120 entries across pages with both built-in and copied discussion templates; table headers repeat. End-to-end CLI checks cover full-catalog builds and selected-record previews.

The binary embeds the discussion layout through ``gd-theme``. Catalog initialization copies that same source into ``_templates/discussion.typ`` as part of the staged catalog transaction. An existing catalog without a copy uses the embedded layout. Initialization tests compare the installed template with the embedded text, and every platform's release smoke test builds a new catalog and verifies that its generated PDF source imports the discussion template with structured record metadata.

The :doc:`../howto/discussions` guide describes the layout and its customization point.
