.. _documentation-verification--1-october-2026:

Documentation verification — 1 October 2026
===========================================

This is the historical documentation build record. The :doc:`11 October beta
review <beta-review-20261011>` records later fixes, fresh verification, permanent GDS
numbers, and lifecycle decisions.

The manual and GDS are separate Guidedog projects. Each was created through
``guidedog quickstart``. Their source of truth is reStructuredText. Typst remains the PDF
typesetting template and an explicitly supported input format. The API reference is a third
project, generated from Odin source without executing that source.

This record describes observations from the builds and tests. It does not declare the GDS
proposals accepted. Their numbers, states, dates, and review history remain intact.
Discussion records and drafts remain discussions and drafts.

Published material
------------------

+---------+-------------------------------+------------+-------------------------------+
| Project | Sources                       | HTML pages | PDF                           |
+=========+===============================+============+===============================+
| Manual  | 35 authored + 27 generated    | 64         | One 361-page book with the    |
|         | API documents                 |            | complete API                  |
+---------+-------------------------------+------------+-------------------------------+
| GDS     | 14                            | 16         | Six record books: 15, 18, 29, |
|         |                               |            | 14, 9, and 20 pages           |
+---------+-------------------------------+------------+-------------------------------+
| API     | 28                            | 30         | One 309-page reference        |
+---------+-------------------------------+------------+-------------------------------+

HTML page counts include generated navigation and search pages. The GDS project also
preserves the earlier ZDS process as an explicitly historical RST document. The separate
guide project has been removed. Its evidence is retained here. There are exactly three
documentation projects: manual, API, and GDS. The manual generates its API appendix from the
same Odin sources as the API project. Its references link to that appendix inside the
combined PDF.

The manual separates tutorials, topic guides, practical tasks, reference, and internals. It
includes equations, a small proof, resource and publication algorithms, memory ownership,
and static Graphviz diagrams. The HTML has a reading column, project navigation, page
outline, search, and a narrow-screen navigation drawer. Light is the initial theme; the
reader may choose dark or system mode.

The web and PDF editions bundle Libertinus Serif and Libertinus Sans. The PDF uses a
two-sided A4 book layout, embedded fonts, bold heading hierarchy, optimized line breaking
and language-aware hyphenation, running heads, numbered figures and equations, and a linked
table of contents. Long identifiers and code have their own wrapping rules. This is a
readable print-oriented PDF, not a claim of PDF/X certification.

Features exercised
------------------

+----------------------------------+------------------------------------------------------+
| Feature                          | Evidence                                             |
+==================================+======================================================+
| Project creation and tutorials   | Integration tests extract the actual tutorial RST    |
|                                  | blocks, create a project with the CLI, and build its |
|                                  | mathematics, cross-references, diagram, and book.    |
+----------------------------------+------------------------------------------------------+
| Builders                         | The authored tutorial fixture passes strict,         |
|                                  | nitpicky builds for HTML, directory HTML,            |
|                                  | single-page HTML, PDF, text, gettext, and dummy.     |
+----------------------------------+------------------------------------------------------+
| Custom templates                 | Manual, GDS, and API build with their own HTML and   |
|                                  | book templates. Strict builds report no errors or    |
|                                  | warnings.                                            |
+----------------------------------+------------------------------------------------------+
| Equations in HTML                | Browser inspection confirms rendered SVG equations   |
|                                  | and a successfully loaded local MathJax bundle on a  |
|                                  | nested page. No remote font or math service is       |
|                                  | required.                                            |
+----------------------------------+------------------------------------------------------+
| Equations in PDF                 | The manual's rendered equation and proof pages were  |
|                                  | visually inspected. LaTeX-to-Typst conversion        |
|                                  | remains covered by the renderer suites.              |
+----------------------------------+------------------------------------------------------+
| Diagrams                         | Authored Graphviz directives produce static SVG in   |
|                                  | HTML and vector diagrams in PDF. Browser images load |
|                                  | correctly; PDF diagrams were inspected.              |
+----------------------------------+------------------------------------------------------+
| Themes and responsive navigation | Chrome checks cover light, dark, and system modes,   |
|                                  | persistence after navigation, a 390 × 844 viewport,  |
|                                  | the navigation drawer, and absence of page-wide      |
|                                  | overflow.                                            |
+----------------------------------+------------------------------------------------------+
| Search and links                 | Live browser search returns the current pages. All   |
|                                  | three generated sites pass the internal file and     |
|                                  | anchor checker.                                      |
+----------------------------------+------------------------------------------------------+
| Local assets                     | Every local image, script, media source, and         |
|                                  | stylesheet reference in the three HTML sites         |
|                                  | resolves to an existing output file.                 |
+----------------------------------+------------------------------------------------------+
| GDS authoring                    | A CLI integration test uses the actual RST template, |
|                                  | creates a draft, shows it, previews promotion,       |
|                                  | promotes it, changes state with evidence, checks the |
|                                  | index, and publishes HTML and PDF.                   |
+----------------------------------+------------------------------------------------------+
| Selected GDS PDF                 | A preview writes to its own output generation and    |
|                                  | leaves the complete catalog's PDF byte-for-byte      |
|                                  | unchanged.                                           |
+----------------------------------+------------------------------------------------------+
| Incremental publication          | A second strict build reuses the documents and       |
|                                  | output units. Multiple books retain separate         |
|                                  | generated Typst sources.                             |
+----------------------------------+------------------------------------------------------+
| Source and API rules             | The repository checker reports zero violations in    |
|                                  | 644 source files. Integration checks cover public    |
|                                  | API ownership and contracts, examples, imports, and  |
|                                  | source rules. Prose is exempt from source width and  |
|                                  | procedure limits.                                    |
+----------------------------------+------------------------------------------------------+

