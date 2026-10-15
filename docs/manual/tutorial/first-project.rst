Our first project
=================

In this tutorial we create, write, and build our first
documentation project with Guidedog. The goal is modest and clear. We start from an empty
directory. We make an HTML website and a printable PDF book from the same source
file. Then we try the live preview server.

We need a working installation of Guidedog. For PDF output, Guidedog can use its
embedded compiler or an installed Typst binary (0.15.1 or newer). See :doc:`../howto/install`.

On macOS, we can install the official release with Homebrew:

.. code-block:: sh

   brew install insanai/guidedog/guidedog

Elsewhere, we build it from a source checkout with the Rust toolchain, and put
``target/release/guidedog`` on our ``PATH``:

.. code-block:: sh

   cargo build --release -p guidedog-cmd
   target/release/guidedog --version

Project architecture and build pipeline
---------------------------------------

Guidedog takes one directory of source files and compiles it into two
outputs: a responsive HTML website and a typeset PDF book.

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

We run the ``quickstart`` command from a folder where the directory ``notes`` does
not yet exist:

.. code-block:: sh

   guidedog quickstart notes -q -p "Field Notes" -a "Your name" -v 1.0

Guidedog creates a complete, self-contained project:

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

The files are:

* ``conf.toml``: Project settings (title, author, version, language, builders).
* ``index.rst``: The root document. It defines our front page and table of contents.
* ``_templates/layout.html``: The HTML website template (Jinja).
* ``_templates/book.typ``: The PDF book layout and typography template (Typst).
* ``_static/``: CSS stylesheets and JavaScript enhancements.
* ``assets/``: Folders for project images, diagrams, audio, and video files.

Templates in Guidedog are ordinary files that we can see and edit. They live inside
our project, not in an external theme or a system cache. We can change
them at any time to adjust the design of our site and book.
The first build creates ``_build`` for generated output.

2. Write the first page
-----------------------

We open ``notes/index.rst`` in an editor and replace its contents with the following:

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

This page shows headings, mathematical notation, and a note.

3. Build and inspect the outputs
--------------------------------

We compile our project into both HTML and PDF:

.. code-block:: sh

   guidedog build html notes -W
   guidedog build pdf notes -W

The ``-W`` flag tells Guidedog to treat warnings as errors. With this flag in
our routine checks, broken references and markup errors are caught early.

Now we inspect the output files:

* We open ``notes/_build/html/index.html`` in a web browser. We see a clean, responsive
  documentation website with a theme switch, search bar, and formatted math.
* We open the PDF file in ``notes/_build/pdf/``. We see a book layout
  with running headers, page numbers, and typeset math.

Guidedog formats mathematics for each medium. The browser gets MathJax, and
the PDF gets Typst's own vector formulas.

4. Live preview and incremental rebuilds
----------------------------------------

Guidedog includes a built-in development web server:

.. code-block:: sh

   guidedog serve notes

The terminal prints a local URL, such as ``http://localhost:8000``. We open this address in
a browser.

Now we test the live editing loop:

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
3. Refresh the browser page.

Before serving a requested page, Guidedog checks the project's inputs (sources,
templates, static files and configuration) against its saved build environment. If an
input has changed, it rebuilds only the affected documents before it answers the
browser request. A failed rebuild keeps the previous site and reports the problem in the
terminal.

When we run a build without changing any files, Guidedog reuses the saved document
trees and parses nothing again. See :doc:`../topics/projects` for how it decides.

We press ``Ctrl+C`` in the terminal to stop the server when we are done.

Next, :doc:`a-small-book` shows how to organize several chapters and cross-references.
