Configuration reference
=======================

Every Guidedog documentation project is configured with ``conf.toml``. Guidedog reads it
as TOML data and runs no code. Setting names are those of Sphinx's
``conf.py``. So ``guidedog migrate conf.py`` can translate an existing project, and the
Sphinx documentation describes the settings Guidedog shares with it.

This page lists every setting Guidedog reads. The settings are flat top-level keys, as in
``conf.py``. A table setting is written as a TOML table (``[html_context]``) or an
inline table.

Where conf.toml is
------------------

Guidedog looks for ``conf.toml`` in the project folder, then in ``PROJECT/source`` (the
separated layout of ``quickstart --sep``, with ``build/`` beside ``source/``), then in the
nearest parent folder that has one. The folder holding ``conf.toml`` is the source folder.
The build folder is ``_build`` beside it, or ``build`` in the separated layout.
``guidedog build -c DIR`` reads ``conf.toml`` from another folder. ``-C`` builds
without one, with the defaults below.

``-D name=value`` overrides one setting for one build. The value is TOML
(``-D numfig=true``, ``-D 'exclude_patterns=["drafts/**"]'``). A bare word is read as a
string (``-D language=de``). ``-D html_context.key=value`` sets one key of a table setting
and keeps the others, as sphinx-build does.

Minimal configuration
---------------------

A project needs only basic metadata. Omitted keys take their defaults:

.. code-block:: toml

   project = "Field Notes"
   author = "Your Name"
   version = "1.0"
   release = "1.0.0"
   language = "en"
   root_doc = "index"
   source_suffix = [".rst", ".md"]
   templates_path = ["_templates"]
   html_static_path = ["_static"]
   pdf_paper_size = "a4"

How settings are checked
------------------------

Each value is checked against its type when ``conf.toml`` is read. A mistake is an
``INVALID CONFIGURATION`` problem (exit status 2) naming the line and the expected form.
An unknown setting is an error too, with a "Did you mean ...?" hint when the name is
close to a known one. A setting of a Sphinx builder or extension Guidedog does not have
(``latex_*``, ``epub_*``, ``man_*``, ``texinfo_*``, ``autosummary_*``, ``napoleon_*``,
``linkcheck_*``, ``html_sidebars``, ``needs_*`` and similar) is accepted with a
``config.ignored`` warning. So the same file can serve a Sphinx build of the sources.

Some values are derived after reading:

* ``release`` defaults to ``version``.
* ``html_title`` defaults to ``"<project> <release> documentation"`` (or
  ``"<project> documentation"`` without a release). ``html_short_title`` defaults to
  ``html_title``.
* Without ``pdf_documents``, the project has one book. It starts from ``root_doc``, is titled with
  ``project``, is by ``author``, and is in a file named after the project.
* ``extensions = ["myst_parser"]`` adds ``.md`` to ``source_suffix``, as the Sphinx
  extension does.

Project information
-------------------

.. list-table:: Project information
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``project``
     - string
     - ``"Python"``
     - The project's name (Sphinx's default).
   * - ``author``
     - string
     - ``""``
     - The author or organization name.
   * - ``copyright``
     - string
     - ``""``
     - The copyright notice in page footers.
   * - ``version``
     - string
     - ``""``
     - The short version (``"1.0"``).
   * - ``release``
     - string
     - ``version``
     - The full release, including alpha or beta tags (``"1.0.0b1"``).
   * - ``language``
     - string
     - ``"en"``
     - The document language (``"de"``, ``"zh_CN"``): typography, hyphenation, the
       theme's words, and the translation catalogs used.
   * - ``today``
     - string
     - ``""``
     - A fixed date for ``|today|``; when empty, the build date formatted with
       ``today_fmt``.
   * - ``today_fmt``
     - string
     - ``""``
     - The ``strftime`` format of that date (``"%Y-%m-%d"``).

Sources and reading
-------------------