The live previews use **Guidedog's built-in server**, not a Python server:

.. code:: sh

   build/guidedog serve docs/manual --port=8765
   build/guidedog serve docs/gds --port=8766
   build/guidedog serve docs/api --port=8767

The mathematics bundle is MathJax 3.2.2, stored under ``docs/shared/_static/data``. Its
license and provenance are retained. Only pages that need mathematics load it. The theme
script supplies theme choice; the engine's search script supplies search. A template-only
script supplies Knuth–Plass layout for eligible English prose. It uses measured local fonts,
Liang patterns, and a paragraph-wide break search. The bundled Typeset, Hypher, and TeX
patterns retain their licenses and provenance. Link-bearing paragraphs, code, mathematics,
and measures narrower than 400 pixels keep native layout. In Chrome, the typography page has
four optimized paragraphs, 17 explicit lines, and one selected hyphenation break. At 390
pixels it restores ordinary paragraphs and has no page-wide overflow. The diagrams need no
client-side renderer.

Defects found and corrected during publication
----------------------------------------------

These changes address the boundary that caused each defect. Regression tests cover the
resulting contract.

1.  **Nested pages could resolve a local math script against the wrong directory.** Local
    MathJax URLs now use the page's path to the output root. Absolute and CDN URLs retain
    their meaning. A project unit test covers each form.

2.  **Several PDF books shared a generated source filename.** Each published PDF now has its
    own source under ``pdf/sources``. Failure sources are distinct too. The established
    ``book-en.typ`` alias remains for single-book projects. A regression test checks both
    books' source content and incremental reuse.

3.  **A standalone record lost its section outline and showed empty contents.** Only a
    multi-document book's introduction suppresses its root title. Standalone sections retain
    their outline and hierarchy. The book template omits contents when there are no outlined
    headings. Tests check the generated structure; all six GDS tables of contents were
    visually checked.

4.  **Selected publication could replace the full catalog's output generation.** Selected
    PDF previews now have their own output directory. This keeps atomic publication's
    complete-generation contract without deleting unrelated books.

5.  **A failed GDS project build hid its useful source diagnostic.** The GDS adapter now
    retains the first diagnostic's path, line, column, explanation, and repair hint after
    releasing the project arena. A failure test checks the retained report.

6.  **Language suffixes could make distinct book settings resolve to one PDF.** A preflight
    check rejects duplicate final output paths before compiling any book. The regression
    checks the diagnostic and preservation of the previous PDF.

7.  **An API doc comment treated ``file:line`` as a URI.** It now marks that notation as
    code. Browser inspection confirms code text rather than a spurious link.

8.  **Preview reloads reused stale configuration and ignored shared assets.** Each rebuild
    loads a candidate project. It owns storage outside the request arena. A successful build
    replaces the previous project; a failed one retains it. Shared static, template, and
    include directories participate in change detection. The integration regression
    exercises source edits, settings, invalid settings, repair, repeated requests, and
    shared CSS changes under AddressSanitizer.

9.  **Undocumented API entries could keep an empty description attached to the next
    signature.** Signatures are sticky only when they have description content. Empty
    descriptions retain their labels without adding a layout block. The renderer regression
    compiles a sequence of 80 undocumented entries. The final PDF scan finds no pages
    containing only headers or folios.

10. **The main alternate signal stack lacked an alignment guarantee.** Its storage now has
    explicit 16-byte alignment, matching the sanitizer shadow-memory requirements as well as
    the operating-system signal-stack contract. The instrumented CLI now completes
    quickstart and serves/reloads the test project.

