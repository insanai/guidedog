Large manuals built with Guidedog: record of 15 October 2026
============================================================

We built three real Sphinx manuals with Guidedog 0.7.0, as HTML and as a PDF book, each from a clean ``_build`` and then once more unchanged. The per-manual diagnostics are in ``cpython-diagnostics.tsv``, ``django-diagnostics.tsv`` and ``flask-diagnostics.tsv`` (``builder``, ``severity``, ``code``, ``count``, from the clean builds). The full logs stay on the build machine; this page keeps what they show.

Setup
-----

-  Host: Linux 7.0.0-28-generic x86_64, 16 cores, 12 GB RAM. Typst 0.15.1 is embedded in ``guidedog``.

-  Guidedog 0.7.0, the release build of ``cargo build --release -p guidedog-cmd``.

-  Sources, each a checkout of one revision:

   ======= =============== ============================================
   Manual  Folder          Revision
   ======= =============== ============================================
   CPython ``cpython/Doc`` ``333071231d3a46cccc32d7f44b99328c3299d0b1``
   Django  ``django/docs`` ``9332b163a67eabe5bdbf8066b1c5da394be9aa9c``
   Flask   ``flask/docs``  ``d73fa1cdcbd8b1465c151db8924ba58b1dd14e35``
   ======= =============== ============================================

-  Commands, in the documentation folder, one build at a time:

   ::

      guidedog build html . --diagnostics=json -j auto   # clean
      guidedog build html . --diagnostics=json           # unchanged
      linkcheck _build/html -v                           # cargo run -p linkcheck
      rm -rf _build
      guidedog build pdf . --diagnostics=json -j auto    # clean
      guidedog build pdf . --diagnostics=json            # unchanged

   We ran each HTML build three times and report the median. A small wrapper measured the wall time and the peak resident memory of the process, and stopped any build whose memory passed 9 GB, so that a build could not exhaust the machine.

Configuration
-------------

Each ``conf.toml`` starts from ``guidedog migrate conf.py``; ``conf.py`` itself is never run. The changes we made by hand, as a diff from the migrated file to the one we built with:

CPython (``conf.py`` computes these from ``Include/patchlevel.h``, the git branch, and code):

.. code:: diff

   -autodoc_source_paths = ["tools/extensions", "includes"]
   +include_roots = [".."]  # Grammar/, Lib/, Parser/, Modules/, as Sphinx reads them
   +version = "3.16"
   +release = "3.16.0a0"
   +rst_epilog = """
   +.. |python_version_literal| replace:: ``Python 3.16``
   +.. |python_x_dot_y_literal| replace:: ``python3.16``
   +   ... (5 more substitutions of the version)
   +.. |FORCE_COLOR| replace:: ``FORCE_COLOR``
   +.. _FORCE_COLOR: https://force-color.org/
   +.. |NO_COLOR| replace:: ``NO_COLOR``
   +.. _NO_COLOR: https://no-color.org/
   +"""
    [extlinks]
   +issue = ["https://bugs.python.org/issue?@action=redirect&bpo=%s", "bpo-%s"]
   +gh = ["https://github.com/python/cpython/issues/%s", "gh-%s"]
   +source = ["https://github.com/python/cpython/tree/main/%s", "%s"]
    [[object_types]]  opcode, pdbcommand: + name = "first-word"
    [[object_types]]  monitoring-event:   + display = "sys.monitoring.events.%s"

Django:

.. code:: diff

    [extlinks]
   +sourcefile = ["https://github.com/django/django/blob/main/%s", "%s"]
    [[object_types]]  django-admin: + name = "first-word"
                                    + display = "django-admin %s"
                                    + program = true

Flask:

.. code:: diff

   +include_roots = [".."]  # CHANGES.rst, LICENSE.txt
   +autodoc_source_paths = ["../src"]

Extensions the projects list, and what Guidedog does with them:

+---------+---------------------------------------+---------------------------------------+
| Manual  | Built in (kept in ``extensions``)     | Not run (Python code of the project   |
|         |                                       | or a package)                         |
+=========+=======================================+=======================================+
| CPython | ``sphinx.ext.doctest``,               | 13 of its own (``audit_events``,      |
|         | ``sphinx.ext.extlinks``               | ``availability``, ``c_annotations``,  |
|         |                                       | ``changes``, ``glossary_search``,     |
|         |                                       | ``grammar_snippet``,                  |
|         |                                       | ``implementation_detail``,            |
|         |                                       | ``issue_role``, ``lexers``,           |
|         |                                       | ``misc_news``, ``profiling_trace``,   |
|         |                                       | ``pydoc_topics``, ``pyspecific``),    |
|         |                                       | ``sphinx.ext.coverage``, optional     |
|         |                                       | ``sphinx_linklint.ext``,              |
|         |                                       | ``notfound.extension``                |
+---------+---------------------------------------+---------------------------------------+
| Django  | ``sphinx.ext.extlinks``,              | ``djangodocs`` (its own),             |
|         | ``sphinx.ext.intersphinx``,           | ``sphinx.ext.linkcode``               |
|         | ``sphinx.ext.autosectionlabel``       |                                       |
+---------+---------------------------------------+---------------------------------------+
| Flask   | ``sphinx.ext.autodoc``,               | ``sphinxcontrib.log_cabinet``,        |
|         | ``sphinx.ext.extlinks``,              | ``sphinx_tabs.tabs``,                 |
|         | ``sphinx.ext.intersphinx``            | ``pallets_sphinx_themes``             |
+---------+---------------------------------------+---------------------------------------+