.. list-table:: Sources and reading
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``root_doc``
     - string
     - ``"index"``
     - The document that starts the reading order. ``master_doc`` is its older name.
   * - ``master_doc``
     - string
     - ``"index"``
     - The same setting as ``root_doc``.
   * - ``source_suffix``
     - string, array or table
     - ``[".rst"]``
     - Suffixes of source documents: ``".rst"``, ``[".rst", ".md"]``, or Sphinx's
       ``{".rst" = "restructuredtext", ".md" = "markdown"}``. ``.md`` documents are MyST.
       ``.typ`` is refused: Typst shapes the PDF, it is not a document.
   * - ``exclude_patterns``
     - array
     - ``[]``
     - Glob patterns of files and folders that are not documents.
   * - ``include_patterns``
     - array
     - ``["**"]``
     - Glob patterns of the documents to find.
   * - ``include_roots``
     - array
     - ``[]``
     - Folders, relative to the source folder, that ``include``, ``literalinclude``,
       images, graphs, logos and other files may also be read from. Sphinx reads any
       path; Guidedog reads only the source folder and these.
   * - ``templates_path``
     - array
     - ``[]``
     - Folders of HTML templates, searched before the built-in theme.
   * - ``extensions``
     - array
     - ``[]``
     - Built-in Sphinx extensions to enable: ``sphinx.ext.mathjax``, ``sphinx.ext.todo``,
       ``sphinx.ext.githubpages``, ``sphinx.ext.ifconfig``, ``sphinx.ext.extlinks``,
       ``sphinx.ext.imgmath``, ``sphinx.ext.autosectionlabel``, ``sphinx.ext.graphviz``,
       ``sphinx.ext.doctest``, ``sphinx.ext.intersphinx``, ``sphinx.ext.autodoc``, and
       ``myst_parser``. Any other name is an error: native code is selected with
       ``plugins``.
   * - ``rst_prolog``
     - string
     - ``""``
     - reStructuredText read at the start of every document.
   * - ``rst_epilog``
     - string
     - ``""``
     - reStructuredText read at the end of every document.
   * - ``default_role``
     - string
     - ``""``
     - The role of text in single backquotes (``"any"``, ``"py:obj"``).
   * - ``primary_domain``
     - string
     - ``"py"``
     - The domain of directives and roles written without a prefix (``"py"``, ``"c"``,
       ``"cpp"``, ``"js"``, ``"odin"``, ``"rust"``).
   * - ``highlight_language``
     - string
     - ``"default"``
     - The language of code blocks that name none. ``default`` is Python, falling back to
       plain text; ``guess`` guesses.
   * - ``pygments_style``
     - string
     - ``""``
     - The code style of light pages (Pygments' built-in names, or ``sphinx`` or
       ``none``); empty: the theme's. An unknown style is replaced by ``sphinx``, with a
       warning.
   * - ``pygments_dark_style``
     - string
     - ``""``
     - The code style of dark pages; empty or unknown: ``github-dark``.
   * - ``smartquotes``
     - boolean
     - ``true``
     - Turn straight quotes and dashes into typographic ones.
   * - ``trim_footnote_reference_space``
     - boolean
     - ``false``
     - Remove the space before a footnote reference.
   * - ``show_authors``
     - boolean
     - ``false``
     - Show the ``sectionauthor`` and ``codeauthor`` directives' content.
   * - ``add_function_parentheses``
     - boolean
     - ``true``
     - Add ``()`` to the text of function and method references.
   * - ``add_module_names``
     - boolean
     - ``true``
     - Show module names before object names in signatures.
   * - ``toc_object_entries``
     - boolean
     - ``true``
     - List described objects in the page's table of contents.
   * - ``toc_object_entries_show_parents``
     - string
     - ``"domain"``
     - How those entries are named: ``domain``, ``hide``, or ``all``.
   * - ``todo_include_todos``
     - boolean
     - ``false``
     - Show ``todo`` and ``todolist`` content (``sphinx.ext.todo``).
   * - ``autosectionlabel_prefix_document``
     - boolean
     - ``false``
     - Make section labels ``docname:Title`` (``sphinx.ext.autosectionlabel``).
   * - ``autosectionlabel_maxdepth``
     - integer
     - ``0``
     - Sections this deep or deeper get no automatic label; ``0``: all get one.
   * - ``trim_doctest_flags``
     - boolean
     - ``true``
     - Hide doctest flags and ``<BLANKLINE>`` in Python sessions.
   * - ``manpages_url``
     - string
     - ``""``
     - The URL pattern of the ``manpage`` role (``{page}``, ``{section}``, ``{path}``).
   * - ``option_emphasise_placeholders``
     - boolean
     - ``false``
     - Emphasize ``<placeholder>`` parts of ``option`` signatures.
   * - ``extlinks``
     - table
     - ``{}``
     - Short link roles (``sphinx.ext.extlinks``): each key is a role, each value a URL
       pattern and a caption pattern, with ``%s`` where the target goes.
   * - ``extlinks_detect_hardcoded_links``
     - boolean
     - ``false``
     - Note full URLs that one of the ``extlinks`` roles could write.

