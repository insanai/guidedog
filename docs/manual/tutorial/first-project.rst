Your first project
==================

This tutorial guides you through creating, writing, and building your first
documentation project with Guidedog. The goal is modest and clear: start from a clean
directory, generate both an HTML website and a printable PDF book from the same source
file, and experience the live preview server.

You need a working installation of Guidedog. For PDF output, Guidedog can use its
embedded compiler or an installed Typst binary (0.15.1 or newer). See :doc:`../howto/install`.

On macOS, you can install the official release using Homebrew:

.. code-block:: sh

   brew install insanai/guidedog/guidedog

Elsewhere, build it from a source checkout with the Rust toolchain, and put
``target/release/guidedog`` on your ``PATH``:

.. code-block:: sh

   cargo build --release -p guidedog-cmd
   target/release/guidedog --version

Project architecture and build pipeline
---------------------------------------

Guidedog takes a single directory of source files and compiles it into two distinct
artifacts: a responsive HTML website and a typeset PDF book.

.. graphviz::
   :class: graphviz-adaptive
   :caption: How Guidedog compiles a project into an HTML website and a PDF book.
   :alt: Project sources and templates pass through the Guidedog compiler to produce HTML and PDF artifacts.

   digraph first_project {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];
     source [class="gd-input", shape=note, fillcolor="#fbfaf3",
       label=<<B>Project directory</B><BR/><FONT POINT-SIZE="11">conf.toml<BR/>index.rst + chapters<BR/>_templates/ + _static/</FONT>>];
     engine [label=<<B>Guidedog build</B><BR/><FONT POINT-SIZE="11">One document graph</FONT>>];
     html [class="gd-output", shape=note, fillcolor="#eef4fa",
           label=<<B>HTML website</B><BR/><FONT POINT-SIZE="11">_build/html/</FONT>>];
     pdf [class="gd-output", shape=note, fillcolor="#eef4fa",
          label=<<B>PDF book</B><BR/><FONT POINT-SIZE="11">_build/pdf/</FONT>>];
     source -> engine;
     engine -> html [label="html"];
     engine -> pdf [label="pdf"];
   }

1. Create the project
---------------------

Run the ``quickstart`` command from a folder where the target directory ``notes`` does
not yet exist:

.. code-block:: sh

   guidedog quickstart notes -q -p "Field Notes" -a "Your name" -v 1.0

Guidedog creates a complete, self-contained project structure:

.. code-block:: text

   notes/
     conf.toml
     index.rst
     _templates/
       layout.html
       book.typ
     _static/
       guidedog.css
       guidedog.js
     assets/
       images/
       audio/
       video/

Understanding the files:

* ``conf.toml``: Project settings (title, author, version, language, builders).
* ``index.rst``: The root document that defines your front page and table of contents.
* ``_templates/layout.html``: The HTML website template (Jinja).
* ``_templates/book.typ``: The PDF book layout and typography template (Typst).
* ``_static/``: CSS stylesheets and JavaScript enhancements.
* ``assets/``: Dedicated folders for project images, diagrams, audio, and video files.

Templates in Guidedog are directly visible and editable. They are placed directly inside
your project rather than hidden within an external theme or system cache. You can modify
them at any time to customize your site and book design.
The first build creates ``_build`` for generated output.

2. Write the first page
-----------------------

Open ``notes/index.rst`` in your editor and replace its contents with the following:

.. code-block:: rst

   Field Notes
   ===========

   A clear explanation begins with a simple example.

   Measuring a square
   ------------------

   If the side has length :math:`s`, its area is :math:`s^2`.

   .. math::

      A = s^2

   .. note::

      State the measurement units before comparing calculations.

This page demonstrates headings, mathematical notation, and an admonition note.

3. Build and inspect the outputs
--------------------------------

Compile your project into both HTML and PDF formats:

.. code-block:: sh

   guidedog build html notes -W
   guidedog build pdf notes -W

The ``-W`` flag instructs Guidedog to treat warnings as errors. Keeping this flag enabled
in your routine checks ensures that broken references or markup errors are caught early.

Now inspect the output files:

* Open ``notes/_build/html/index.html`` in your web browser. You will see a clean, responsive
  documentation website with a theme switch, search bar, and formatted math.
* Open the PDF file generated in ``notes/_build/pdf/``. You will see a book layout
  complete with running headers, page numbers, and typographic math.

Guidedog formats mathematics appropriately for each medium: using client-side MathJax for
the browser, and native vector formulas for the Typst PDF engine.

4. Live preview and incremental rebuilds
----------------------------------------

Guidedog includes a built-in development web server:

.. code-block:: sh

   guidedog serve notes

The terminal prints a local URL, such as ``http://localhost:8000``. Open this address in
your browser.

Now test the live editing loop:

.. graphviz::
   :class: graphviz-adaptive
   :caption: The live development cycle with guidedog serve.
   :alt: Editing and saving a file triggers an input check and a selective rebuild upon refreshing the browser.

   digraph live_preview {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.35", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     edit [class="gd-input", label=<<B>1. Edit Source</B><BR/><FONT POINT-SIZE="11">(index.rst or layout.html)</FONT>>, fillcolor="#fbfaf3"];
     save [class="gd-input", label=<<B>2. Save File</B><BR/><FONT POINT-SIZE="11">(The source changes)</FONT>>, fillcolor="#fbfaf3"];
     req [label=<<B>3. Refresh Browser</B><BR/><FONT POINT-SIZE="11">(HTTP request sent)</FONT>>];
     detect [label=<<B>4. Check Inputs</B><BR/><FONT POINT-SIZE="11">(guidedog serve inspects sources)</FONT>>];
     rebuild [label=<<B>5. Incremental Rebuild</B><BR/><FONT POINT-SIZE="11">(Only changed pages re-rendered)</FONT>>, fillcolor="#e0eee9"];
     serve [label=<<B>6. Return Fresh HTML</B><BR/><FONT POINT-SIZE="11">(Updated page displayed)</FONT>>, fillcolor="#eef6f2"];

     edit -> save -> req -> detect -> rebuild -> serve;
   }

1. Edit ``notes/index.rst`` to add a new paragraph.
2. Save the file.
3. Refresh your browser page.

Before serving a requested page, Guidedog checks the project's inputs (sources,
templates, static files and configuration) against its saved build environment. If an
input has changed, it rebuilds only the affected documents before responding to the
browser request. A failed rebuild keeps the previous site and reports the problem in the
terminal.

When you run a build without modifying any files, Guidedog reuses the saved document
trees, avoiding redundant re-parsing. See :doc:`../topics/projects` for how it decides.

Press ``Ctrl+C`` in your terminal to stop the server when you are done.

Continue to :doc:`a-small-book` to learn how to organize multiple chapters and cross-references.