The object types that CPython's and Django's extensions define are declared in ``conf.toml`` (``[[object_types]]``, ``[[crossref_types]]``).

Results
-------

Every build that finished exited 0 and reported the same diagnostics clean and unchanged.

======= ======= ======================== ============================ ===========
Manual  Builder Clean: time, peak memory Unchanged: time, peak memory Diagnostics
======= ======= ======================== ============================ ===========
CPython HTML    1.24 s, 219 MB           0.92 s, 170 MB               1,048
CPython PDF     stopped at 9 GB          (not reached)
Django  HTML    2.66 s, 127 MB           0.57 s, 101 MB               166
Django  PDF     25.8 s, 6.1 GB           0.72 s, 115 MB               167
Flask   HTML    7.38 s, 66 MB            0.15 s, 52 MB                12
Flask   PDF     10.8 s, 872 MB           0.15 s, 55 MB                12
======= ======= ======================== ============================ ===========

The CPython book does not fit this machine. Typst lays out the whole book of about 590 documents in one compilation, and its memory passed the 9 GB guard. We have to build it on a machine with more memory, or split the book.

The Flask clean HTML build includes fetching the inventories of its eight ``intersphinx_mapping`` projects over the network, which takes most of its time. Almost all of the PDF builds' time and memory is Typst's layout of the book.

Diagnostics by code (clean builds; the TSV files have them per builder):

+------------------------------+---------+--------+-------+------------------------------+
| Code                         | CPython | Django | Flask | What it is                   |
+==============================+=========+========+=======+==============================+
| ``rst.directive.unknown``    | 1,032   | 166    | 8     | Directives the projects'     |
| (error)                      |         |        |       | Python extensions define:    |
|                              |         |        |       | CPython ``availability``,    |
|                              |         |        |       | ``audit-event``,             |
|                              |         |        |       | ``impl-detail`` and others;  |
|                              |         |        |       | Django ``console``; Flask    |
|                              |         |        |       | ``tabs``                     |
+------------------------------+---------+--------+-------+------------------------------+
| ``rst.role.unknown`` (error) | 1       |        | 4     | CPython ``:c:ref:`` (not a   |
|                              |         |        |       | Sphinx role); Flask          |
|                              |         |        |       | ``:gh:``, defined in         |
|                              |         |        |       | ``conf.py`` code             |
+------------------------------+---------+--------+-------+------------------------------+
| ``rst.target.unresolved``,   | 3 + 3   |        |       | ``TRANSLATION_REPO_``, a     |
| ``rst.reference.unresolved`` |         |        |       | target CPython's ``conf.py`` |
| (error)                      |         |        |       | adds to ``rst_epilog`` in    |
|                              |         |        |       | code                         |
+------------------------------+---------+--------+-------+------------------------------+
| ``build.extlink.hardcoded``  | 7       |        |       | A URL written out where an   |
| (warning)                    |         |        |       | extlink role would do        |
+------------------------------+---------+--------+-------+------------------------------+
| ``build.label.duplicate``    | 1       |        |       | ``customization``, in        |
| (warning)                    |         |        |       | ``using/windows.rst`` and    |
|                              |         |        |       | ``reference/datamodel.rst``  |
+------------------------------+---------+--------+-------+------------------------------+
| ``build.xref.unresolved``    | 1       |        |       | ``:ref:`` to                 |
| (warning)                    |         |        |       | ``asyncio_example_gather``,  |
|                              |         |        |       | which the sources do not     |
|                              |         |        |       | define                       |
+------------------------------+---------+--------+-------+------------------------------+
| ``typst.warning``            |         | 1      |       | An SVG image with a foreign  |
| (warning, PDF only)          |         |        |       | object, which Typst may draw |
|                              |         |        |       | incorrectly                  |
+------------------------------+---------+--------+-------+------------------------------+

No diagnostic is about a construct Docutils or Sphinx itself defines. The content of an unknown directive is left out of the output, so CPython's "Availability" notes, Django's ``console`` examples and Flask's tab sets are missing from both HTML and PDF until the directives are declared or migrated (see the compatibility guide).

Broken links
------------

The link check reads every page of the HTML output. Every ``href`` inside the site must name an existing file, and its ``#fragment`` an id or name on that page.

======= ===== ============== ============= ===============
Manual  Pages Internal links Missing files Missing anchors
======= ===== ============== ============= ===============
CPython 589   178,783        2             0
Django  678   245,481        0             0
Flask   79    8,539          0             0
======= ===== ============== ============= ===============

CPython's two missing files are inert. ``PAGEURL`` is a placeholder that the script of ``improve-page.rst`` fills in. ``lifecycle.dot.css`` is an ``<?xml-stylesheet?>`` instruction inside an SVG that ``c-api/lifecycle.rst`` includes as raw HTML.
