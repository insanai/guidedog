Command-line reference
======================

The ``guidedog`` executable creates, builds, previews, translates, converts, and publishes
documentation projects.
Its commands and options follow ``sphinx-quickstart``, ``sphinx-build`` and
``sphinx-intl`` where those have an equivalent. Existing habits and scripts carry
over.

General syntax
--------------

.. code-block:: sh

   guidedog <command> [arguments] [options]
   guidedog <command> --help
   guidedog --version
   guidedog --licenses
   guidedog --help

Options may come before or after the arguments. ``--`` ends the options: every word after
it is an argument, even one that starts with ``-``. A lone ``-`` is an argument (standard
input or output, where a command accepts it). A long option takes its value as
``--name=VALUE`` or as the next word, ``--name VALUE``.

Global options
--------------

Every command accepts these:

.. list-table:: Global options
   :header-rows: 1
   :widths: 35 65

   * - Option
     - Meaning
   * - ``--color=auto|always|never``
     - Terminal styling (default: ``auto``). ``auto`` styles only a terminal, and
       respects ``NO_COLOR`` and ``TERM=dumb``.
   * - ``--diagnostics=text|json``
     - Explained reports (default), or one JSON object per report on standard error.
   * - ``-q``, ``--quiet``
     - Suppress successful status messages; warnings and errors still show.
   * - ``-h``, ``--help``
     - Show the help of ``guidedog``, or of the command it follows.
   * - ``--version``
     - Show the versions of Guidedog, the Guidedoc engine (and its document schema),
       the tree-sitter grammars, and the Typst backend in use.
   * - ``--licenses``
     - Show Guidedog's licence and the notices of the libraries built in.

Exit codes
----------

.. list-table:: Exit statuses
   :header-rows: 1
   :widths: 20 80

   * - Exit code
     - Condition
   * - ``0``
     - Success: all operations completed with no unhandled errors.
   * - ``1``
     - Build or conversion error (or warnings treated as errors via ``-W``).
   * - ``2``
     - Command-line syntax error, invalid flag, unrecognized option, or invalid
       configuration.
   * - ``3``
     - A configured resource limit was reached, or memory was unavailable.
   * - ``4``
     - A file operation or external program failed.
   * - ``5``
     - An internal error; the diagnostic explains how to report it.
   * - ``6``
     - ``publish`` only: the host accepted the publication, but has not confirmed it live.
   * - ``7``
     - ``publish`` only: whether the publication was applied is not known; run the
       publisher's ``recover``.
   * - ``70``
     - A crash; the previous published output remains available.

Commands overview
-----------------

.. list-table:: CLI commands
   :header-rows: 1
   :widths: 30 70

   * - Command
     - Purpose
   * - ``quickstart [DIR]``
     - Creates a documentation project with visible templates and configuration.
   * - ``build [BUILDER] [DIR]``
     - Builds the project into the requested output target.
   * - ``serve [DIR]``
     - Builds the website, serves it locally, and rebuilds it when sources change.
   * - ``clean [DIR]``
     - Removes the contents of the build folder.
   * - ``migrate [conf.py|DIR]``
     - Converts a Sphinx ``conf.py`` into declarative ``conf.toml``.
   * - ``intl update|stat [DIR]``
     - Updates translation catalogs (PO), or reports how much is translated.
   * - ``convert INPUT``
     - Converts one file, or standard input, without a project.
   * - ``formats``
     - Lists the readers and renderers built in, and the Typst backend.
   * - ``publish --publisher=ID``
     - Builds the project a publisher's configuration names, and publishes the site.
   * - ``plugin COMMAND``
     - Registers, checks, lists, inspects, runs, or unregisters plugin programs.

Plugins add commands of their own (see `Commands plugins add`_). The GitHub Pages plugin, linked into every ``guidedog``, adds publishing and maintenance commands:

.. list-table:: Commands of the linked GitHub Pages plugin
   :header-rows: 1
   :widths: 30 70

   * - Command
     - What it does
   * - ``publish pages``
     - Publishes to GitHub Pages, doing first whatever setup is still missing.
   * - ``pages workflow``
     - Writes a GitHub Actions workflow that builds and deploys the site.

