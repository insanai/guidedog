Guidedog
========

**Write once. Explain well. Publish on the web and on paper.**

Guidedog builds HTML documentation sites and PDF books from reStructuredText and Markdown.
It is written in Rust. Its conversion engine is **Guidedoc**, and the embedded Typst
compiler typesets the PDF pages. No Python environment or LaTeX installation is needed.

Sphinx inspires the authoring workflow. Guidedog is an independent system: configuration is
declarative TOML, API extraction is static, and Python configuration and extensions are
never executed. Complete Sphinx compatibility is not a goal.

This page is the repository overview. On GitHub, the ``README`` files at the root of the
repository give the same overview in each language of the manual.

Start a project
---------------

.. code:: sh

   guidedog quickstart docs
   guidedog build html docs
   guidedog build pdf docs
   guidedog serve docs

The project contains ``conf.toml``, ``index.rst``, ``_templates/layout.html``,
``_templates/book.typ``, and ``_static/``. Templates are ordinary files you can edit. HTML
provides navigation, search, code highlighting, diagrams, and equations. PDF books provide
chapters, cross-references, figures, indexes, and language-aware typesetting.

The default theme is light. Readers can choose dark or the system theme. The manual detects
the browser language on its landing page and repository overview. A saved choice takes
precedence; direct links to an edition or chapter remain explicit requests.

.. code:: sh

   guidedog convert notes.md --to pdf --output notes.pdf
   guidedog clean docs
   guidedog migrate conf.py

Install
-------

Each `release <https://github.com/insanai/guidedog/releases>`__ has an archive for Linux,
macOS, and Windows, with ``SHA256SUMS``. The Typst compiler and the tree-sitter parsers are
compiled into ``guidedog``. Diagrams use the Graphviz library (``libgvc``), which the
archive's notes say how to install where the system does not provide it. Fetching remote
intersphinx inventories and publishing to GitHub Pages run the system's ``curl`` program.
Check and install the archive:

.. code:: sh

   sha256sum -c --ignore-missing SHA256SUMS      # on macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
   tar -xzf guidedog-linux-amd64.tar.gz
   sudo install guidedog-linux-amd64/guidedog guidedog-linux-amd64/dsc /usr/local/bin/

The release notes describe each platform's requirements and limits. See the `installation
guide <howto/install.rst>`__ for the details.

Build from source
-----------------

Install a Rust toolchain (the workspace's ``rust-version`` or later) and the Graphviz
development files (``libgvc``, ``libcgraph``; for example ``graphviz-dev`` on Debian and
Ubuntu, or ``brew install graphviz``). Then, at the checkout root:

.. code:: sh

   cargo build --release -p guidedog-cmd     # target/release/guidedog
   cargo test --workspace
   cargo clippy --workspace --all-targets
   cargo run -p check                        # source limits

The ``typst`` crates are a default feature of ``gd-typst``; without them Guidedog runs an
installed, pinned ``typst`` program for PDF output.
`RELEASING.md <https://github.com/insanai/guidedog/blob/main/RELEASING.md>`__ describes the
release workflow.

The workspace is split by responsibility: the engine crates (``guidedoc``, the readers and
renderers, ``guidedoc-defaults``) use only the standard library; supporting libraries
(``gd-toml``, ``gd-json``, ``gd-jinja``, ``gd-highlight``, ``gd-gettext``, ``gd-graphdog``,
``gd-typst``, ...) never depend on the engine; the project builder and the command line sit
above both; the programs are under ``cmd/``. See `Internals <internals/index.rst>`__.

Translate with Guidedog
-----------------------

.. code:: sh

   mkdir -p docs/manual/_editions
   guidedog build -b gettext docs/manual docs/manual/locales/templates -W
   guidedog intl update -p docs/manual/locales/templates \
       -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
   guidedog intl stat -l zh_CN -l ja -l ko -l hi -l es -l de docs/manual
   guidedog build -b html docs/manual docs/manual/_editions/de \
       -D language=de -D 'html_extra_path=[]' -W

