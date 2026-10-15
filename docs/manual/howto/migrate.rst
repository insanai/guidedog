Move a Sphinx project to Guidedog
=================================

Guidedog follows Sphinx closely. It uses the same directory
structure and understands standard reStructuredText and MyST Markdown markup. It recognizes
the same configuration setting names and provides Sphinx's Jinja template variables.

One difference shapes the whole move. Sphinx runs arbitrary
Python code in ``conf.py`` and needs a Python virtual environment with external packages.
Guidedog is a single compiled program that reads plain data
from ``conf.toml``. It does not run Python.

So moving a project means separating configuration data from
Python code.

Migration workflow
------------------

The move goes through these steps:

.. graphviz::
   :class: graphviz-adaptive
   :caption: The step-by-step workflow for migrating a Sphinx project to Guidedog.
   :alt: conf.py is converted by guidedog migrate into conf.toml, followed by semantic verification, custom object mapping, and visual inspection.

   digraph sphinx_migration {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     conf_py [class="gd-input", label=<<B>1. Sphinx Project</B><BR/><FONT POINT-SIZE="11">conf.py + docs/</FONT>>, shape=note, fillcolor="#fbfaf3"];
     migrate_cmd [label=<<B>2. Run guidedog migrate</B><BR/><FONT POINT-SIZE="11">Static AST translation without Python<BR/>runtime</FONT>>, fillcolor="#edf5f2"];
     conf_toml [class="gd-input", label=<<B>3. Review conf.toml</B><BR/><FONT POINT-SIZE="11">Literal settings mapped; dynamic scripts<BR/>reported</FONT>>, shape=note, fillcolor="#fbfaf3"];
     dry_run [label=<<B>4. Semantic Validation</B><BR/><FONT POINT-SIZE="11">guidedog build dummy docs -n -W (fix broken<BR/>links)</FONT>>, fillcolor="#edf5f2"];
     adjust [label=<<B>5. Map Extensions and Objects</B><BR/><FONT POINT-SIZE="11">Built-in domains, autodoc, and declarative<BR/>types</FONT>>, fillcolor="#e0eee9"];
     build_pub [label=<<B>6. Full Production Build</B><BR/><FONT POINT-SIZE="11">guidedog build html and pdf (verify visual<BR/>styling)</FONT>>, fillcolor="#eef6f2"];

     conf_py -> migrate_cmd -> conf_toml -> dry_run -> adjust -> build_pub;
   }

Step 1: Automated configuration migration
-----------------------------------------

Guidedog has a migration tool that parses our existing ``conf.py``:

.. code-block:: sh

   guidedog migrate path/to/conf.py

What ``guidedog migrate`` does:

1. It extracts literal configuration values (strings, numbers, booleans, lists, and tables)
   and converts them to standard TOML syntax in ``conf.toml``.
2. It detects standard Sphinx settings (such as ``project``, ``author``, ``extensions``,
   ``templates_path``, ``html_theme``, ``intersphinx_mapping``, and ``html_context``) and
   maps them.
3. It finds computed expressions (such as Python calls like
   ``version = get_version()`` or ``git_revision = subprocess.check_output(...)``).
   Guidedog does not run Python, so its migration report flags these values.
   We then give fixed values for them in ``conf.toml``.

We review the generated ``conf.toml`` and replace each flagged expression with a fixed value.

Step 2: The three-pass verification strategy
--------------------------------------------

Once ``conf.toml`` exists, we work through the migration in three passes:

Pass 1: Verify the document graph
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

We check that Guidedog finds all documents and builds the expected hierarchy:

* Verify that ``root_doc`` names our main entry point (usually ``index`` or ``contents``).
* Confirm that ``source_suffix`` lists every extension we use (for example, ``[".rst", ".md"]``).
* Check that ``exclude_patterns`` excludes build folders, virtual environments, and temporary files.
* Run a quick semantic check using the dummy builder:

  .. code-block:: sh

     guidedog build dummy path/to/docs -n -W

  The dummy builder checks every document and link without writing HTML files. We fix each
  warning about a missing document or a malformed include.