The discussion plugin, also linked into every ``guidedog``, adds ``dsc`` and its commands: ``guidedog dsc init``, ``new``, ``build``, and the rest (see `guidedog dsc`_).

A command word that is neither guidedog's nor a plugin's is a usage error (exit status 2). Its hint lists the commands, and suggests the closest of both kinds.

guidedog quickstart
-------------------

Creates a project folder, as ``sphinx-quickstart`` does. It holds ``conf.toml``, ``index.rst``, working templates
in ``_templates/``, styles in ``_static/``, ``assets/`` folders, and a build folder:

.. code-block:: sh

   # Interactive prompt
   guidedog quickstart docs

   # Non-interactive creation with predefined options
   guidedog quickstart docs -q -p "My Project" -a "Author Name" -v 1.0 -l en

``DIR`` defaults to the current folder. In a terminal, and without ``-q``, quickstart
asks whether to separate source and build folders. It also asks for the project name, the
authors, the release, and the language (default ``en``). It skips what the options
already gave. The project name and author are required.

.. list-table:: Options of quickstart
   :header-rows: 1
   :widths: 35 65

   * - Option
     - Meaning
   * - ``-q``, ``-Q``, ``--quiet``
     - Ask nothing; take the answers from the options below.
   * - ``-p NAME``, ``--project NAME``
     - The project name.
   * - ``-a NAME``, ``--author NAME``
     - The author or organization name(s).
   * - ``-v VERSION``, ``--version-name VERSION``
     - The short version (``1.0``). Without it, the release is used.
   * - ``-r RELEASE``, ``--release RELEASE``
     - The full release (``1.0.0-rc1``).
   * - ``-l LANGUAGE``, ``--language LANGUAGE``
     - The document language (default: ``en``).
   * - ``--sep``, ``--no-sep``
     - Separate ``source/`` and ``build/`` folders, or keep the sources at the root with
       ``_build`` beside them (the default).
   * - ``--suffix EXT``
     - The source suffix (default: ``.rst``).

guidedog build
--------------

Reads the sources and resolves toctrees and cross-references. It publishes the output as
one complete generation:

.. code-block:: sh

   # Standard project build
   guidedog build html docs
   guidedog build pdf docs

   # sphinx-build forms
   guidedog build -b html docs _build/html
   guidedog build -M html docs _build

``guidedog build [BUILDER] [PROJECT]`` builds ``PROJECT`` with ``BUILDER`` into the project's build folder, ``_build/BUILDER``.
``PROJECT`` defaults to the current folder. Guidedog looks for ``conf.toml`` there, in ``PROJECT/source``, then in the nearest parent.
``BUILDER`` defaults to ``html``.
``-b BUILDER SOURCEDIR OUTPUTDIR`` writes to ``OUTPUTDIR`` itself.
``-M BUILDER SOURCEDIR BUILDDIR`` writes to ``BUILDDIR/BUILDER``. Both forms need exactly the two folders.

Builders:

* ``html``: a complete HTML site with search, indexes, navigation, and ``objects.inv``.
* ``dirhtml``: HTML with directory-style URLs (``dir/index.html``).
* ``singlehtml``: all documents in one HTML page.
* ``pdf``: PDF books typeset by Typst. Sphinx's ``latex``, ``latexpdf``, ``latexpdfja``
  and ``xelatexpdf`` select it too.
* ``text``: a plain text file for each document.
* ``gettext``: message templates (POT) for translation.
* ``dummy``: reads and resolves every document and writes nothing (a syntax and
  reference check).
* ``coverage``: reads every document and writes ``python.txt`` and ``c.txt``, the objects the documents do not describe (sphinx.ext.coverage).

