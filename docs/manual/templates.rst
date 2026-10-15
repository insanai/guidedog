.. _templates:

Templates
=========

A template controls presentation. It does not duplicate the source prose.
Guidedog uses Jinja for HTML and Typst for PDF.
For a complete worked project, follow :doc:`tutorial/a-custom-theme`.
This chapter is the reference for its settings, blocks and parameters.

Templates in Guidedog are ordinary project files. We can see them and edit them.
When we start a project with ``guidedog quickstart``, it writes the working
templates into our project:

* ``_templates/layout.html`` defines the HTML website structure (Jinja).
* ``_templates/book.typ`` defines the PDF book layout, typography, and cover page (Typst).
* ``_static/guidedog.css`` and ``_static/guidedog.js`` supply default styles and client behaviors.

Templates live in the project directory. They are not hidden in an external
package or binary cache. We edit them directly, rebuild with ``guidedog build``,
and preview changes with ``guidedog serve``.

HTML templates
--------------

Guidedog renders ``layout.html`` as the root page template using a built-in Jinja engine.
Split a layout into partials and macro libraries. Inherit shared blocks from a base
layout. Each file then has one job.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Template inheritance and block composition in Guidedog.
   :alt: Built-in theme layout is extended by project templates with block overrides and partial includes.

   digraph template_inheritance {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];
     builtin [label=<<B>Built-in layout</B><BR/><FONT POINT-SIZE="11">!layout.html</FONT>>];
     layout [label=<<B>Project layout.html</B><BR/><FONT POINT-SIZE="11">Override named blocks</FONT>>];
     parts [class="gd-input", shape=note, fillcolor="#fbfaf3",
            label=<<B>Reusable components</B><BR/><FONT POINT-SIZE="11">include partials<BR/>import macros</FONT>>];
     page [class="gd-output", fillcolor="#eef4fa",
           label=<<B>Complete HTML page</B><BR/><FONT POINT-SIZE="11">Content + navigation + assets</FONT>>];
     builtin -> layout [label="extends"];
     layout -> parts [label="composes"];
     parts -> page [label="renders"];
   }

Modular template structure
~~~~~~~~~~~~~~~~~~~~~~~~~~

A project can organize templates across any number of files and subdirectories under
the directories listed in ``templates_path`` (by default, ``["_templates"]``):

.. code-block:: text

   my-docs/
   |-- conf.toml
   |-- index.rst
   `-- _templates/
       |-- layout.html
       |-- base.html
       |-- partials/
       |   |-- header.html
       |   |-- navigation.html
       |   |-- searchbox.html
       |   `-- footer.html
       `-- macros/
           `-- components.html

Guidedog scans all files and subdirectories within ``templates_path``. Any file or
partial can be included or imported using paths relative to ``templates_path``:

.. code-block:: html+jinja
   :caption: _templates/layout.html

   {% extends "base.html" %}

   {% block header %}
     {% include "partials/header.html" %}
   {% endblock %}

   {% block footer %}
     {% include "partials/footer.html" %}
   {% endblock %}

Jinja macros can also be defined in separate files and imported wherever needed:

.. code-block:: html+jinja
   :caption: _templates/macros/components.html

   {% macro badge(label, type="info") %}
     <span class="badge badge-{{ type }}">{{ label }}</span>
   {% endmacro %}

.. code-block:: html+jinja
   :caption: Using macros in any template

   {% import "macros/components.html" as ui %}
   {{ ui.badge("New", type="success") }}

Theme inheritance with ``!layout.html``
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Sometimes we want to change only some sections of the default theme, not write
the whole page structure from scratch. Then we inherit from Guidedog's built-in
theme layout with Sphinx's exclamation mark syntax:

.. code-block:: html+jinja
   :caption: _templates/layout.html

   {# Inherit Guidedog's built-in layout #}
   {% extends "!layout.html" %}

   {# Inject custom metadata or web fonts into the document head #}
   {% block extrahead %}
     {{ super() }}
     <link rel="stylesheet" href="{{ pathto('_static/custom.css', 1) }}">
   {% endblock %}

   {# Replace or extend the footer with custom content #}
   {% block footer %}
     {% include "partials/footer.html" %}
   {% endblock %}

``{% extends "!layout.html" %}`` (or ``{% extends "basic/layout.html" %}``) tells
the loader to fetch the theme's underlying template. Without the mark, it would load the project's
own ``_templates/layout.html`` again, recursively.

Inside any overridden block, ``{{ super() }}`` renders the parent block's default
markup. So we can add markup before or after it without copying existing code.

Layout blocks
~~~~~~~~~~~~~

Guidedog's built-in ``layout.html`` defines blocks matching Sphinx's conventions:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Block
     - Purpose
   * - ``doctype``
     - Document type declaration (default: ``<!DOCTYPE html>``).
   * - ``htmltitle``
     - The ``<title>`` element inside ``<head>``.
   * - ``linktags``
     - Navigation and meta link tags (``favicon``, ``index``, ``search``, ``prev``, ``next``).
   * - ``css``
     - Stylesheet link tags and inline CSS root variables.
   * - ``scripts``
     - JavaScript tags, including search indexing and interactive features.
   * - ``extrahead``
     - Empty insertion point at the end of ``<head>`` for analytics, fonts, or custom meta tags.
   * - ``header``
     - Header slot rendered immediately before the main navigation bar.
   * - ``relbar1``
     - Top navigation bar containing branding, version badge, search box, and theme toggle.
   * - ``rootrellink``
     - Insertion point in navigation bar before related links.
   * - ``relbaritems``
     - Custom items inserted into the navigation bar.
   * - ``sidebar1``
     - Left-hand navigation sidebar container.
   * - ``sidebartoc``
     - Table of contents navigation tree rendered inside ``sidebar1``.
   * - ``breadcrumbs``
     - Hierarchical breadcrumb navigation path above the article content.
   * - ``document``
     - Article wrapper enclosing the page body.
   * - ``body``
     - Rendered HTML content of the current document (``{{ body }}``).
   * - ``relbar2``
     - Bottom pagination controls offering previous and next chapter links.
   * - ``footer``
     - Page footer containing copyright notice, last updated timestamp, and source links.
   * - ``sidebar2``
     - Right-hand secondary rail containing the local page outline ("On this page").

Finding templates
~~~~~~~~~~~~~~~~~

Guidedog searches each directory listed in ``conf.toml`` under ``templates_path`` in order,
followed by its built-in templates. Template names are relative to the directories in
``templates_path``. They cannot escape those directories with parent directory traversal (``..``).

Every file and subdirectory in ``templates_path`` is an input dependency of the
build. When a template or partial file is added, edited, or removed, ``guidedog build``
and the live server ``guidedog serve`` detect the change and render the site again.


Page variables
~~~~~~~~~~~~~~

Where Sphinx has a name for a variable, Guidedog uses it, so parts of Sphinx themes
carry over.

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Variable
     - Value
   * - ``body``
     - The document, as HTML.
   * - ``title``
     - The document's title.
   * - ``pagename``, ``docname``
     - The document's name, such as ``usage/install``. On a page that is no
       document's, ``docname`` is empty and ``pagename`` is the page's name, as in
       Sphinx: ``genindex``, ``py-modindex``, ``search``, or the name of a page made
       from a template.
   * - ``toc``
     - The navigation built from the toctrees, as HTML.
   * - ``outline``
     - The document's own sections, as HTML; empty when it has none.
   * - ``prev``, ``next``
     - The neighbouring pages in reading order, with ``url`` (also ``link``) and
       ``title``; none at either end.
   * - ``project``, ``version``, ``release``, ``copyright``, ``language``
     - The settings of the same names.
   * - ``html_title``, ``docstitle``
     - ``html_title``, by default "<project> <release> documentation".
   * - ``html_short_title``, ``shorttitle``
     - ``html_short_title``.
   * - ``root_doc``, ``master_doc``
     - The root document's name.
   * - ``pathto_root``
     - The path from the page to the site's root, such as ``../``.
   * - ``root_url``, ``search_url``, ``genindex_url``
     - Links to the root page, the search page, and the general index.
   * - ``css_files``, ``js_files``
     - Lists of files with a ``url``: Guidedog's own, then ``html_css_files`` and
       ``html_js_files``.
   * - ``logo_url``, ``favicon_url``
     - ``html_logo`` and ``html_favicon`` below ``_static``; empty when unset.
   * - ``accent``
     - ``html_theme_options.accent``, the theme's colour.
   * - ``sourcelink_url``
     - The document's source below ``_sources``, when it is shown; otherwise empty.
   * - ``last_updated``
     - The build date in ``html_last_updated_fmt`` when it is set; otherwise empty.
   * - ``show_copyright``, ``show_sphinx``, ``show_guidedog``, ``has_source``,
       ``show_source``
     - The ``html_show_*`` and ``html_copy_source`` settings.
   * - ``builder``, ``file_suffix``
     - The builder's name, such as ``html``, and the page suffix.
   * - every key of ``html_context``
     - Its value, with its type, as Sphinx passes a Python value: ``false`` is false
       in ``{% if %}``, numbers are numbers, arrays are lists, and tables are dicts
       whose keys keep the order ``conf.toml`` gives them. ``guidedog migrate``
       carries the literal entries of ``conf.py``'s ``html_context``. A value
       ``conf.py`` computes is left undefined, which a template reads as false.

``pathto`` is Sphinx's template function. ``pathto("usage/install")`` is the URL of a
document's page, and ``pathto("_static/logo.svg", 1)`` the URL of a file below the
site's root, both relative to the current page. With the singlehtml builder, as with
Sphinx's, a document is its section of the single page: ``pathto("usage/install")`` is
``#document-usage-install`` on that page and ``index.html#document-usage-install`` on the
index and search pages. A name that is no document's, such as ``genindex`` or a page
made from a template (below), is a page at the site's root with every builder.

Pages made from templates
~~~~~~~~~~~~~~~~~~~~~~~~~

``html_additional_pages`` makes pages from a template alone, as in Sphinx. Each key is
a page's name and each value a template in ``templates_path``:

.. code-block:: toml
   :caption: conf.toml

   root_doc = "contents"

   [html_additional_pages]
   index = "indexcontent.html"
   download = "download.html"

A page's template usually extends ``layout.html`` and fills its blocks, so the page
looks like every other one:

.. code-block:: html+jinja
   :caption: _templates/indexcontent.html

   {% extends "layout.html" %}
   {% block htmltitle %}<title>{{ shorttitle }}</title>{% endblock %}
   {% block body %}
     <h1>{{ docstitle|e }}</h1>
     <p><a href="{{ pathto("tutorial/index") }}">Tutorial</a></p>
   {% endblock %}

The template sees the variables of a page that is no document's: ``pagename`` is the
page's name (``index``), ``title`` and ``body`` are empty, and ``pathto``, ``toc``,
``toctree()``, and the values of ``html_context`` work as on any page. The page is
written at the site's root as ``<name>.html`` (``html_file_suffix``) by ``html``,
``dirhtml``, and ``singlehtml``, and written again on every build. A change to its
template, as to any file in ``templates_path``, rebuilds every page.

Where the page goes, and where it differs from Sphinx:

- A page named as a document takes that document's place, as in Sphinx, where it is
  written last. ``index = "landing.html"`` makes the landing page ``index.html``, while
  ``index.rst`` still gives the table of contents its entries. With ``singlehtml`` the
  single page keeps its place and the template page is not written, with a warning.
- ``dirhtml`` writes the page as ``download.html``, beside ``genindex.html`` and
  ``search.html``, where Sphinx writes ``download/index.html``. ``pathto("download")``
  gives that file.
- A name must be a file name at the site's root: ``"sub/page"`` is refused with a
  warning (``build.additional_page``), since the page's links are made from the root.
  So is a name whose file the site already has, such as ``genindex``, where Sphinx
  would write one of the two over the other.
- ``guidedog migrate`` carries ``html_additional_pages`` over from ``conf.py``.

Errors
~~~~~~

A mistake in a template stops the build with the template's file and line, the line
itself, and a suggestion::

   TEMPLATE ERROR                                                  template.error

     _templates/base.html:1

   No filter named 'defualt'.

   Did you mean the filter 'default'?

A variable that is not defined renders as nothing, as in Jinja.

Runaway templates stop with an error too, never a crash. The limits are: syntax nested more than 100
levels deep (brackets, tags, and each link of a long chain of operators, filters or
``elif`` branches); macros, includes and recursive loops more than 200 deep; and any
recursion needing more than 512 KiB of stack. A list that contains itself prints as
``[...]``, as in Python.


PDF templates
-------------

The PDF book is typeset by Typst. ``_templates/book.typ`` is an ordinary Typst file
that defines a ``book`` function. Guidedog writes the book's source as

.. code-block:: typst

   #import "/_templates/book.typ": book
   #show: book.with(title: ..., author: ..., version: ..., date: ...,
     lang: ..., paper: ..., numbering: ..., logo: ...)
   // the chapters, one per document of the root toctree

so everything that decides how the book looks is in that file: fonts, page size and
margins, headings, the title page, running heads, and the table of contents.

Parameters
~~~~~~~~~~

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Parameter
     - Value
   * - ``title``
     - The book's ``title`` in ``pdf_documents``, otherwise ``project``.
   * - ``author``
     - The book's ``author``, otherwise ``author``.
   * - ``version``
     - ``release``.
   * - ``date``
     - ``|today|``: ``today`` when it is set, otherwise the build date in ``today_fmt``.
   * - ``lang``
     - ``language``.
   * - ``paper``
     - ``"a4"``, or ``"us-letter"`` when ``pdf_paper_size`` is ``letter``.
   * - ``numbering``
     - ``true`` when a toctree has the ``numbered`` option.
   * - ``logo``
     - The path of ``pdf_logo``; absent when it is unset.
   * - ``copyright``
     - ``copyright``, for the colophon. It is passed only to a template that takes it.
   * - ``body``
     - The chapters.

A template may add parameters with defaults of its own. The template that quickstart
writes has ``accent``, the fonts ``serif``, ``sans``, and ``mono``, the text ``size``,
and ``page-ref``. When ``page-ref`` is set to a function such as ``n => [p. #n]``,
cross-references to other pages are followed by their page number.

Modular Typst templates
~~~~~~~~~~~~~~~~~~~~~~~

Like HTML templates, Typst templates are not restricted to a single file. We can split
book styling, macros, and custom covers into several ``.typ`` files under
``_templates/``. Standard Typst ``#import`` and ``#include`` statements put them together:

.. code-block:: typst
   :caption: _templates/book.typ

   #import "cover.typ": title-page
   #import "typography.typ": apply-styles

   #let book(
     title: "",
     author: "",
     version: "",
     date: "",
     lang: "en",
     paper: "a4",
     numbering: false,
     logo: none,
     body,
   ) = {
     apply-styles([
       #title-page(title: title, author: author, version: version, logo: logo)
       #body
     ])
   }

Here ``apply-styles(body)`` must apply its rules around the content it receives.
A helper that sets text and returns nothing cannot change its caller's later body.
The complete tutorial shows a working wrapper around the generated book template.

What a template can use
~~~~~~~~~~~~~~~~~~~~~~~

Typst packages
   ``#import "@preview/cetz:0.4.2"`` and other packages work, downloaded on first use.
   Set ``pdf_packages = "offline"`` to build only with packages already on disk.

Fonts
   The system's fonts are available, as with the ``typst`` command. Add folders with
   ``pdf_font_paths``, or set ``pdf_fonts = "embedded"`` to use only the fonts built
   into Typst, for byte-for-byte reproducible books.

A preamble
   ``pdf_preamble`` names a Typst file whose content follows the ``#show: book``
   line. It suits a few ``set`` and ``show`` rules without a template of our own.

Typst in a document
   Documents are reStructuredText and Markdown. Typst files are not documents. To put
   Typst markup into the book, we write it in a raw block, which web pages leave out:

   .. code-block:: rst

      .. raw:: typst

         #align(center)[#text(size: 14pt)[Only in the book]]

   ``.. only:: pdf`` keeps ordinary content to the book in the same way.

When Typst reports an error, the build names the file and line, whether it is in our
template or in the generated book. Each PDF has its own generated source at
``_build/pdf/sources/<pdf-filename>.typ``. For example, ``reference-en.pdf`` has
``sources/reference-en.pdf.typ``. A single-book build also keeps the familiar
``_build/pdf/book-<language>.typ`` inspection copy.
A failed build publishes nothing. Its source remains in ``_build/.doctrees/failed``
at the path named by the error. Several books keep distinct failure sources.


Writing a new template
----------------------

We start from the copies quickstart wrote. They show every part in use:

1. Keep the ``layout.html`` markup that the default style sheet expects, or replace
   ``_static/guidedog.css`` along with it.
2. Move repeated parts into files of their own and ``include`` them, or put a base
   layout in ``base.html`` and ``extends`` it from ``layout.html``.
3. For the book, change the ``book`` function's rules, or write a new function with
   the same parameters and keep its name ``book``.

Preview with ``guidedog serve`` while editing. Save the source and refresh the browser.
The server rebuilds changed inputs before serving the page.