Translate and review ``msgstr`` values in the PO files. Guidedog extracts POT files, merges
catalogs, reports progress, and builds each edition. It does not invent translations.
Commands, paths, code, API names, and reference targets stay unchanged. Only the manual and
repository overview are translated; API and GDS remain English. See the `manual build
instructions <howto/maintain-docs.rst>`__ for assembling all editions and PDF books.

Extend and publish
------------------

Write a build plugin or a publisher with the
{doc}\ ``step-by-step plugin chapter <plugins>``. A plugin implements one Rust trait; the
same value is linked into a custom ``guidedog`` binary with ``gd_cli::run_with``, or served
as an installed program with ``gd_plugins::serve_plugin``. The chapter then follows the real
GitHub Pages publisher through ownership, credentials, conditional updates and recovery.
``guidedog publish --timeout=SECONDS`` bounds each complete exchange (default: 300 seconds);
choose a value longer than ``--wait``. After an unknown outcome, recover before publishing
again.

Keep discussions
----------------

``dsc`` keeps proposals and decisions as reStructuredText records, one file each, and builds
them into a website and PDF books with Guidedog. Each catalog has a prefix: Guidedog's
design records use ``gds``, and another project might use ``eds``.

.. code:: sh

   dsc init docs/eds --prefix=eds --name="Enso Discussions"
   cd docs/eds
   dsc new "Cache policy" --labels=architecture
   dsc promote cache-policy
   dsc state 1 --to=accepted --reason="Review converged"
   dsc build

A catalog's ``discussion.toml`` sets its lifecycle states, record types, and numbering, and
``dsc config`` prints it. ``dsc`` comes in the Guidedog archive and builds with the
installed ``guidedog``. ``dscc`` is ``dsc`` with Guidedog built in, for a machine with
nothing else installed. ``guidedog gds`` runs the same commands on Guidedog's own
``docs/gds``. See `Keep a discussion catalog <howto/discussions.rst>`__.

Read and embed
--------------

-  `Manual <index.rst>`__: tutorials, topics, practical tasks, reference, and internals.
-  `API reference <api/index>`__: generated from the Rust source of the main crates and
   included in the manual's PDF appendix.
-  {download}\ ``Engine guide <_static/repository/library-README.md>``: the ``guidedoc``
   crate's workspaces, requests, adapters, and conversion examples.
-  `Templates <templates.rst>`__: customize HTML with Jinja and books with Typst.
-  {download}\ ``GDS <_static/repository/gds-README.md>``: numbered design records and
   implementation decisions.
-  {download}\ ``Discussion library <_static/repository/discussion-README.md>``: the
   catalogs that ``dsc``, ``dscc``, and ``guidedog gds`` keep.
-  {download}\ ``Beta review <_static/repository/beta-review-20261011.md>``: the review of
   the Odin implementation's beta: fixes, verification, support scope.

Guidedoc works in a caller-owned, reusable workspace and returns structured diagnostics. The
host manages files, budgets, caching, publication, and native compilers. ``--untrusted``
restricts capabilities; it is not a process sandbox or a limit on all native resource use.

Credits and license
-------------------

Created by **Vikrant Rathore**, with assistance from **Ronak Rathore** and **Kanak
Rathore**. Contributions are welcome through issues and pull requests. Documentation credits
use names or handles, not personal email addresses; Git identity settings are managed
separately.

Thanks to Sphinx and Georg Brandl, Docutils and David Goodger, Typst, Tree-sitter and Max
Brunsfeld, Graphviz, the Rust project, and Ginger Bill and the Odin community, in whose
language Guidedog was first written.

Guidedog is released under the
{download}\ ``GNU Affero General Public License v3.0 or later <_static/repository/LICENSE>``
(AGPL-3.0-or-later). For other licensing terms, contact the authors.
{download}\ ``NOTICE <_static/repository/NOTICE>`` says how to reach them and lists the
third-party components, such as the bundled fonts and the Rust crates compiled into the
programs, that keep their own licenses.

An additional permission allows Guidedog to be linked and distributed with Graphviz; see
{download}\ ``NOTICE <_static/repository/NOTICE>``.
