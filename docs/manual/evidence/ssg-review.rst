Static publishing review
========================

On 16 October 2026 we reviewed and implemented the complete GDS 0013 contract in Rust.
The scope includes publication state, filtered collections, taxonomy archives, pagination, collection feeds, standalone article PDFs, mounted projects, language editions, shared assets, pinned themes, starter presets and navigation tracing.
Beta.6 is withheld until the complete implementation and release qualification pass.

Review corrections
------------------

We retain the existing ``conf.toml`` model, ``gd-jinja`` renderer, explicit environment records and output journal.
We do not add a competing configuration format or a second rendering pipeline.
Publication filtering precedes resolution, navigation, search, source downloads and discovery output.
Source publication metadata is excluded from gettext translation, so a catalog cannot rename a draft key or clear its value.

The first visual review found leading configuration fields in the page body.
Recognized publication fields now supply layout data and are omitted from body rendering and search text.
Unknown leading fields and ordinary later field lists remain content.
Regression coverage exercises both RST and Markdown.

The initial 1,000-article measurement found repeated chronological sorting during navigation checks.
We replaced it with one article-neighbor index per site build and parse each selected date once per collection query.
HTML records actual navigation context reads, native function arguments and rendered toctree answers rather than depending on every document's navigation.
Tests verify that a section beyond a requested depth leaves its listing page cached.

Behavioral checks
-----------------

``crates/gd-project/tests/ssg.rs`` covers publication boundaries, clock-only cache reuse, preview transitions, source cleanup, invalid metadata, gettext protection, template overrides and real article PDFs.
``ssg_collections.rs`` checks filtering, taxonomy paths, stable pagination, relative links, per-collection feeds, metadata context, output collisions and removal of obsolete pages under HTML and dirhtml.
``ssg_navigation.rs`` checks dynamic ``hasdoc`` and ``pathto`` calls, parameterized template trees, body toctrees, title changes and changes outside a traced depth.

``crates/gd-ssg/tests/site.rs`` builds real projects through mounted publication.
A later mount failure preserves the preceding whole website.
An old artifact cannot satisfy a cross-mount link to a newly withheld target.
Separate source editions, shared assets, path conflicts, confined sources and HTML-only previews are covered.

Theme checks cover canonical archive round trips, raw archive pins, traversal, reserved platform names, case aliases, file/directory overlap and symlinks.
The maximum-file-count regression reproduced an inconsistency between archive validation and directory verification.
File and directory bounds now agree, directory case aliases are rejected, and the deepest accepted path survives installation and offline verification.
We verify the installed tree and encoded lock before selecting the lock, so a validation failure cannot select an unusable theme.
A loopback HTTP install makes exactly one request.
Subsequent builds succeed after that server shuts down.
Changed installed files fail verification before a project can build.
Project templates take precedence over the installed theme.

Every release archive runs the complete SSG smoke scenario with an empty ``PATH``.
It creates all three presets, packs and installs a pinned local theme, verifies it offline, assembles mounts, checks taxonomy and second pagination pages, emits collection feeds and compiles the research paper's real PDF.
The lifecycle smoke scenario separately checks scheduled publication, draft cleanup and standalone article PDFs.

Measurements
------------

We ran::

   cargo test -p gd-project --test ssg_benchmark -- --ignored --nocapture

The fixture has 1,000 dated articles, roughly 5 MiB of prose, tag and year archives, 20 articles per archive page, feeds and a home-page collection.
The Linux debug build recorded:

==================  =======  ======  =======  ======
Build               Seconds  Parsed  Written  Reused
==================  =======  ======  =======  ======
Cold                8.650    1001    1102     0
Unchanged           4.240    0       0        1102
One body edit       4.126    1       2        1100
Unused template     3.294    0       0        1102
==================  =======  ======  =======  ======

The body edit writes its article HTML and the search index.
It does not rewrite summary-only collections or unrelated pages.
The earlier repeated-sort implementation took 36.935 seconds cold and 26.667 seconds unchanged on the same fixture.
These are local debug measurements with other builds active, not production latency guarantees.
Every build still discovers and hashes project inputs.
CI runs the same benchmark as a regression check on parsed and written counts.

Visual and website review
-------------------------

We built the project, journal and research presets and reviewed their home pages in headless Chromium at 1440 by 1100 pixels.
We also reviewed the research article's HTML, mathematics, metadata and download link.
We rasterized its generated PDF with Poppler and reviewed the title, date, abstract, equation, derivation and page bounds.
The article uses continuous prose and the shared print theme.

The complete repository website is now declared in ``docs/conf.toml``.
It includes seven landing editions, seven manual HTML editions, seven manual books, the discussion website, all active discussion PDFs and the discussion index PDF.
``cargo xtask build-site`` delegates to ``guidedog ssg build``.
Its first complete mounted output passed 712 HTML pages and 171,521 internal links with no missing files or anchors.
The final release record gives the final website and archive qualification results.