.. code-block:: toml

   [extlinks]
   issue = ["https://github.com/insanai/guidedog/issues/%s", "issue %s"]

Build plugins
-------------

.. list-table:: Build plugins
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``plugins``
     - array
     - ``[]``
     - The build plugins this project selects, by ID, in the order they run.
   * - ``plugin_configs``
     - table
     - ``{}``
     - Plugin ID to its configuration file, relative to the folder of ``conf.toml``.
   * - ``plugin_timeout``
     - integer
     - ``60``
     - Seconds per complete request to an installed plugin program, from 1 to 86400.

A plugin ID is looked up among the plugins linked into the ``guidedog`` executable
first, then among the programs registered with ``guidedog plugin register ID PATH``.
A normal build invokes no publisher. ``--untrusted`` disables build plugins.
Build plugins transform source text, post-process complete HTML pages, and add generated
assets. See :doc:`../plugins`.

.. code-block:: toml

   plugins = ["product-badge"]
   plugin_timeout = 60

   [plugin_configs]
   product-badge = "badge.toml"

Object types and directive aliases
----------------------------------

.. list-table:: Declared object types
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``object_types``
     - array of tables
     - ``[]``
     - Directives and roles that describe and link custom objects, as Sphinx's
       ``app.add_object_type``. Fields: ``directive`` (required), ``role``, ``index``,
       ``name`` (``"whole"`` or ``"first-word"``), ``display``, and ``program``.
   * - ``crossref_types``
     - array of tables
     - ``[]``
     - Targets and roles without a description, as ``app.add_crossref_type``. Fields:
       ``directive`` (required), ``role``, and ``index``.
   * - ``directive_aliases``
     - table
     - ``{}``
     - New directive names for existing directives
       (``django-admin-option = "option"``).

.. code-block:: toml

   [[object_types]]
   directive = "confval"
   role = "confval"
   index = "pair: %s; configuration value"

See :doc:`../object-types`.

Numbering and mathematics
-------------------------

