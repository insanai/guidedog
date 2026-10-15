Build and install Guidedog
==========================

Install on macOS with Homebrew
------------------------------

Use the official ``insanai/homebrew-guidedog`` tap on Apple silicon or Intel Macs:

.. code-block:: sh

   brew tap insanai/guidedog
   brew install insanai/guidedog/guidedog
   guidedog --version

Homebrew selects the release archive for our processor and checks its SHA-256.
The executable has the Typst compiler built in and links Graphviz, which Homebrew
installs with it. We do not need a Rust toolchain, a Python environment, or a separate
Typst installation to build HTML and PDF. The release requires macOS 12 or later.

The package also installs two more programs. ``dsc`` keeps discussion catalogs and builds them with
this ``guidedog`` (see :doc:`discussions`). ``guidedog-pages`` publishes sites
to GitHub Pages (see :doc:`github-pages`).

We create our first project in :doc:`../tutorial/first-project`.
To update an installed release:

.. code-block:: sh

   brew update
   brew upgrade insanai/guidedog/guidedog

Run ``brew test insanai/guidedog/guidedog`` to check the installed package by
creating a project and building HTML and PDF.

Build from source
-----------------

Guidedog is a Cargo workspace. We need:

* a Rust toolchain at the version ``rust-version`` names in the root ``Cargo.toml``
  (Rust 1.90 or newer; edition 2024), installed with ``rustup``;
* Graphviz's development files, because Guidedog links the Graphviz library
  (``libgvc`` and ``libcgraph``) to draw graphs in process: ``libgraphviz-dev`` on
  Debian and Ubuntu, ``graphviz-devel`` on Fedora, ``graphviz`` from Homebrew on macOS;
* ``curl`` on the ``PATH`` for intersphinx inventories and the GitHub Pages publisher,
  which run the ``curl`` program instead of linking an HTTP library.

Nothing else is built by hand. The Typst compiler (0.15.1) and the tree-sitter grammars
for Python and Rust are ordinary Cargo dependencies, compiled with the workspace. There
are no native build scripts to run first.

Release build
-------------

From the repository root:

.. code-block:: sh

   cargo build --release -p guidedog-cmd
   target/release/guidedog --version
   target/release/guidedog formats

The executable is ``target/release/guidedog`` (``guidedog.exe`` on Windows). To run
``guidedog`` without its path, we put it on our ``PATH`` or install it into
Cargo's binary folder:

.. code-block:: sh

   cargo install --path cmd/guidedog

``guidedog formats`` lists the readers and renderers built in, and the Typst backend it
found.

Typst: embedded or installed
----------------------------

By default the Typst compiler is linked into ``guidedog`` (the ``embedded`` feature of
the ``gd-typst`` crate). PDF then needs no other program. A build of ``gd-typst`` without
that feature runs an installed ``typst`` program (0.15.1) instead. Either way,
``--typst=/absolute/path/to/typst`` (or ``typst`` in ``conf.toml``) selects a program
explicitly, and ``guidedog --version`` names the backend in use.

The embedded compiler removes the need to start a separate process. It does not remove
the compiler's own memory costs. ``--budget`` bounds what Guidedog itself holds.

Development checks
------------------

Before we propose a change, we run the same checks as the project's continuous integration:

.. code-block:: sh

   cargo build --workspace
   cargo test --workspace
   cargo clippy --workspace --all-targets
   cargo run -p check

``cargo run -p check`` is the source-limit checker in ``tools/check``: function and file
lengths and line widths.

Discussion programs
-------------------

``dsc`` and ``dscc`` build the same way:

.. code-block:: sh

   cargo build --release -p dsc-cmd
   cargo build --release -p dscc-cmd

``dsc`` is small: it keeps the catalog and runs an installed ``guidedog`` to build it.
``dscc`` links Guidedog, so it needs what ``guidedog`` needs (Graphviz's library).
Each release also has a ``dscc`` archive for every platform, for a machine with nothing
else installed.

The GitHub Pages publisher builds the same way. It does not link the engine:

.. code-block:: sh

   cargo build --release -p guidedog-pages-cmd

It keeps tokens in the system's credential store: the Keychain on macOS, the Credential
Manager on Windows, and on Linux the Secret Service through the ``secret-tool`` program
(``libsecret-tools``). It needs that program only when it first stores or reads a token.

Embed the engine
----------------

The conversion engine is a library. A Rust program can convert text without the
``guidedog`` command, through the ``guidedoc`` crate and the reader and renderer crates
it needs:

.. code-block:: toml

   [dependencies]
   guidedoc = { path = "crates/guidedoc" }
   guidedoc-commonmark = { path = "crates/guidedoc-commonmark" }
   guidedoc-html = { path = "crates/guidedoc-html" }

.. code-block:: rust

   use guidedoc::{Registry, Workspace, convert_text};

   fn main() {
       let registry = Registry {
           readers: vec![guidedoc_commonmark::READER],
           renderers: vec![guidedoc_html::renderer()],
       };
       let mut ws = Workspace::new(); // reuse it: its buffers are kept
       match convert_text(&mut ws, &registry, "commonmark", "html", "# Hello\n") {
           Ok(html) => println!("{html}"),
           Err(fail) => eprintln!("{fail}"), // title, message, and hint
       }
   }

``guidedoc_rst::reader()``, ``guidedoc_commonmark::MYST_READER``,
``guidedoc_typst::renderer()`` and ``guidedoc_text::renderer()`` join the registry the
same way. For code highlighting, the ``guidedoc-defaults`` crate joins the highlighter to
the renderers. We pass a ``guidedoc_defaults::HtmlHighlighter`` (or ``TypstHighlighter``) as
``highlighter`` in the request's ``RenderOptions``. The staged calls
(``read_document``, ``transform_document``, ``check_render``, ``render_document``), the
request fields, and the limits are in the crate's ``README.md``.
