.. _plugin-review--14-october-2026:

Plugin review — 14 October 2026
===============================

Guidedog 0.6.2 introduces build services alongside publishing services. A plugin can
transform primary RST or Markdown input, transform a complete HTML page, or provide
namespaced HTML assets. GitHub Pages is a publishing service. Normal builds never publish to
a remote destination.

The public interface is an Odin procedure table. One reusable package supplies its
descriptor. An application can borrow that table through ``project.Load_Options.plugins``; a
small executable wrapper exposes it to the CLI through ``plugins.serve_plugin``. Installed
plugins run in separate processes. There is no Python runtime, Python plugin loader, or
dynamically loaded native ABI.

Findings and design changes
---------------------------

-  The former tutorial taught publishing before showing how to extend a build. It now
   compiles a product-badge package, creates a project with Guidedog, configures it,
   registers it, builds it, and explains the exact tested source. The Windows command
   actually creates the ``.exe`` before registration uses it.
-  Build and publishing services now have separate versioned contracts. A table cannot mix
   the two. Unknown hooks or incompatible versions are refused with correction directions.
   Earlier SDK compatibility is not a release requirement.
-  Registration now qualifies build plugins in bounded child processes before recording
   their checksums. Checks exercise fresh sessions, repeated RST and Markdown
   transformations, complete HTML, assets, and cleanup. A separate ``plugin check`` command
   makes the same checks without registering anything.
-  Plugin callbacks do not receive mutable document trees or writable output handles.
   Replacements are copied into owned results. Asset paths are checked for traversal,
   platform-specific forbidden names and case collisions. Asset staging also checks
   conflicts against project files, including cached units.
-  A build owns private sessions and serializes their callbacks. Plugins need no thread-safe
   global state. Text and asset sizes are bounded, and process requests have deadlines that
   cover both blocked writes and reads.
-  Cache keys include plugin identity, loaded configuration fingerprints, dependencies and
   generated asset bytes. The SDK's file readers fingerprint the bytes actually loaded.
   Changing a file after load cannot silently pair old session state with a new file's
   fingerprint. Relative configuration paths use the configuration directory, including a
   separate ``-c`` directory.
-  SDK cleanup retains its allocation owner even when a callback clears the session. The
   descriptor, options and owner remain SDK-managed across hooks. Allocation-refusal tests
   check that failure and cleanup leave no live allocations or invalid frees.
-  macOS pipe writes now suppress SIGPIPE on the descriptor. A dead plugin reader cannot
   terminate the host or change its process-wide signal handlers.
-  Invalid builder argument order no longer silently builds HTML. The diagnostic explains
   the accepted command and how to select a custom output directory.
-  Plugin-expanded markup diagnostics identify the expanded input and show the actual
   excerpt. Plugin problems must include a concrete corrective hint.

Verification before tagging
---------------------------

Heavy checks ran on the Linux review machine in an isolated working directory. The complete
176-test integration suite and all library/tool suites passed with leak and invalid-free
checks. The full AddressSanitizer run passed; the final owner changes also passed targeted
SDK and project AddressSanitizer checks. The SDK suite has 21 tests. macOS SDK checks,
source limits, diagnostic contracts, and compilation of documented programs passed locally.
The source checker reported zero violations across 781 Odin files.

The end-to-end tests compile the tutorial's real executable and exercise check, registration
(including refusal of a crashing program), RST, Markdown, HTML, directory HTML, single HTML,
text, nested assets, generated pages and unchanged rebuilds. Configuration changes
invalidate the right cached work. Allocation failures, child exits, a real child runtime
bounds failure, a stalled child, malformed replies, invalid UTF-8 and invalid expanded
markup all preserve the previous complete output. A following healthy build succeeds.

Guidedog built the landing page and manual in all seven languages, including seven PDF
books, the English API and the GDS catalog with individual PDFs. The new GDS's missing
PDF-list entry was found and corrected. Link checking reported 582 HTML pages, 197,840
internal links, zero missing files and zero missing anchors. Translation statistics report
no fuzzy or untranslated active entries. The tutorial's English PDF pages were rendered and
visually inspected. Safari displayed the HTML chapter and pipeline diagram through
Guidedog's built-in preview server. Its navigation and code blocks match the executable
tutorial. Sample tutorial pages from the six translated books were also rendered and
inspected; commands, typography and glyphs fit without clipping.

Fresh and unchanged HTML and PDF builds of CPython, Django and Flask also passed on the
remote machine. All twelve builds matched the earlier diagnostic counts. All six unchanged
rebuilds preserved every output file's path, size, inode and modification time. These remain
imported workload fixtures with their recorded unsupported project-specific markup, not
claims of Sphinx compatibility.

`Verify <https://github.com/insanai/guidedog/actions/runs/38431147374>`__ passed on Linux,
macOS and Windows at ``b5f9b060df48604083a6af42c5f6d1470e24f92d``. The `complete release
candidate <https://github.com/insanai/guidedog/actions/runs/38431698108>`__ at that same
revision passed Linux and macOS qualification, full AddressSanitizer checks, all five
builds, all five packaged smoke tests and Homebrew installation and testing. The Intel
archive ran on a real Intel Mac. Credential checks used the native stores, including an
isolated Linux Secret Service keyring.

The downloaded Apple silicon candidate matched every program checksum in its build
manifests. ``guidedog``, ``dsc``, ``dscc`` and ``guidedog-pages`` reported 0.6.2. Its
Guidedog checked and registered the compiled badge example, then built HTML and an
embedded-Typst PDF containing transformed text, math and a Graphviz figure. The tag names
that exact qualified revision. The `tagged release
workflow <https://github.com/insanai/guidedog/actions/runs/38434657070>`__ passed every
qualification, build, smoke, Homebrew and publication job and released
`0.6.2 <https://github.com/insanai/guidedog/releases/tag/v0.6.2>`__. The guide's Actions
example pins its published Linux archive checksum. Local Linux testing alone does not
establish every platform; the release used the separate native runner checks.

The published Apple silicon Guidedog and dscc archives were downloaded separately and
matched ``SHA256SUMS``. GitHub build provenance verification succeeded for the Guidedog
archive. All four published programs reported 0.6.2. The published host also rebuilt the
registered badge example as HTML and embedded-Typst PDF without warnings. The official tap's
`Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/38438075296>`__
verified the release, installed and tested it, and committed the 0.6.2 formula.

Limits that remain explicit
---------------------------

Qualification demonstrates interoperability, not a proof of safety. Runtime responses are
still validated. Process isolation keeps a plugin memory fault outside Guidedog; it does not
sandbox a trusted program's filesystem or network access. Child RSS is outside the host's
build budget. Native applications that link callbacks directly trust those callbacks; a
native crash cannot be caught reliably, and a stuck native callback cannot be forcibly
interrupted safely.

Primary-source hooks do not rewrite engine-managed include files. Assets are HTML services.
Installed custom directives, roles and builders are outside this version's contract;
compiled applications can use the existing reader/renderer registry. Plugin authors must
declare other loaded inputs and retain their fingerprints in ``session.cache_key``.

At release time GDS 0008, 0009 and 0010 remained in discussion. On 14 October, the
maintainer requested review and acceptance of completed designs. Their implemented scope is
now committed; the decision and its limits are recorded in `the acceptance
review <../../gds/evidence/plugin-acceptance-20261014.rst>`__. GDS 0002 remains published
because its broader native-Typst obligations are unfinished.