.. list-table:: Numbering and mathematics
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``numfig``
     - boolean
     - ``false``
     - Number figures, tables, code blocks, and sections with a caption or label.
   * - ``numfig_format``
     - table
     - see below
     - The number formats of ``figure``, ``table``, ``code-block``, and ``section``.
   * - ``numfig_secnum_depth``
     - integer
     - ``1``
     - Section levels in figure numbers (``1`` gives ``Fig. 2.1``).
   * - ``math_number_all``
     - boolean
     - ``false``
     - Number every displayed equation.
   * - ``math_eqref_format``
     - string
     - ``"({number})"``
     - The text of ``:eq:`` references.
   * - ``math_numfig``
     - boolean
     - ``true``
     - With ``numfig``, number equations by section.
   * - ``mathjax_path``
     - string
     - MathJax 4 on jsDelivr
     - The MathJax script: a URL, or a copy in ``html_static_path``.
   * - ``mathjax_options``
     - table
     - ``{}``
     - Attributes of the MathJax ``script`` element.
   * - ``mathjax_inline``
     - array
     - ``["\\(", "\\)"]``
     - The opening and closing delimiters of inline math in HTML.
   * - ``mathjax_display``
     - array
     - ``["\\[", "\\]"]``
     - The opening and closing delimiters of displayed math in HTML.
   * - ``mathjax3_config``
     - table
     - none
     - The MathJax 3 configuration object, written into the page as JSON.
   * - ``mathjax4_config``
     - table
     - none
     - The MathJax 4 configuration object, written into the page as JSON.
   * - ``mathjax_config_path``
     - string
     - ``""``
     - A ``.js`` file, relative to ``conf.toml``, that configures MathJax.

Default ``numfig_format``:

.. code-block:: toml

   [numfig_format]
   figure = "Fig. %s"
   table = "Table %s"
   code-block = "Listing %s"
   section = "Section %s"

Diagnostics and strictness
--------------------------

.. list-table:: Diagnostics
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``nitpicky``
     - boolean
     - ``false``
     - Warn about every unresolved reference (as ``-n``).
   * - ``nitpick_ignore``
     - array
     - ``[]``
     - ``[type, target]`` pairs exempt from nitpicky warnings
       (``[["py:class", "MyType"]]``).
   * - ``nitpick_ignore_regex``
     - array
     - ``[]``
     - The same, as regular expressions that must match the whole type and target.
   * - ``suppress_warnings``
     - array
     - ``[]``
     - Warning types not reported (``"ref.ref"``, ``"toc.excluded"``).
   * - ``keep_warnings``
     - boolean
     - ``false``
     - Also show warnings in the output pages.

HTML output
-----------

.. list-table:: HTML output
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``html_theme``
     - string
     - ``"guidedog"``
     - The theme. Sphinx's theme names (``alabaster``, ``furo``, ``sphinx_rtd_theme``,
       ``pydata_sphinx_theme``, ``sphinx_book_theme`` and others) build with Guidedog's
       theme, with a note; any other name is an error.
   * - ``html_theme_options``
     - table
     - ``{}``
     - Theme options, such as ``accent = "#216553"``.
   * - ``html_title``
     - string
     - derived
     - The site title in page titles and the header.
   * - ``html_short_title``
     - string
     - ``html_title``
     - A shorter title for navigation.
   * - ``html_logo``
     - string
     - ``""``
     - A logo image, relative to the source folder, or a URL.
   * - ``html_favicon``
     - string
     - ``""``
     - A favicon, relative to the source folder, or a URL.
   * - ``html_static_path``
     - array
     - ``[]``
     - Folders copied into the output's ``_static``; a missing folder is a warning.
   * - ``html_extra_path``
     - array
     - ``[]``
     - Folders copied into the output root as they are.
   * - ``html_css_files``
     - array
     - ``[]``
     - Style sheets every page loads, after the theme's.
   * - ``html_js_files``
     - array
     - ``[]``
     - Scripts every page loads.
   * - ``html_last_updated_fmt``
     - string
     - unset
     - Show a "Last updated" date: unset shows none; ``""`` uses the default format.
   * - ``html_permalinks``
     - boolean
     - ``true``
     - Add a link to each heading, caption, and signature.
   * - ``html_permalinks_icon``
     - string
     - ``"¶"``
     - The text of those links.
   * - ``html_use_index``
     - boolean
     - ``true``
     - Write the general index, ``genindex.html``.
   * - ``html_split_index``
     - boolean
     - ``false``
     - Split the general index into one page per letter.
   * - ``html_domain_indices``
     - boolean
     - ``true``
     - Write the Python module index, ``py-modindex.html``.
   * - ``modindex_common_prefix``
     - array
     - ``[]``
     - Prefixes the module index leaves out when sorting.
   * - ``html_copy_source``
     - boolean
     - ``true``
     - Copy each source into ``_sources``.
   * - ``html_show_sourcelink``
     - boolean
     - ``true``
     - Link each page to its copied source.
   * - ``html_sourcelink_suffix``
     - string
     - ``".txt"``
     - The suffix of those copies.
   * - ``html_show_copyright``
     - boolean
     - ``true``
     - Show ``copyright`` in the footer.
   * - ``html_show_sphinx``
     - boolean
     - ``true``
     - Show the "built with" note in the footer.
   * - ``html_baseurl``
     - string
     - ``""``
     - The site's address, for canonical links.
   * - ``html_file_suffix``
     - string
     - ``".html"``
     - The suffix of the pages.
   * - ``html_link_suffix``
     - string
     - ``""``
     - The suffix in links to pages; empty: ``html_file_suffix``.
   * - ``html_secnumber_suffix``
     - string
     - ``". "``
     - The text after section numbers.
   * - ``html_context``
     - table
     - ``{}``
     - Variables for the templates; any TOML values, tables and arrays included.
   * - ``html_additional_pages``
     - table
     - ``{}``
     - Extra pages made from templates: page name to template
       (``download = "download.html"``).

