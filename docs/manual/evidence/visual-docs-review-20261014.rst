Documentation and diagram review, 14 October 2026
=================================================

The review compared the manual's diagrams and examples with the project host, conversion
engine, plugin lifecycle, publication code and HTML renderer. It also checked the
implemented scope of GDS 0008, 0009 and 0010. Heavy builds and tests ran in the isolated
remote Linux checkout. Temporary analysis helpers are not part of Guidedog and add no Python
or Ruby dependency.

Findings and corrections
------------------------

-  A transparent Graphviz canvas left dark text on dark pages. Ordinary drawings retain
   their colors and light fallback panel. Explicit ``graphviz-adaptive`` diagrams now carry
   fixed CSS within the external SVG image. The image inherits the page's selected color
   scheme. There is no page-inline SVG, script or external stylesheet. Semantic classes
   distinguish inputs, outputs, decisions, plugin hooks and native allocation boundaries.
   The stylesheet participates in the cache key; ordinary drawings retain their existing
   cache identity. PDF uses a separate light drawing without the adaptive stylesheet.
-  Styling copies reserve the full replacement before allocation, including the original's
   live storage. Tests cover budget refusal, failed allocation, malformed output and
   unchanged geometry. Class scanning uses a borrowed string iterator rather than allocating
   a list.
-  Several drawings were 700–1,100 points wide. They would shrink to unreadable text in the
   book's roughly 363-point measure. The revised diagrams are at most about 431 points wide.
   They use short bold titles, quieter descriptions and one question per drawing. Memory
   accounting and process isolation are separate diagrams. Narrow HTML figures scroll
   without widening the page.
-  Asset collection was drawn after HTML callbacks. The host actually collects assets at
   plugin startup, before source discovery and reading. The diagrams now show that order and
   distinguish HTML hooks from PDF/text paths. They do not imply that native Graphviz,
   tree-sitter or embedded Typst runs in a child process.
-  The preorder example now includes the heading's text leaf. Its five nodes have consistent
   nested half-open intervals rather than treating a heading as a text leaf. The proof still
   states the assumptions the validator checks.
-  Publication diagrams now describe the journaled switch. Directory exchange is atomic
   where supported; the two-rename fallback can briefly leave the output path absent.
   Neither promises a snapshot across a whole browsing session. The guide also permits
   dedicated output below the source root and distinguishes semantic reference checks from
   external HTTP checks.
-  The themes chapter lacked a complete reproducible lesson. The new tutorial starts with
   ``guidedog quickstart``, then edits ordinary project files. It uses the real
   ``!layout.html`` blocks, a partial, a macro, relative links and theme tokens. Its PDF
   wrapper forwards the host's metadata and body to the generated book base. A style
   function must wrap its content; settings returned by an otherwise empty helper do not
   affect its caller's later body.
-  Downloadable tutorial files match the tested source files byte for byte. The integration
   test builds HTML, directory HTML, single-page HTML and PDF, checks nested links and edits
   a partial to verify invalidation. The lessons now name real language-suffixed PDFs,
   enable figure numbering explicitly and explain that page numbers in cross-references are
   a book-template choice.
-  Local download roles previously produced ordinary links. HTML templates opened as pages
   instead of saving their source. Resolved local downloads now carry the HTML ``download``
   attribute. Ordinary and external links keep their behavior. PDF downloads use the
   absolute published ``html_baseurl`` when configured. The manual sets its canonical site
   URL; tests change that URL and require a rebuilt book with the new links. Unconfigured
   projects keep source-relative links and do not embed attachments. The link checker no
   longer parses attachments as generated pages; a regression still requires it to report
   links to missing attachments.
-  Guidedog's gettext and intl commands refreshed the maintained PO catalogs. Updated
   explanations, captions, alternative text and the complete theme lesson have Simplified
   Chinese, Japanese, Korean, Hindi, Spanish and German text. Filenames, command syntax and
   API identifiers remain unchanged. Unrelated catalog timestamp churn was removed. API and
   GDS prose remain English.

Verification
------------

The final affected suites passed with leak and invalid-free checks:

======================= =================================================================
Check                   Result
======================= =================================================================
Integration tests       177 passed
Project library         66 passed
HTML renderer           32 passed
Site link checker       3 passed
AddressSanitizer, Linux Project and HTML suites passed
Source checker          785 files, zero violations
Complete website        Seven manual HTML editions and seven PDF books; API and GDS built
Internal links          590 HTML pages, 199,212 links, zero missing files or anchors
Translation catalogs    Zero fuzzy or untranslated active messages in all six languages
Theme example           Fresh quickstart, HTML and PDF, zero warnings
======================= =================================================================

The strict site build used ``tools/build-site.sh`` with the current Guidedog binary. The
exact tutorial also builds through the integration test. The complete GDS catalog passes
``guidedog gds check``.

Chromium displayed the manual through Guidedog's built-in preview server over a loopback SSH
tunnel. Light, explicit dark and system modes were checked, including an explicit dark
choice while the system preferred light. A 390-pixel viewport kept page width bounded while
its diagram scrolled horizontally. The system choice followed a changed emulated preference.
The adaptive dark palette preserved its semantic colors rather than turning every node
green.

Poppler rendered the English book's pipeline and memory diagrams and the theme lesson in all
seven books. The sampled pages showed complete labels, code, native script glyphs, running
heads and page numbers without clipping. These are visual samples, not a claim to have
manually inspected every page of every book.

The Pages workflow's pinned ``configure-pages`` v6.0.0 was checked against GitHub's latest
release. No new product release is made by this documentation change; adaptive Graphviz and
the download correction are recorded under Unreleased.

Design record decisions
-----------------------

GDS 0008, 0009 and 0010 were accepted and then committed through Guidedog's own catalog CLI,
with reasons and evidence. Their scope is implemented and supported by the published 0.6.2
qualification, rechecked against GitHub's release record. The `acceptance
record <../../gds/evidence/plugin-acceptance-20261014.rst>`__ preserves that decision and
its limits. Acceptance does not freeze the beta SDK or add a sandbox, installer, mutable
doctree interface or custom builders.

GDS 0002 remains published. Hard native-Typst isolation and native Typst chapter modules are
still unfinished. Its stale claim that Windows runtime testing was outstanding has been
corrected with the actual release evidence.