.. list-table:: Options of build
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Meaning
   * - ``-a``
     - Write all outputs, not only the changed ones.
   * - ``-E``
     - Fresh build: ignore the saved environment.
   * - ``-W``
     - Treat warnings as errors.
   * - ``--keep-going``
     - With ``-W``, finish the build to report every problem before failing.
   * - ``-n``
     - Nitpicky: warn about every unresolved reference.
   * - ``-j N``, ``-j auto``
     - Read documents on ``N`` threads, or one per core.
   * - ``-D name=value``
     - Override a ``conf.toml`` setting for this build (``-D language=de``); repeatable.
       ``-D table.key=value`` sets one key of a table setting, such as ``html_context``,
       and keeps its other keys, as sphinx-build does.
   * - ``-t TAG``
     - Define a tag for ``only`` directives; repeatable.
   * - ``-c DIR``
     - Read ``conf.toml`` from ``DIR`` instead of the source folder.
   * - ``-C``
     - Build without any ``conf.toml``.
   * - ``-d DIR``
     - Accepted for sphinx-build scripts and ignored: the environment lives in the
       build's own cache folder.
   * - ``-v``
     - Also show notes (Docutils' info messages); repeatable.
   * - ``-q``, ``-Q``
     - Show only errors: no progress, summary, warnings or notes (``-W`` still counts
       the warnings).
   * - ``--untrusted``
     - Leave out raw content, fetch nothing, and run no build plugin. This is a
       capability policy, not a process sandbox.
   * - ``--budget=MIB``
     - Memory the build may hold at once, from 1 to 1048576 (default: ``1024``).
   * - ``--memory=ram``
     - At the budget, stop without asking. Without it, a terminal asks whether to raise
       the budget or stop; a build without a terminal stops. ``--memory=disk`` and
       ``--disk-budget`` (working memory on disk) are refused with ``cli.memory-disk``.
       Working memory stays in memory, bounded by the budget, so we raise ``--budget``
       instead.
   * - ``--max-depth=N``
     - How deep content may nest (default: ``256``).
   * - ``--max-nodes=N``
     - Nodes per document (default: ``1000000``).
   * - ``--stack-kib=N``
     - Stack for nested content (default: ``512``; at most what the reading threads
       hold, 6144 on Linux and macOS). Raise it together with ``--max-depth``.
   * - ``--typst=PATH``
     - The Typst program for PDF, overriding ``typst`` in ``conf.toml``.

The build prints what it read and wrote, then a verdict: ``build succeeded`` or
``build failed``, with the number of errors and warnings and where the output is. A failed build
leaves the previous output as it was. The warnings of ``conf.toml`` are the build's own,
so ``-W`` counts them. ``suppress_warnings`` silences warnings before ``-W`` counts.

``read`` counts the documents parsed and those unchanged since the last build (taken
from the environment). ``write`` counts the outputs written and those kept. An output is a page
for ``html``, ``dirhtml`` and ``text`` (and the search index in the HTML builders), a
book for ``pdf``, and the single output of ``singlehtml`` and ``gettext``. Every build
also writes the site's shared files (theme, indexes, ``objects.inv``). They are not
counted.

guidedog serve
--------------

Builds the HTML site and serves it on ``http://127.0.0.1:PORT/``. When sources, templates, static files, or ``conf.toml`` changed,
it rebuilds before answering a request:

.. code-block:: sh

   guidedog serve docs
   guidedog serve docs --port 9000

``--port PORT`` (``-p`` is quickstart's project name, not the port) chooses the port,
from 1 to 65535 (default: ``8000``). ``-c``, ``-C``, ``-D name=value`` and the build
options of ``guidedog build`` apply to every rebuild. Each rebuild reloads ``conf.toml``. A
failed rebuild keeps the previous site and reports the problem. Ctrl+C stops the server.

It serves ``_build/html``. It watches the source folder, the Odin and Rust API sources
(``odin_autoapi_dirs``, ``odin_collections``, ``rust_autoapi_crates``), and the
``html_static_path``, ``html_extra_path``, ``templates_path``, ``include_roots``,
``autodoc_source_paths`` and ``locale_dirs`` folders outside the source folder. The
build folder is never watched. Changes are found by file names, sizes and times. The check runs
before an HTML page is answered:

.. code-block:: text

   guidedog building html for Demo
   ...
     Serving _build/html at http://127.0.0.1:8000/ (Ctrl+C to stop)
     Sources changed; rebuilding...
   guidedog building html for Demo
   ...
     Rebuilt.

guidedog clean
--------------

Removes the contents of the project's build folder, the environment cache included:

.. code-block:: sh

   guidedog clean docs

guidedog migrate
----------------

Converts a Sphinx ``conf.py`` (or an old ``guidedog.toml``) to ``conf.toml`` without
running Python. Literal settings carry over. Anything Python would compute is reported
and kept in ``conf.toml`` as a comment:

.. code-block:: sh

   # Write conf.toml beside conf.py
   guidedog migrate docs/conf.py

   # Write elsewhere, or print it
   guidedog migrate docs/conf.py -o docs/conf.toml --force
   guidedog migrate docs -o -

.. list-table:: Options of migrate
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Meaning
   * - ``-o FILE``, ``--output FILE``
     - Write here instead of beside ``conf.py``; ``-`` prints it.
   * - ``--force``
     - Replace an existing ``conf.toml``.

guidedog intl
-------------

Keeps translation catalogs in step with the documents, as ``sphinx-intl`` does. First
we write the message templates with ``guidedog build gettext``. ``update`` then creates
``locales/LANG/LC_MESSAGES/*.po``, or merges new templates into them. Translations stay,
changed messages turn fuzzy, and removed ones become obsolete. To use the catalogs, we build with
``-D language=LANG``, or set ``language`` in ``conf.toml``.

.. code-block:: sh

   guidedog build gettext docs
   guidedog intl update -l de -l fr docs
   guidedog intl stat -l de docs

.. list-table:: Options of intl
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Meaning
   * - ``-l LANG``, ``--language LANG``
     - A language; repeat it for several.
   * - ``-p DIR``, ``--pot-dir DIR``
     - The templates (default: ``_build/gettext``).
   * - ``-d DIR``, ``--locale-dir DIR``
     - The catalogs (default: the first of ``locale_dirs``).
   * - ``-w N``, ``--line-width N``
     - Wrap lines at ``N`` characters (default: ``76``).
   * - ``--no-obsolete``
     - Drop messages the templates no longer have.
   * - ``-c DIR``, ``-D name=value``
     - Where ``conf.toml`` is; override a setting.

``stat`` prints, for each catalog, how many messages are translated, fuzzy, and
untranslated.

guidedog convert
----------------

Converts one file, or standard input, without a project or configuration:

.. code-block:: sh

   guidedog convert guide.rst --output guide.html
   guidedog convert guide.md --to pdf --output guide.pdf
   guidedog convert guide.typ --output guide.pdf
   cat document.md | guidedog convert - --from commonmark --to html --output -
   guidedog convert page.md --from myst --output page.rst

The input format comes from ``--from`` or the input's suffix: ``.rst`` and ``.rest`` are
reStructuredText, ``.md`` and ``.markdown`` CommonMark, ``.typ`` Typst. A Markdown file is
plain CommonMark 0.31.2. ``--from myst`` reads it as MyST with the Sphinx layer instead, so
roles such as ``{doc}`` and ``{ref}`` and directives such as ``{note}`` are understood.
``--from sphinx`` likewise reads reStructuredText with Sphinx's directives, roles, and
domains. The output format comes from ``--to`` or the output's suffix (``.html``,
``.htm``, ``.pdf``, ``.rst``, ``.rest``). Only the options below apply to ``convert``.

``--to rst`` writes reStructuredText with Guidedog's own writer (the ``rst`` renderer).
It keeps the author's lines: each line break inside a source paragraph stays a line break, and the writer adds none of its own, so a converted page keeps the sentence and paragraph shape we wrote.
It never splits inline markup, inline code, a role or a URL across lines.
It uses one section adornment per level (``=``, ``-``, ``~``, ``^``, ...).
It keeps roles, cross-references, and directives as Sphinx writes them. It reads
the input without smart quotes, so quotes and dashes stay as written. Some things
reStructuredText cannot express exactly (a hard line break, an image inside text, a link
title). The writer comes as close as it can and reports a note. Typst input cannot be
converted to reStructuredText.

.. list-table:: Options of convert
   :header-rows: 1
   :widths: 35 65

   * - Option
     - Meaning
   * - ``-o FILE``, ``--output=FILE``
     - The destination, or ``-`` for standard output. Required.
   * - ``--from=rst|sphinx|commonmark|myst|typst``
     - The input format (``restructuredtext``, ``md``, ``markdown`` and ``typ`` are
       accepted spellings).
   * - ``--to=html|pdf|rst``
     - The output format (``restructuredtext`` is an accepted spelling).
   * - ``--emit-typst=FILE``
     - Also write the generated Typst source (not for Typst input).
   * - ``--force``
     - Replace an existing output file.
   * - ``--raw=refuse|omit|allow``
     - Raw HTML or Typst content: fail (default), leave it out, or keep it.
   * - ``--strict``
     - Treat warnings as failures.
   * - ``--title=TEXT``, ``--lang=TAG``
     - The document's title and language.
   * - ``--number-sections``
     - Number section headings.
   * - ``--fragment``
     - Write the body only, not a complete document.
   * - ``--typst=PATH``
     - The Typst program (default: the embedded compiler, else ``typst`` on ``PATH``).
   * - ``--font-path=DIR``
     - An extra font folder; repeat it for more.
   * - ``--embedded-fonts-only``
     - Ignore system fonts, for reproducible PDFs.
   * - ``--offline``
     - Use only Typst packages already on disk.
   * - ``--package-cache=DIR``
     - The Typst package cache folder.
   * - ``--budget=MIB``
     - The memory budget of the conversion (default: ``256``).
   * - ``--max-depth=N``, ``--max-nodes=N``
     - Nesting and node limits.
   * - ``--stack-kib=N``
     - Stack for nested content (default: ``512``, at most ``6144``).

The exit status is 0 for success, 1 for invalid or refused content, 2 for usage, 3 for
a limit, 4 for a file or Typst failure, and 5 for an internal error. A failed conversion
writes no output.

guidedog formats
----------------

Reports the readers and renderers this executable actually has, with their versions and
conformance (``planned``, ``partial``, ``conformant``). It also reports the Typst backend it found
(embedded, or an installed program) and whether it is the pinned version.

.. code-block:: sh

   guidedog formats
   guidedog formats --format=json
   guidedog formats --typst=/opt/typst/bin/typst

guidedog dsc
------------

Keeps a discussion catalog and builds it. The commands come from the discussion plugin, which every ``guidedog`` links. They work on any catalog: its prefix, lifecycle, record types, and numbering are in its ``discussion.toml``. Each command works on the catalog the current folder is in, or the one ``--dir`` names. Guidedog's own design records are one catalog, in ``docs/gds``. :doc:`../howto/discussions` shows the whole workflow.

.. code-block:: sh

   guidedog dsc init DIR --prefix=eds --name="Enso Discussions" --author="Ada"
   guidedog dsc new "Cache policy" --labels=architecture
   guidedog dsc list --state=discussion
   guidedog dsc show cache-policy --format=json
   guidedog dsc promote cache-policy --dry-run
   guidedog dsc state 1 --to=accepted --reason="Review converged"
   guidedog dsc index --check
   guidedog dsc check
   guidedog dsc build --target=both
   guidedog dsc recover --rollback
   guidedog dsc config > discussion.toml

``guidedog dsc build`` builds the catalog as a Guidedog project, as ``guidedog build`` does, with ``pdf_documents`` set to the record books and warnings as errors. ``guidedog dsc config`` prints the catalog's settings in effect, as ``discussion.toml``. ``guidedog dsc --help`` lists the commands; ``guidedog dsc COMMAND --help`` shows one command's options.

.. list-table:: Options of guidedog dsc
   :header-rows: 1
   :widths: 35 65

   * - Option
     - Meaning
   * - ``--dir=DIR``
     - The catalog. By default, the one the current folder is in.
   * - ``--prefix=NAME``
     - The prefix of a catalog without ``discussion.toml``; ``init`` needs it.
   * - ``--name=NAME``, ``--config=FILE``, ``--process=FILE``, ``--template=FILE``
     - The name, settings, process draft, and record template of a new catalog
       (``init``).
   * - ``--author=NAME``, ``--labels=a,b``, ``--slug=NAME``
     - A new draft's author (default: Git's ``user.name``), labels, and file name
       (``new``; ``init`` takes the first two).
   * - ``--state=STATE``, ``--label=NAME``, ``--include-historical``
     - ``list``: filter by state or label; include inherited records.
   * - ``--number=N``, ``--discussion=URL``
     - The number and review link of a promotion; ``--number`` also selects a record to
       build.
   * - ``--to=STATE``, ``--reason=TEXT``, ``--evidence=TEXT``
     - A state change, why, and for ``committed`` the evidence.
   * - ``--check``, ``--render``
     - ``index``: check without writing; ``check``: also build the artifacts.
   * - ``--target=html|pdf|both``
     - What ``build`` writes (default: both). A record selects one PDF.
   * - ``--typst=PATH``
     - ``build`` and ``check``: the Typst program for a catalog of older Typst records
       (default: the embedded compiler), and for the PDF books.
   * - ``--dry-run``, ``--format=table|json``, ``--diagnostics=text|json``, ``-q``
     - Preview a change; machine-readable output; quiet success.

The exit statuses are those of ``guidedog``. Problems about a catalog have codes in its
prefix, such as ``eds.recovery-needed``.

guidedog publish
----------------

Builds the project that a publisher's configuration names, as HTML with Guidedog's own
engine. It freezes the site, and the publisher plans and publishes it:

.. code-block:: sh

   guidedog publish --publisher=pages --config=pages.toml --dry-run
   guidedog publish --publisher=pages --config=pages.toml --wait=600 --timeout=900

The publisher is looked up among the plugins linked into this executable first, then in
the machine's registry (``guidedog plugin register``). A build plugin is refused, because only
publishers publish.

.. list-table:: Options of publish
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Meaning
   * - ``--publisher=ID``
     - The publisher.
   * - ``--config=FILE``
     - The publisher's configuration, which names the project or site.
   * - ``--dry-run``
     - Build and plan; write nothing to the target.
   * - ``--wait=SECONDS``
     - How long to wait for the host to confirm the deployment, from 0 (do not wait)
       to 86400.
   * - ``--timeout=SECONDS``
     - The bound on each whole exchange with the publisher, writing the request
       included, from 1 to 86400 (default: 300). Choose a value longer than ``--wait``.
   * - ``--verbose``
     - List every change of the plan.

A timeout or Ctrl+C after a possible write reports an unknown outcome (exit status 7).
We then run the publisher's recovery command before retrying. Each invocation builds into its
own private folder, which is removed afterwards. Concurrent invocations cannot change
each other's snapshots. See :doc:`../plugins`.

guidedog publish pages
~~~~~~~~~~~~~~~~~~~~~~

``guidedog publish pages`` is the guided way to GitHub Pages; the GitHub Pages plugin adds it. It does whatever is still
missing and asks only when it must. Then it builds and publishes:

#. ``pages.toml``: written once, with the repository taken from the ``origin`` remote and
   the project from the nearest ``conf.toml`` (or as given).
#. A token: GitHub's device sign-in, stored in the system's credential store, or the
   GitHub CLI's token for this run only.
#. The binding: the target is reviewed, and it asks us once to let this machine
   publish there.
#. The Pages source: when Pages publishes another branch, or uses Actions, it
   asks us once to switch it.
#. The publication: one push to the site's own branch, then a wait for the deployment.

On a machine that is already set up, it publishes without a question. Without a terminal
it never asks. A missing step then fails with a hint naming the command or option that does
it. ``--yes`` answers the confirmations. It never signs in or replaces other work on
our behalf. Its options are those of ``guidedog pages init`` and ``publish`` (``--config``,
``--repository``, ``--project``, ``--branch``, ``--profile``, ``--wait``, ``--verbose``,
``--dry-run``); ``guidedog publish pages --help`` lists them. guidedog builds the project itself, with its own engine and linked plugins; ``--guidedog=PATH`` builds with another program instead.
See :doc:`../howto/github-pages`.

guidedog pages workflow
~~~~~~~~~~~~~~~~~~~~~~~

``guidedog pages workflow`` is added by the linked GitHub Pages plugin. It prints, writes, or checks
a GitHub Actions workflow. The workflow installs this Guidedog release, builds the project, and
deploys it with GitHub's Pages actions. We use it instead of publishing from our own machine
when every push to the source branch should deploy.

.. code-block:: sh

   guidedog pages workflow --write
   guidedog pages workflow --check

.. list-table:: Options of pages workflow
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Meaning
   * - ``--write``, ``--force``
     - Write ``.github/workflows/docs-pages.yml`` instead of printing it; replace an
       existing one.
   * - ``--check``
     - Check an existing workflow: an older release, a missing checksum, an unpinned
       action, ``pull_request_target``, a checkout that keeps its credentials.
   * - ``--project=DIR``, ``--branch=NAME``
     - The project (default: ``docs``) and the source branch (default: ``main``).
   * - ``--version=X``, ``--sha256=HEX``, ``--offline``
     - The release to install, its archive's SHA-256 (default: read from the release's
       ``SHA256SUMS``), or a marked placeholder without network access.

See :doc:`../howto/github-pages`.

guidedog plugin
---------------

Records which plugin programs may run on this machine, in ``plugins.toml`` in
Guidedog's configuration folder (``GUIDEDOG_CONFIG_DIR`` moves it):

.. code-block:: sh

   guidedog plugin register product-badge ./target/release/product-badge \
       --project=docs --config=badge.toml
   guidedog plugin check ./target/release/product-badge --project=docs --config=badge.toml
   guidedog plugin list
   guidedog plugin inspect product-badge
   guidedog plugin run pages -- auth status
   guidedog plugin unregister product-badge

Subcommands:

* ``register ID PATH [--project=DIR] [--config=FILE] [--timeout=SECONDS] [--yes]``:
  inspects the program after we confirm (``--yes`` skips the question) and records
  its SHA-256. The program must say it is ``ID``. A build plugin passes the qualification
  checks below before it is registered.
* ``check PATH [--project=DIR] [--config=FILE] [--timeout=SECONDS]``: runs the
  qualification checks against a build plugin without changing the registry. A publisher
  is refused. We test a publisher with its own tests and ``guidedog publish --dry-run``.
* ``list``: the registered programs, their IDs, paths, and kinds, beside the plugins
  linked into this executable.
* ``inspect ID``: an entry's declared build hooks (source, HTML, assets) or publisher
  capabilities, the commands it adds, and whether the program on disk is still the one
  registered.
* ``run ID -- [ARGS...]``: runs a registered program's own commands, after checking its
  SHA-256.
* ``unregister ID``: removes the entry from ``plugins.toml``. The program, and anything
  it published, stay unchanged.

Build plugins are selected by ``plugins`` in ``conf.toml`` and run during
``guidedog build``. ``guidedog publish`` accepts publishers only. ``--untrusted`` disables build
plugins. See :doc:`../plugins` for the complete guide, an example, and configuration.

Build-plugin qualification checks
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``guidedog plugin check PATH --project=DIR --config=FILE`` exercises the declared hooks
in child processes: loading, repeated results, and cleanup. ``--project`` defaults to the
current folder. A relative ``--config`` is relative to it. The request timeout defaults
to 60 seconds. The program's SHA-256 must be the same before and after the checks.
Publisher registration does not invoke publication. Passing checks is interoperability
evidence, not a sandbox or a safety certificate.

Custom binaries with linked plugins
-----------------------------------

The whole command line is the ``gd-cli`` crate. A Rust program can be a ``guidedog`` with
plugins linked in, instead of registered as separate programs. It passes a
``gd_plugins::Registry`` of its plugins to ``gd_cli::run_with``. That function reads the process
arguments and returns the exit status.

.. code-block:: rust

   use std::process::ExitCode;

   fn main() -> ExitCode {
       match gd_plugins::Registry::new(vec![badge::plugin()]) {
           Ok(linked) => gd_cli::run_with(linked),
           Err(problem) => {
               eprintln!("{problem}");
               ExitCode::from(2)
           }
       }
   }

The program accepts every command and option on this page. A selected plugin ID is
looked up among the linked plugins first, then in the machine's registry. Linked plugins
need no registration and no SHA-256 pin, because they are part of the executable.
``gd_cli::run`` is the same with no linked plugins. ``guidedog`` itself
links one, the GitHub Pages plugin. The plugin is the same ``Plugin`` value its installed form serves with
``gd_plugins::serve_plugin`` (see :doc:`../plugins`).

Commands plugins add
~~~~~~~~~~~~~~~~~~~~

A plugin may add commands to guidedog, or targets to ``guidedog publish``. ``guidedog --help`` lists them under PLUGIN COMMANDS, each with the ID of its plugin, and ``guidedog plugin inspect ID`` shows a plugin's:

.. code-block:: console

   $ guidedog --help
     ...
     PLUGIN COMMANDS
       publish pages      Publish to GitHub Pages, doing whatever setup is missing (pages)
       pages workflow     Print, write, or check a GitHub Actions workflow ... (pages)

guidedog looks for a word it does not know (or a ``publish`` target) among the commands of the linked plugins first, then among those of the registered programs, and runs the plugin that adds it. ``--help`` after such a command prints its help, made from what the plugin declares. A plugin cannot replace one of guidedog's commands, and the first plugin to add a name keeps it.

A linked plugin's command runs inside guidedog. A registered program's command runs as that program, started on our terminal after its SHA-256 is checked: it may ask questions, and its exit status is the command's. When either asks for a build, guidedog builds the project. The program's commands are read when it is registered, so a new version is registered again, as always.

guidedog pages
--------------

The linked Pages plugin publishes through ``guidedog publish pages`` and exposes maintenance through ``guidedog pages``. Guidedog builds the project with its own engine. The step commands stay for scripts and for checking
each part. See :doc:`../howto/github-pages`.

.. code-block:: sh

   guidedog publish pages
   guidedog pages init --repository=OWNER/REPO --project=docs
   guidedog pages auth import --profile=default
   guidedog pages auth login --client-id=APP_ID --scope=public
   guidedog pages auth status
   guidedog pages bind --adopt
   guidedog pages setup
   guidedog pages doctor
   guidedog publish pages --dry-run
   guidedog publish pages --wait=600
   guidedog pages watch
   guidedog pages status --operation=COMMIT
   guidedog pages recover
   guidedog pages unbind
   guidedog pages auth logout
   guidedog pages workflow --write

.. list-table:: Settings of pages.toml
   :header-rows: 1
   :widths: 30 70

   * - Setting
     - Meaning
   * - ``schema = 1``
     - The file's version; required.
   * - ``repository = "OWNER/REPO"``
     - The GitHub.com repository; required.
   * - ``branch = "gh-pages"``
     - The branch the site is published to: its own, never the default branch.
   * - ``project = "docs"`` or ``site = "build/site"``
     - The Guidedog project to build, or the folder of a site built already; one of
       them, relative to ``pages.toml``.
   * - ``profile = "default"``
     - The credential profile whose token publishes.
   * - ``site_url``
     - The address readers open, and ``html_baseurl`` for the build (default:
       ``https://OWNER.github.io/REPO/``).
   * - ``strict = true``
     - Build with warnings as errors.
   * - ``wait_seconds = 300``
     - How long ``publish`` waits for the deployment.
   * - ``[watch] debounce_ms``, ``max_wait_ms``
     - How long ``watch`` lets sources settle, and the most it waits after a change.

.. list-table:: Options of guidedog pages
   :header-rows: 1
   :widths: 35 65

   * - Option
     - Meaning
   * - ``--config=FILE``
     - The configuration (default: ``pages.toml``).
   * - ``--profile=NAME``
     - The profile ``auth`` and ``init`` use (default: ``default``).
   * - ``--repository=OWNER/REPO``, ``--project=DIR``, ``--site=DIR``, ``--branch=NAME``
     - ``init``: what the new ``pages.toml`` says.
   * - ``--credential=env``
     - Use the token in ``GUIDEDOG_PAGES_TOKEN`` for this command, without storing it.
   * - ``--guidedog=PATH``
     - The ``guidedog`` that builds (default: ``GUIDEDOG_PAGES_GUIDEDOG``, then ``PATH``).
   * - ``--dry-run``, ``--wait=SECONDS``, ``--verbose``
     - ``publish``: plan only; how long to wait for the deployment; list every change.
   * - ``--adopt``, ``--yes``
     - ``bind``: let a branch of other work be replaced; answer yes to the question.
   * - ``--client-id=ID``, ``--scope=public|private``, ``--open``, ``--replace``
     - ``auth login``: the app; the repositories an OAuth App token may reach; open the
       browser; give the profile to another account.
   * - ``--operation=COMMIT``
     - ``status``: the publication to ask about (default: the last one).
   * - ``--diagnostics=text|json``, ``--color=auto|always|never``
     - How problems and results are written.

Its exit statuses are those of ``guidedog publish``: 0 live or unchanged, 1 nothing
published, 2 a usage or configuration problem, 4 a file, network or program failure,
6 accepted but not confirmed, 7 unknown until ``recover``, and 130 cancelled.