See :doc:`../templates` for the template variables.

PDF and Typst
-------------

.. list-table:: PDF output
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``pdf_documents``
     - array of tables
     - one book
     - The books: ``root`` (the first document), ``file``, ``title``, and ``author``.
   * - ``pdf_paper_size``
     - string
     - ``"a4"``
     - The paper size (``"a4"``, ``"us-letter"``).
   * - ``pdf_logo``
     - string
     - ``""``
     - A logo for the cover.
   * - ``pdf_toplevel_sectioning``
     - string
     - ``"chapter"``
     - What top-level sections become: ``"chapter"`` or ``"part"``.
   * - ``pdf_show_urls``
     - string
     - ``"no"``
     - How link addresses are printed: ``"no"``, ``"inline"``, or ``"footnote"``.
   * - ``pdf_preamble``
     - string
     - ``""``
     - Typst source placed before the book.
   * - ``pdf_packages``
     - string
     - ``"download"``
     - Typst packages: download what is missing, or ``"offline"``, only what is on disk.
   * - ``pdf_fonts``
     - string
     - ``"system"``
     - Fonts: the system's and the embedded ones, or ``"embedded"`` only, for
       byte-reproducible PDFs.
   * - ``pdf_font_paths``
     - array
     - ``[]``
     - More font folders, relative to the source folder.
   * - ``typst``
     - string
     - ``"typst"``
     - The Typst program when the compiler is not built in, or to use another one
       (``--typst`` overrides it).

The book's layout is ``_templates/book.typ`` in the source folder, written by
``quickstart``. A book's file gets the language as a suffix
(``field-notes-en.pdf``).

.. code-block:: toml

   [[pdf_documents]]
   root = "index"
   file = "guide.pdf"
   title = "Field Notes Complete Guide"
   author = "Author Name"

   [[pdf_documents]]
   root = "reference/index"
   file = "reference.pdf"
   title = "Field Notes Reference"
   author = "Author Name"

Graphviz
--------

.. list-table:: Graphviz
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``graphviz_output_format``
     - string
     - ``"svg"``
     - Graphs are SVG; ``"png"`` is read as ``"svg"``, with a warning.
   * - ``graphviz_dot_args``
     - array
     - ``[]``
     - Graph, node, and edge attributes for every graph: ``-Gname=value``,
       ``-Nname=value``, ``-Ename=value``.
   * - ``graphviz_dot``
     - string
     - ``""``
     - Accepted from ``conf.py`` and unused: Graphviz is linked into Guidedog.

MyST Markdown
-------------

The names are myst-parser 0.16.1's.