The RST GDS adapter keeps the existing explicit lifecycle API and legacy Typst catalog
behavior. Metadata is data, never evaluated code. JSON arrays represent authors and labels.
The CLI validates the initial field list, dates, states, number and filename consistency,
and managed index regions. Promotion and edits use the existing transaction and recovery
machinery. The publication adapter delegates to the project engine rather than introducing a
second renderer.

Test results
------------

The isolated Linux checkout on ``insan@10.175.52.18`` is ``/home/insan/gd-docs-20261001``.
It uses the installed Odin, Typst, and native parser dependencies from the earlier
heavy-build environment.

-  The earlier complete engine pass ran **884 library tests** across 26 suites and **163
   integration tests**, with memory tracking enabled.
-  The final reading edition reran **26 Typst renderer tests**, including the new pagination
   regression, **45 host tests**, and all **163 integration tests** on the remote machine.
   All passed. The earlier local 163-test pass also passed.
-  The earlier **53 targeted AddressSanitizer tests** passed: 3 GDS and 50 project tests.
   The final instrumented CLI and test runner also passed the preview reload regression
   after the ownership and signal-stack corrections.
-  Strict, nitpicky HTML/PDF builds passed for the combined manual and standalone API.
   ``gds check --render`` passed. All reported zero errors and warnings.
-  A fresh independent quickstart project reused the complete template and asset set. Strict
   HTML/PDF builds with mathematics and Graphviz passed, as did all 31 internal links.
   Publication used only the native Guidedog CLI. Initial theme copying is a file operation;
   there is no custom-theme installation command.
-  Incremental manual builds reused all **62 documents** and the unchanged PDF.
-  Internal HTML checks: manual **21,134**, GDS **1,030**, API **19,388** links; **zero
   missing files and zero missing anchors**.
-  Local HTML asset audit: manual **652**, GDS **171**, API **300** references; **zero
   missing local assets**.
-  Chrome checks cover the reading layout, actual optimized paragraph/hyphen breaks, mobile
   fallback, light/dark/system themes, mathematics, navigation, and search.
-  All authored manual pages and all six GDS books were rendered and visually inspected.
   Representative API pages and the corrected enumeration sequences, contents, and final
   index were inspected. Automated checks cover all **775 pages** in the eight PDFs: every
   font is embedded, no extracted word lies outside its page, and no page contains only
   running furniture. Blank verso pages are intentional. All **9,989 internal PDF link
   destinations** resolve.
-  The repository checker passes for **644 implementation files**. Explicit checks also
   cover the custom HTML, CSS, JavaScript, configuration, and template files; prose and
   vendored data remain exempt. ``git diff --check`` passes.

Build and test logs are retained locally under ``build/review-docs-20261001/logs``. The
earlier CPython, Django, and Flask heavy-build evidence is separately retained under
``build/review-remote-20261001``; those runs are not counted as new runs here.

The earlier complete engine run used the source snapshot with SHA-256
``b15fb5a1dd4c7553ed923bb3bc6aa70c56a2e2357659787245f8ba7e18979739``. Its PDF collision
overlay was tested separately:

+---------------------------------+-------------------------------------------------------+
| File                            | SHA-256                                               |
+=================================+=======================================================+
| ``lib/project/builders.odin``   | ``eb7980d152512                                       |
|                                 | dda6432886bdb4a65b5250fd7d4f867ddc0932e57fdf8b81684`` |
+---------------------------------+-------------------------------------------------------+
| ``tests/incremental_test.odin`` | ``691695effcd3d                                       |
|                                 | 8841be511858d7f2168d5b3765b701ebb793e971feb5a5a3e66`` |
+---------------------------------+-------------------------------------------------------+

The later reading-edition changes have separate ``reading-*`` and ``showcase-*`` logs. The
earlier snapshot hash is historical; it does not identify this later edition. The final
publication uses the native Odin CLI, its HTML builder, and Typst PDF backend. Temporary
inspection and asset-conversion scripts are outside the repository. There is no Python
documentation-build pipeline.

Multilingual manual verification, 2026-10-01
--------------------------------------------

The manual now has Simplified Chinese for China (``zh_CN``), Japanese (``ja``), Korean
(``ko``), Hindi (``hi``), Spanish (``es``), and German (``de``) editions. The English API
appendix remains in every manual book. GDS and the standalone API project remain English.
There are 16 committed POT domains and 102 PO files. Native ``intl stat`` reports 792 manual
messages per translation, plus project UI messages: 796 translated entries in Chinese,
Japanese, Spanish, and German; 872 in Korean and Hindi. Every catalog has zero fuzzy and
untranslated entries.

