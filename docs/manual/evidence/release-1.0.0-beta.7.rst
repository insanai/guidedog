Release 1.0.0-beta.7
====================

On 17 October 2026 we published `v1.0.0-beta.7 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.7>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``398b7b1c7b044dd5901ed89228df2df8cdd5364f``.

Rich article excerpts
---------------------

This release fixes `issue 3 <https://github.com/insanai/guidedog/issues/3>`_.
The built-in reader recognizes ``.. more::`` as a parsed boundary, diagnoses duplicate and nested boundaries, and leaves literal code examples untouched.
Full HTML and PDF articles retain both sides of the boundary.
Native postlists and collection templates render the selected introduction during the build.
``post.excerpt_html`` preserves ordinary document formatting and uses the plain-text summary fallback when no boundary is present.
Feeds continue to use the independent summary.

The shared link pass resolves each excerpt in its source article's context.
A second AST pass relocates links and namespaces anchors for its destination listing.
Foreign AST copies preserve local links and footnote backlinks; excluded targets link to the full article.
Cross-mount paths retain leading parent components.
Source-scoped lookup traces and cached object digests keep listings current after changes to introductions, included files and referenced titles.
Draft, scheduling and expiry filters apply before excerpt selection.

The `regression suite <https://github.com/insanai/guidedog/blob/v1.0.0-beta.7/crates/gd-project/tests/ssg_excerpts.rs>`_ covers HTML, dirhtml, singlehtml, standalone article PDFs and PDF books.
It checks nested archives and pagination, resource and anchor targets, repeated listings, local and excluded footnotes, literal examples, invalid boundaries, publication filtering and incremental changes.
The :doc:`../howto/static-publishing` guide documents authoring, templates and Tinkerer migration.
All six translated manual catalogs have zero fuzzy or untranslated messages.

Qualification
-------------

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/39059010321>`_ passed on Linux x86-64 and macOS Apple silicon.
Each workspace suite passed 1,966 tests, with five ignored.
Both runners separately passed the ignored 1,000-article regression, examples and program smoke tests.
The Linux runner also passed the real Secret Service check.
Formatting, Clippy with warnings denied, compilation, all nine generated tables and source limits passed.
The source checker examined 1,107 files with no violations and six existing soft-limit warnings.

The large-site check reused all 1,102 units on an unchanged build.
A summary-only article body edit reparsed one document and rewrote only its HTML and the search index, retaining 1,100 units.
Changing an unused template rewrote nothing.
Explicit rich excerpts conservatively depend on the complete cached source object, including edits after the boundary.

The `Website run <https://github.com/insanai/guidedog/actions/runs/39059010275>`_ built and deployed all seven language editions, their manual PDF books and the discussion PDFs.
It checked 789 HTML pages and 175,473 internal links with no missing files or anchors.
We inspected the generated release collection, the standalone release PDF and the manual's excerpt examples in PDF.
The live collection renders the formatted introduction without the article tail.

Published archives
------------------

The `candidate run <https://github.com/insanai/guidedog/actions/runs/39059662377>`_ qualified the five native platform archives before tagging.
All 16 jobs of the `release run <https://github.com/insanai/guidedog/actions/runs/39061199616>`_ passed.
The fresh-runner archive tests used an empty ``PATH`` and exercised the complete SSG scenario, including rich excerpts, cross-mount links, taxonomy, pagination, feeds, pinned themes and article PDFs.
Discussion HTML/PDF and offline Pages checks also passed.
Homebrew formula installation and testing passed before publication.

We downloaded all ten published assets and verified their GitHub API digests, including ``SHA256SUMS`` itself.
All nine entries in that checksum manifest match.
The four Unix archives have matching beta.7 versions, the tagged commit and verified executable hashes.
The Linux archive's GitHub provenance attestation verifies the tagged workflow and commit.
Its published executable passed the full SSG, discussion and offline Pages smoke tests outside the checkout.
The source archive includes the excerpt implementation, regression tests and manual, has the correct version and contains no agent instruction files.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   a5ac102adada94e9919c3b0a0c18a162f6a470d023f0da0cf1db8c560c52a5dd

Homebrew and documentation
--------------------------

The tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/39062331354>`_ verified the published assets and installed and tested the formula on Apple silicon and Intel macOS.
It committed ``d9f0053f3dc8411e52a710bf7f302ed977ebddc5``.
The tracked formula matches the published ``guidedog.rb``.
The workflow example in :doc:`../howto/github-pages` now pins beta.7 and its verified published Linux checksum.
The final documentation deployment includes this evidence record.