.. list-table:: MyST Markdown
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``myst_enable_extensions``
     - array
     - ``["dollarmath"]``
     - Optional syntaxes: ``amsmath``, ``colon_fence``, ``deflist``, ``dollarmath``,
       ``fieldlist``, ``html_admonition``, ``html_image``, ``linkify``, ``replacements``,
       ``smartquotes``, ``substitution``, and ``tasklist``.
   * - ``myst_heading_anchors``
     - integer
     - ``0``
     - Heading depth that gets slug anchors; ``0``: none.
   * - ``myst_dmath_allow_labels``
     - boolean
     - ``true``
     - ``$$ ... $$ (label)`` labels an equation.
   * - ``myst_dmath_allow_space``
     - boolean
     - ``true``
     - Inline ``$`` math may start or end with a space.
   * - ``myst_dmath_allow_digits``
     - boolean
     - ``true``
     - Inline ``$`` math may touch digits.
   * - ``myst_dmath_double_inline``
     - boolean
     - ``false``
     - ``$$ ... $$`` inside a paragraph is displayed math.
   * - ``myst_linkify_fuzzy_links``
     - boolean
     - ``true``
     - ``linkify`` also links addresses without a scheme.
   * - ``myst_substitutions``
     - table
     - ``{}``
     - Values for ``{{ name }}`` (the ``substitution`` syntax).
   * - ``myst_sub_delimiters``
     - array
     - ``["{", "}"]``
     - The two substitution characters, each doubled around a name.
   * - ``myst_url_schemes``
     - array
     - ``["http", "https", "mailto", "ftp"]``
     - Schemes that make external links; empty: all.
   * - ``myst_footnote_transition``
     - boolean
     - ``true``
     - Put a transition before the footnotes.
   * - ``myst_html_meta``
     - table
     - ``{}``
     - ``meta`` elements for every Markdown page.
   * - ``myst_commonmark_only``
     - boolean
     - ``false``
     - CommonMark syntax only; headings still make sections.
   * - ``myst_disable_syntax``
     - array
     - ``[]``
     - Syntax rules not parsed, by markdown-it's names.
   * - ``myst_words_per_minute``
     - integer
     - ``200``
     - The reading speed of the reading-time estimate.
   * - ``myst_dmath_enable``
     - boolean
     - ``false``
     - Older spelling of ``dollarmath`` in ``myst_enable_extensions``.
   * - ``myst_amsmath_enable``
     - boolean
     - ``false``
     - Older spelling of ``amsmath`` in ``myst_enable_extensions``.

Intersphinx
-----------

.. list-table:: Intersphinx
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``intersphinx_mapping``
     - table
     - ``{}``
     - Other projects: name to ``[target URI, inventory]``. The inventory is ``""`` (the
       ``objects.inv`` at the URI), a location, or a list of locations tried in order.
   * - ``intersphinx_cache_limit``
     - integer
     - ``5``
     - Days a downloaded inventory is reused; below 0: always.
   * - ``intersphinx_timeout``
     - integer
     - ``0``
     - Seconds a download may take; ``0``: no limit.
   * - ``intersphinx_disabled_reftypes``
     - array
     - ``["std:doc"]``
     - Reference types that never resolve through inventories.
   * - ``intersphinx_resolve_self``
     - string
     - ``""``
     - A mapping name that refers to this project itself.

.. code-block:: toml

   extensions = ["sphinx.ext.intersphinx"]
   intersphinx_cache_limit = 5
   intersphinx_timeout = 30

   [intersphinx_mapping]
   python = ["https://docs.python.org/3/", ""]
   click = ["https://click.palletsprojects.com/", ["click.inv", ""]]

Inventories are downloaded with the built-in HTTP client, which honours ``HTTPS_PROXY`` and ``NO_PROXY``. ``--untrusted`` fetches nothing.
See :doc:`../intersphinx`.

Python autodoc
--------------

Guidedog reads Python source without importing it (see :doc:`../autodoc`).

