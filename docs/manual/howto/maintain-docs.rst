Maintain this documentation
===========================

Guidedog documents itself with Guidedog. The repository has four projects:
``docs/manual`` (this manual), ``docs/api`` (the Rust API reference of the crates),
``docs/gds`` (the design records), and ``docs/site`` (the front page).
The manual and GDS began with ``guidedog quickstart``. We edit their generated templates in
place. Design records stay out of the manual's table of contents.

All documentation is reStructuredText under ``docs/``. The only Markdown files are each
crate's ``README.md``, a short cheat sheet, and the files GitHub needs at the root.
Design notes, internals, ledgers and conformance tables are pages under
``docs/manual/internals``.

Build the manual
----------------

We build ``guidedog`` first (see :doc:`install`). Then, from the repository root:

.. code-block:: sh

   cargo build --release -p guidedog-cmd
   target/release/guidedog build html docs/manual -j auto -W
   target/release/guidedog build pdf docs/manual -W
   target/release/guidedog serve docs/manual

The PDF command creates one book, with links between its chapters.
``cargo run --release -p guidedog-cmd -- build html docs/manual -W`` builds and runs in
one step.

The API reference
-----------------

``docs/api`` generates its pages from the crates' source with ``rust_autoapi_crates``
(see :doc:`document-code`). The reference is the doc comments themselves (``///`` on every
public item). There is no copied API manuscript to keep in sync. We fix a mistake in the
crate, then rebuild:

.. code-block:: sh

   target/release/guidedog build html docs/api -W
   target/release/guidedog build pdf docs/api -W

A warning such as ``rust.autodoc.link`` names the doc comment and the link that does not
resolve.

Build and check discussions
---------------------------

.. code-block:: sh

   cd docs/gds
   ../../target/release/guidedog dsc index
   ../../target/release/guidedog dsc check
   ../../target/release/guidedog dsc build --target=both

``docs/gds`` is a discussion catalog like any other, so ``guidedog dsc`` runs in it (see :doc:`discussions`).
It manages metadata, numbers, state, and recovery.
Its RST publication path calls the project engine in process.
It produces a complete HTML site and a separate PDF for each record.
A selector limits the PDFs. HTML remains a coherent site with a complete catalog.
A selected PDF preview uses ``_build/selected/<artifact>/pdf``.
It leaves the complete catalog in ``_build/pdf`` intact.

.. code-block:: sh

   target/release/guidedog dsc build guidedog-in-rust --target=pdf --dir=docs/gds
   target/release/guidedog build html docs/gds -W
   target/release/guidedog build pdf docs/gds -W

The ordinary project commands work too.
The GDS project uses its own ``conf.toml`` and its own ``_build`` directory.
Read ``docs/gds/workflow.rst`` for the authoring workflow.

Translations
------------

The manual's catalogs are in ``docs/manual/locales``. After we change English text,
we refresh them:

.. code-block:: sh

   target/release/guidedog build gettext docs/manual
   target/release/guidedog intl update -l de -l es -l hi -l ja -l ko -l zh_CN docs/manual

See :doc:`../intl` for the translation workflow.

Before publishing
-----------------

Check links and diagnostics. Inspect wide tables and code blocks.
Render the PDF pages after changing typography.
Test the HTML at desktop and phone widths, in light and dark modes.
Make sure the mathematics and diagrams work without a network connection.
For engine changes, run ``cargo test --workspace``,
``cargo clippy --workspace --all-targets`` and ``cargo run -p check``.