Extraction uses ``gettext_exclude_patterns = ["api/**"]``. Its regression test verifies that
excluding the API from gettext leaves its published pages and references intact. Graphviz
alt text now participates in the ``image`` target; its source participates independently in
``literal-block``. Four target combinations are checked. All six manual diagram descriptions
have translations.

Visual inspection found a Japanese cover title that wrapped its last character. The cover
now measures the title before choosing its size, with a readable minimum. Inspection also
found unshaped Hindi prose inside equations. The Typst adapter now captures the prose fonts
outside math mode and boxes text with normal shaping. Bold, italic, and monospaced text
retain explicit text styles. Book templates supply their sans faces for mathematical prose;
standalone output keeps its previous sans fallback. The renderer version is 0.1.1.
Measurement and writing agree for the math cases, and compilation exercises both sans paths.

The final native builds and checks passed:

-  Seven strict manual HTML builds and seven strict PDF builds: zero errors and warnings.
   Heavy PDF builds ran on ``insan@10.175.52.18``.
-  The affected suites passed with memory tracking: 32 i18n, 26 Typst renderer, 164
   integration, and 50 project tests. The final English API HTML/PDF builds and
   ``gds check --render`` also passed.
-  The combined manual site contains 448 HTML pages. All 152,040 internal links resolve to
   existing files and anchors. The standalone API has 19,398 valid internal links. The HTML
   asset audit found no missing local assets; 91 CSS font references also resolve. Published
   books match the checked PDF files byte for byte.
-  Automated inspection covers all 2,527 pages in the seven manual books: every font is
   embedded, no extracted word lies outside its page, and no page contains only running
   furniture. Intentional blank versos remain. All 35,179 internal PDF link destinations and
   all outline destinations resolve.
-  Representative pages in all six translations were rendered and inspected: covers,
   contents, tutorials, diagrams, diagnostics, internals, the English API appendix, and the
   final index. Hindi proof and memory equations were inspected at full page size after the
   shaping correction.
-  Regular and bold PDF/web faces cover every translated character. Regional Noto CJK and
   Devanagari fonts have bundled licenses and provenance. The eight script WOFF2 subsets
   total 2,420,484 bytes. Complete PDF faces remain outside the HTML assets.
-  Chrome checks passed for chapter-preserving language selection, Simplified Chinese
   search, local mathematics, and light/dark/system controls. All six mathematical chapters
   fit the phone viewport without page-wide horizontal overflow. Eighteen isolated
   JavaScript checks cover language matching, saved preferences, direct links, query/anchor
   retention, and storage failures. The preview uses Guidedog's built-in server.
-  API paragraph comparisons across all six editions preserve the English prose; generated
   UI labels and smart quotation marks follow the edition language.
-  All 27 authored README files were reviewed. All 118 local README links resolve. The
   implementation checker passes for 644 files, as do explicit checks of the language
   scripts, styles, HTML template, and configuration. ``git diff --check`` passes.
   Documentation prose and vendored data remain exempt from code limits.

The reproduction commands are in ``docs/manual/README.md``. Final logs and inspection
reports are retained under ``build/review-docs-20261001/logs/intl-*``. Temporary catalog
authoring, font subsetting, and inspection scripts remain outside the repository.
Extraction, catalog merging/statistics, HTML/PDF publication, and preview use the native
Odin Guidedog CLI.

Reproduce the core checks
-------------------------

Run from the repository root after building the native dependencies:

.. code:: sh

   odin build cmd/guidedog -o:speed -vet -strict-style -out:build/guidedog
   odin run tools/check
   odin test tests -vet -strict-style -define:ODIN_TEST_FAIL_ON_BAD_MEMORY=true
   tools/sanitize.sh lib/gds lib/project
   build/guidedog build html docs/manual -W
   build/guidedog build pdf docs/manual -W
   build/guidedog build html docs/api -W
   build/guidedog build pdf docs/api -W
   build/guidedog gds check --render
   odin run tools/linkcheck -- docs/manual/_build/html -v
   odin run tools/linkcheck -- docs/gds/_build/html -v
   odin run tools/linkcheck -- docs/api/_build/html -v

Repeat a build to check reuse. Inspect PDF pages after changing source or templates. Use the
tutorial integration test to exercise the remaining builders.

Limits of the evidence
----------------------

The compatibility chapter records the pinned Sphinx, Docutils, CommonMark, and MyST
behavior, including documented differences. This work does not promise execution of Python
Sphinx extensions. Static autodoc has the limits explained in the manual. A missing
renderer, unsupported math command, invalid metadata, or external tool failure must produce
an actionable diagnostic rather than a false success.

Browser behavior was checked in Chrome. PDF visual inspection and automated checks support
the supplied editions; they do not prove that every future source, browser, font
installation, printer, or third-party extension will behave identically. The retained tests
make the supported contracts repeatable.
