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
``_templates/book.typ``, and ``_static/``. Templates are ordinary files we can edit. HTML
provides navigation, search, code highlighting, diagrams, and equations. PDF books provide
chapters, cross-references, figures, indexes, and language-aware typesetting.

The default theme is light. Readers can choose dark or the system theme. The manual detects
the browser language on its landing page and repository overview. A saved choice takes
precedence. Direct links to an edition or chapter remain explicit requests.

.. code:: sh

   guidedog convert notes.md --to pdf --output notes.pdf
   guidedog clean docs
   guidedog migrate conf.py

Install
-------

Each `release <https://github.com/insanai/guidedog/releases>`__ has an archive for Linux,
macOS, and Windows, with ``SHA256SUMS``. The Typst compiler and the tree-sitter parsers are
compiled into ``guidedog``. Diagrams use the Graphviz library (``libgvc``). Where the system does not provide it,
the archive's notes say how to install it. Fetching remote
intersphinx inventories and publishing to GitHub Pages use the HTTP client built into ``guidedog``, so no ``curl`` program is needed.
Check and install the archive:

.. code:: sh

   sha256sum -c --ignore-missing SHA256SUMS      # on macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
   tar -xzf guidedog-linux-amd64.tar.gz
   sudo install guidedog-linux-amd64/guidedog /usr/local/bin/

The release notes describe each platform's requirements and limits. See the :doc:`installation
guide <howto/install>` for the details.

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

The ``typst`` crates are a default feature of ``gd-typst``. Without them, Guidedog runs an
installed, pinned ``typst`` program for PDF output.
`RELEASING.md <https://github.com/insanai/guidedog/blob/main/RELEASING.md>`__ describes the
release workflow.

The workspace is split by responsibility. The engine crates (``guidedoc``, the readers and
renderers, ``guidedoc-defaults``) use only the standard library. Supporting libraries
(``gd-toml``, ``gd-json``, ``gd-jinja``, ``gd-highlight``, ``gd-gettext``, ``gd-graphdog``,
``gd-typst``, ...) never depend on the engine. The project builder and the command line sit
above both. The programs are under ``cmd/``. See :doc:`Internals <internals/index>`.

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
repository overview are translated. API and GDS remain English. See the :doc:`manual build
instructions <howto/maintain-docs>` for assembling all editions and PDF books.

Extend and publish
------------------

Write a build plugin or a publisher with the
:doc:`step-by-step plugin chapter <plugins>`. A plugin implements one Rust trait. The
same value is linked into a custom ``guidedog`` binary with ``gd_cli::run_with``, or served
as an installed program with ``gd_plugins::serve_plugin``. The chapter then follows the real
GitHub Pages publisher through ownership, credentials, conditional updates and recovery.
``guidedog publish --timeout=SECONDS`` bounds each complete exchange (default: 300 seconds).
We choose a value longer than ``--wait``. After an unknown outcome, we recover before publishing
again.

Keep discussions
----------------

``guidedog dsc`` keeps proposals and decisions as reStructuredText records, one file each. It builds
them into a website and PDF books. Each catalog has a prefix: Guidedog's
design records use ``gds``, and another project might use ``eds``.

.. code:: sh

   guidedog dsc init docs/eds --prefix=eds --name="Enso Discussions"
   cd docs/eds
   guidedog dsc new "Cache policy" --labels=architecture
   guidedog dsc promote cache-policy
   guidedog dsc state 1 --to=accepted --reason="Review converged"
   guidedog dsc build

A catalog's ``discussion.toml`` sets its lifecycle states, record types, and numbering.
``guidedog dsc config`` prints it. The commands come from the discussion plugin, which every ``guidedog`` links, so nothing else needs to be installed. Guidedog's own ``docs/gds`` is one such catalog: we run ``guidedog dsc check`` in that folder. See :doc:`Keep a discussion catalog <howto/discussions>`.

Read and embed
--------------

-  :doc:`Manual <index>`: tutorials, topics, practical tasks, reference, and internals.
-  :doc:`API reference <api/index>`: generated from the Rust source of the main crates and
   included in the manual's PDF appendix.
-  :download:`Engine guide <_static/repository/library-README.md>`: the ``guidedoc``
   crate's workspaces, requests, adapters, and conversion examples.
-  :doc:`Templates <templates>`: customize HTML with Jinja and books with Typst.
-  :download:`GDS <_static/repository/gds-README.md>`: numbered design records and
   implementation decisions.
-  :download:`Discussion library <_static/repository/discussion-README.md>`: the
   catalogs that ``guidedog dsc`` keeps.
-  :download:`Beta review <_static/repository/beta-review-20261011.md>`: the review of
   the Odin implementation's beta: fixes, verification, support scope.

Guidedoc works in a caller-owned, reusable workspace and returns structured diagnostics. The
host manages files, budgets, caching, publication, and native compilers. ``--untrusted``
restricts capabilities. It is not a process sandbox or a limit on all native resource use.

Credits and license
-------------------

Created by **Vikrant Rathore**, with assistance from **Ronak Rathore** and **Kanak
Rathore**. Contributions are welcome through issues and pull requests. Documentation credits
use names or handles, not personal email addresses. Git identity settings are managed
separately.

Thanks to Sphinx and Georg Brandl, Docutils and David Goodger, Typst, Tree-sitter and Max
Brunsfeld, Graphviz, the Rust project, and Ginger Bill and the Odin community, in whose
language Guidedog was first written.

Guidedog is released under the
:download:`GNU Affero General Public License v3.0 or later <_static/repository/LICENSE>`
(AGPL-3.0-or-later). For other licensing terms, contact the authors.
:download:`NOTICE <_static/repository/NOTICE>` says how to reach them and lists the
third-party components, such as the bundled fonts and the Rust crates compiled into the
programs, that keep their own licenses.

An additional permission allows Guidedog to be linked and distributed with Graphviz. See
:download:`NOTICE <_static/repository/NOTICE>`.