.. list-table:: Python autodoc
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``autodoc_source_paths``
     - array
     - ``[]``
     - Folders holding the packages, like ``sys.path``, relative to the source folder.
   * - ``autoclass_content``
     - string
     - ``"class"``
     - A class's docstring: ``"class"``, ``"init"``, or ``"both"``.
   * - ``autodoc_class_signature``
     - string
     - ``"mixed"``
     - ``"mixed"`` shows the constructor's signature with the class; ``"separated"``
       documents ``__init__`` apart.
   * - ``autodoc_default_options``
     - table
     - ``{}``
     - Options for every ``auto*`` directive: an option's value, or ``true`` for a flag
       (``members = true``).
   * - ``autodoc_docstring_signature``
     - boolean
     - ``true``
     - Read a signature from the docstring's first line.
   * - ``autodoc_inherit_docstrings``
     - boolean
     - ``true``
     - Members without a docstring take their parent class's.
   * - ``autodoc_member_order``
     - string
     - ``"alphabetical"``
     - ``"alphabetical"``, ``"bysource"``, or ``"groupwise"``.
   * - ``autodoc_mock_imports``
     - array
     - ``[]``
     - Accepted: nothing is imported, so nothing needs mocking.
   * - ``autodoc_preserve_defaults``
     - boolean
     - ``false``
     - Show default values as written in the source.
   * - ``autodoc_typehints``
     - string
     - ``"signature"``
     - Where type hints go: ``"signature"``, ``"description"``, ``"none"``, or
       ``"both"``.
   * - ``autodoc_typehints_description_target``
     - string
     - ``"all"``
     - Which parameters get types in the description: ``"all"``, ``"documented"``, or
       ``"documented_params"``.
   * - ``autodoc_typehints_format``
     - string
     - ``"short"``
     - ``"short"`` or ``"fully-qualified"`` type names.
   * - ``autodoc_type_aliases``
     - table
     - ``{}``
     - Type aliases kept as written: alias to its full name.
   * - ``autodoc_use_type_comments``
     - boolean
     - ``true``
     - Read ``# type:`` comments.
   * - ``autodoc_warningiserror``
     - boolean
     - ``true``
     - Treat a module autodoc cannot find as an error.
   * - ``python_display_short_literal_types``
     - boolean
     - ``false``
     - Write ``Literal["a"]`` types as ``"a"``.
   * - ``strip_signature_backslash``
     - boolean
     - ``false``
     - Remove backslashes from signatures written in directives.

Odin API documentation
----------------------

.. list-table:: Odin API pages
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``odin_autoapi_dirs``
     - array
     - ``[]``
     - Folders whose Odin packages get API pages, relative to the source folder.
   * - ``odin_collections``
     - table
     - ``{}``
     - Collection names to folders (``core = "/opt/odin/core"``), for imports such as
       ``core:fmt``.
   * - ``odin_autoapi_root``
     - string
     - ``"api"``
     - The folder of the generated pages, inside the source folder.
   * - ``odin_autoapi_options``
     - array
     - ``["members", "undoc-members"]``
     - ``members``, ``undoc-members``, and ``private-members``.
   * - ``odin_autoapi_member_order``
     - string
     - ``"source"``
     - ``"source"``, ``"alphabetical"``, or ``"groupwise"``.
   * - ``odin_autoapi_add_toctree_entry``
     - boolean
     - ``true``
     - Add the API index to the root document's toctree.
   * - ``odin_autoapi_generate_api_docs``
     - boolean
     - ``true``
     - Generate the pages; ``false``: only the ``odin:auto*`` directives read the
       folders.

See :doc:`../odin`.

Rust API documentation
----------------------

New in the Rust release of Guidedog. Crates are read with tree-sitter, never compiled.
See :doc:`../howto/document-code` and the ``rust`` domain in :doc:`../topics/domains`.