Pass 2: Verify references, domains, and custom objects
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Next, we verify that cross-references and domain directives resolve:

* **Standard domains:** Guidedog supports the standard Python, C, C++, and JavaScript
  domains, with their roles such as ``:py:func:``, ``:c:type:``, and ``:cpp:class:``.
  It adds ``odin`` and ``rust`` domains of its own (see :doc:`../topics/domains`).
* **Intersphinx links:** If our Sphinx project linked to external documentation through Intersphinx,
  we keep our ``[intersphinx_mapping]`` in ``conf.toml``. Guidedog fetches and parses standard
  ``objects.inv`` files itself.
* **Declarative custom object types:** In Sphinx, projects often wrote small Python extensions
  that call ``app.add_object_type()`` to create custom directives and roles (such as
  ``.. confval::`` or ``.. setting::``). In Guidedog, we declare these in
  ``conf.toml`` under ``[[object_types]]``, without code. See :doc:`../object-types`.

Pass 3: Verify presentation and templates
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Finally, we check how the output looks:

* **HTML website:** Run ``guidedog build html path/to/docs -W`` and inspect the generated pages.
* **Jinja templates:** Guidedog's native Jinja engine supports block inheritance:

  .. code-block:: html+jinja

     {% extends "!layout.html" %}
     {% block extrahead %}
       <link rel="stylesheet" href="{{ pathto('_static/custom.css', 1) }}">
     {% endblock %}

* **PDF books:** Build a PDF book with ``guidedog build pdf path/to/docs -W``. Guidedog typesets
  the book with Typst and ``_templates/book.typ``. It gives vector math, proper
  hyphenation, and running headers without a LaTeX installation.

Configuration settings mapping
------------------------------

Common Sphinx ``conf.py`` variables correspond to Guidedog's ``conf.toml`` like this:

.. list-table:: Common configuration mapping
   :header-rows: 1
   :widths: 35 35 30

   * - Sphinx (conf.py)
     - Guidedog (conf.toml)
     - Notes
   * - ``project = 'Manual'``
     - ``project = "Manual"``
     - Standard string setting.
   * - ``version = '1.0'``
     - ``version = "1.0"``
     - Documented version number.
   * - ``extensions = [...]``
     - ``extensions = [...]``
     - Built-in extensions (graphviz, mathjax, intersphinx, myst_parser).
   * - ``source_suffix = ['.rst', '.md']``
     - ``source_suffix = [".rst", ".md"]``
     - Supported source formats.
   * - ``root_doc = 'index'``
     - ``root_doc = "index"``
     - Entry document name (without extension).
   * - ``html_static_path = ['_static']``
     - ``html_static_path = ["_static"]``
     - Custom styles and script folders.
   * - ``templates_path = ['_templates']``
     - ``templates_path = ["_templates"]``
     - Jinja template directory.
   * - ``html_context = {...}``
     - ``[html_context]``
     - Passed directly to Jinja templates.

Handling custom Python extensions
---------------------------------

Some Sphinx projects rely on large Python extensions that inspect Python ASTs
or modify Sphinx's internal classes. For those, we have three options:

1. **Declarative object types:** If the extension only created custom directives or roles,
   we replace it with ``[[object_types]]`` in ``conf.toml``.
2. **Build plugins:** If the extension transformed source text, added custom HTML
   wrappers, or generated static assets, we write a build plugin in Rust with the
   ``gd-plugins`` crate's typed interface described in :doc:`../plugins`. The same plugin
   runs as an installed program (``guidedog plugin register``) or linked into our own
   ``guidedog`` binary through ``gd_cli::run_with``.
3. **Static API extraction:** Guidedog already reads Python docstrings, Odin declarations,
   and Rust doc comments without running the documented code. Use :doc:`document-code`.
   Runtime-generated objects may need explicit RST descriptions.

:doc:`../compatibility` gives the evidence for Sphinx compatibility and
the limits that remain.