.. list-table:: Rust API pages
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``rust_autoapi_crates``
     - array
     - ``[]``
     - Crate folders (with ``Cargo.toml``, ``lib.rs``, ``main.rs`` or ``mod.rs``), or root
       ``.rs`` files, that get API pages, relative to the source folder. A missing folder
       is a ``config.rust`` warning.
   * - ``rust_autoapi_root``
     - string
     - ``"rustapi"``
     - The folder of the generated pages, inside the source folder.
   * - ``rust_autoapi_options``
     - array
     - ``[]``
     - ``private-members`` (document private items, with their visibility),
       ``hidden-members`` (``#[doc(hidden)]`` items), and ``noindex``.
   * - ``rust_autoapi_member_order``
     - string
     - ``"source"``
     - ``"source"`` or ``"alphabetical"``.
   * - ``rust_autoapi_add_toctree_entry``
     - boolean
     - ``true``
     - Add the API index to the root document's toctree.
   * - ``rust_autoapi_generate_api_docs``
     - boolean
     - ``true``
     - Generate the pages; ``false`` generates none.

.. code-block:: toml

   rust_autoapi_crates = ["../crates/my-parser"]
   rust_autoapi_root = "rustapi"
   rust_autoapi_options = ["noindex"]

With ``--untrusted``, crate folders outside the source folder are not read.

Internationalization
--------------------

The names are Sphinx's. See :doc:`../intl`.

.. list-table:: Internationalization
   :header-rows: 1
   :widths: 28 14 18 40

   * - Key
     - Type
     - Default
     - Description
   * - ``locale_dirs``
     - array
     - ``["locales"]``
     - Folders of message catalogs, relative to the source folder.
   * - ``gettext_compact``
     - boolean or string
     - ``true``
     - One catalog per top-level folder (``true``), one per document (``false``), or one
       catalog of that name.
   * - ``gettext_uuid``
     - boolean
     - ``false``
     - Write a UUID with each message in the templates.
   * - ``gettext_location``
     - boolean
     - ``true``
     - Write each message's source locations in the templates.
   * - ``gettext_auto_build``
     - boolean
     - ``true``
     - Compile ``.po`` catalogs to ``.mo`` beside them during the build.
   * - ``gettext_additional_targets``
     - array
     - ``[]``
     - Also translate ``index``, ``literal-block``, ``doctest-block``, ``raw``, or
       ``image``.
   * - ``gettext_exclude_patterns``
     - array
     - ``[]``
     - Document patterns left out of the templates only.
   * - ``gettext_allow_fuzzy_translations``
     - boolean
     - ``false``
     - Use fuzzy translations too.
   * - ``gettext_last_translator``
     - string
     - ``"FULL NAME <EMAIL@ADDRESS>"``
     - The ``Last-Translator`` header of new catalogs.
   * - ``gettext_language_team``
     - string
     - ``"LANGUAGE <LL@li.org>"``
     - The ``Language-Team`` header of new catalogs.
   * - ``figure_language_filename``
     - string
     - ``"{root}.{language}{ext}"``
     - The file name of a translated image, from ``{root}``, ``{path}``, ``{docpath}``,
       ``{basename}``, ``{ext}``, and ``{language}``.
   * - ``translation_progress_classes``
     - boolean or string
     - ``false``
     - Mark translated and untranslated text with classes: ``true``, ``false``,
       ``"translated"``, or ``"untranslated"``.

Trust
-----

``guidedog build --untrusted`` builds a project whose sources we did not write. Raw
content is left out, nothing is fetched, and no build plugin runs. The project cannot
widen what the build reads. ``include_roots`` and ``pdf_preamble`` are not used. Files
and folders that lead outside the source folder (``html_logo``, ``html_favicon``,
``pdf_logo``, ``mathjax_config_path``, ``html_static_path``, ``html_extra_path``,
``templates_path``, ``locale_dirs``, ``pdf_font_paths``, ``autodoc_source_paths``,
``odin_autoapi_dirs``, ``rust_autoapi_crates``) are left out. No Typst package is
downloaded. Each dropped setting is a ``build.untrusted`` warning. This is a capability
policy, not a process sandbox. See :doc:`cli`.
