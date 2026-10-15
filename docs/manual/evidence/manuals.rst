Large manuals built with Guidedog: record of 30 September 2026
==============================================================

Three real Sphinx manuals, built as HTML and as a PDF book, each from a clean ``_build`` and
then once more unchanged. The per-manual diagnostics are in ``cpython-diagnostics.tsv``,
``django-diagnostics.tsv``, and ``flask-diagnostics.tsv`` (``builder``, ``severity``,
``code``, ``count``, from the clean builds). Full logs stay on the build machine; this file
keeps what they show.

Setup
-----

-  Host: Linux 7.0.0-28-generic x86_64, 32 cores, 121 GB RAM; Typst 0.15.1.

-  Guidedog: revision ``70708cde81a3d3f3d904cfb49c973b685022635a`` (no uncommitted changes
   to tracked files), built with ``odin build cmd/guidedog -o:speed``; binary SHA-256
   ``efedffa545365099faab1e1f6b3fc35e47a3c27c69c7bf97f9ca1d7032609620``.

-  Sources, each copied (without ``.git`` and ``_build``) before building:

   ======= =============== ============================================
   Manual  Checkout        Revision
   ======= =============== ============================================
   CPython ``cpython/Doc`` ``333071231d3a46cccc32d7f44b99328c3299d0b1``
   Django  ``django/docs`` ``9332b163a67eabe5bdbf8066b1c5da394be9aa9c``
   Flask   ``flask/docs``  ``d73fa1cdcbd8b1465c151db8924ba58b1dd14e35``
   ======= =============== ============================================

-  Commands, in the documentation folder, one build at a time:

   ::

      /usr/bin/time -v guidedog build html . --diagnostics=json -j auto   # clean
      /usr/bin/time -v guidedog build html . --diagnostics=json           # unchanged
      linkcheck _build/html -v                                            # tools/linkcheck
      rm -rf _build
      /usr/bin/time -v guidedog build pdf . --diagnostics=json -j auto    # clean
      /usr/bin/time -v guidedog build pdf . --diagnostics=json            # unchanged

Configuration
-------------

Each ``conf.toml`` starts from ``guidedog migrate conf.py``; ``conf.py`` itself is never
run. The changes made by hand, as a diff from the migrated file to the one built with
(object type declarations that only moved are left out):

CPython (conf.py computes these from ``Include/patchlevel.h``, the git branch, and code):

.. code:: diff

   -autodoc_source_paths = ["tools/extensions", "includes"]
   +include_roots = [".."]  # Grammar/, Lib/, Parser/, Modules/, as Sphinx reads them
   +version = "3.15"
   +release = "3.15.0a0"
   +rst_epilog = """
   +.. |python_version_literal| replace:: ``Python 3.15``
   +.. |python_x_dot_y_literal| replace:: ``python3.15``
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

``guidedog migrate`` reports these per code (CPython 70 reports: 14 ``migrate.extension``,
12 ``migrate.extension.read``, 3 ``migrate.extension.parse_node``, 20 ``migrate.unknown``,
10 ``migrate.code``, 6 ``migrate.computed``, 2 ``migrate.path``, and one each of
``migrate.setup``, ``migrate.sphinx``, ``migrate.theme``; Django 18; Flask 10). The object
types CPython's and Django's extensions define are declared in ``conf.toml``
(``[[object_types]]``, ``[[crossref_types]]``).

Results
-------

Peak memory is ``/usr/bin/time -v``'s maximum resident set size, which for PDF builds is the
Typst compiler's (the largest child process). Every build exited 0 and reported the same
diagnostics clean and unchanged.

======= ============== ======================== ============================ ===========
Manual  Builder        Clean: time, peak memory Unchanged: time, peak memory Diagnostics
======= ============== ======================== ============================ ===========
CPython HTML           2.41 s, 773 MB           0.76 s, 226 MB               1,041
CPython PDF (135.6 MB) 76.7 s, 14.4 GB          1.41 s, 287 MB               1,047
Django  HTML           2.66 s, 616 MB           0.49 s, 137 MB               166
Django  PDF (52.8 MB)  29.8 s, 5.1 GB           0.76 s, 170 MB               166
Flask   HTML           4.47 s, 354 MB           0.09 s, 62 MB                12
Flask   PDF (6.4 MB)   7.84 s, 681 MB           4.04 s, 681 MB               12
======= ============== ======================== ============================ ===========

The Flask clean builds include fetching the inventories of its eight ``intersphinx_mapping``
projects. Its first PDF build after a clean one writes the book again
(``write 1 output(s) written``), although no document changed; the next ones are unchanged
(0.2 s). This was a Guidedog incremental-build defect, since fixed (see the last section):
seven of the eight inventories had moved (``werkzeug.palletsprojects.com/objects.inv``
redirects to ``/en/stable/``), and only the build that fetched them linked to the new
folder, as Sphinx does; the builds that read them from the cache linked to the old one, so
the book's text changed.

Diagnostics by code (clean builds; the TSV files have them per builder):

+------------------------------+---------+--------+-------+------------------------------+
| Code                         | CPython | Django | Flask | What it is                   |
+==============================+=========+========+=======+==============================+
| ``rst.directive.unknown``    | 1,032   | 166    | 8     | Directives the projects'     |
| (error)                      |         |        |       | Python extensions define:    |
|                              |         |        |       | CPython ``availability``     |
|                              |         |        |       | 474, ``audit-event`` 228,    |
|                              |         |        |       | ``impl-detail`` 88,          |
|                              |         |        |       | ``corresponding-type-slot``  |
|                              |         |        |       | 80, ``deprecated-removed``   |
|                              |         |        |       | 67, ``soft-deprecated`` 56,  |
|                              |         |        |       | ``grammar-snippet`` 30, 6    |
|                              |         |        |       | others 9; Django ``console`` |
|                              |         |        |       | 166; Flask ``tabs`` 8        |
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
| ``build.label.duplicate``    | 1       |        |       | ``customization``, in        |
| (warning)                    |         |        |       | ``using/windows.rst`` and    |
|                              |         |        |       | ``reference/datamodel.rst``  |
|                              |         |        |       | (the sources')               |
+------------------------------+---------+--------+-------+------------------------------+
| ``build.xref.unresolved``    | 1       |        |       | ``:ref:`` to                 |
| (warning)                    |         |        |       | ``asyncio_example_gather``,  |
|                              |         |        |       | which the sources do not     |
|                              |         |        |       | define                       |
+------------------------------+---------+--------+-------+------------------------------+
| ``typst.raw.foreign``        | 6       |        |       | ``.. raw:: html`` blocks     |
| (warning, PDF only)          |         |        |       | left out of the book         |
+------------------------------+---------+--------+-------+------------------------------+

No diagnostic is about a construct Docutils or Sphinx itself defines. The content of an
unknown directive is left out of the output: CPython's 474 "Availability" notes, Django's
166 ``console`` examples, and Flask's 8 tab sets are missing from both HTML and PDF until
the directives are declared or migrated (see the compatibility guide).

Missing content and broken links
--------------------------------

Source documents against pages: every document that is not excluded was read and has a page
(CPython 555 documents read of 562 source files, 587 pages with the generated ones; Django
675 of 679, 678 pages; Flask 76 of 76, 79 pages). The source files without a page are all
excluded by ``exclude_patterns`` or not documents: CPython ``README.rst``,
``includes/*.rst`` (3), ``constraints.txt``, ``requirements.txt``,
``tools/removed-ids.txt``; Django ``README.rst`` (``.txt`` is its source suffix),
``requirements.txt``, two files under the excluded ``_theme/``. No document failed to read.

Internal links of the HTML output (``tools/linkcheck``: every ``href`` inside the site must
name an existing file, and its ``#fragment`` an id or name on that page):

======= ===== ============== ============= ===============
Manual  Pages Internal links Missing files Missing anchors
======= ===== ============== ============= ===============
CPython 587   176,157        589           0
Django  678   245,481        0             0
Flask   79    8,539          0             0
======= ===== ============== ============= ===============

CPython's 589: 587 links, one per page, to ``index.html``, which CPython's own
``tools/templates/layout.html`` names (``pathto('index')``) and which Sphinx renders from
the ``indexcontent.html`` template through ``html_additional_pages``. Guidedog did not
render Jinja template pages then, so the page was missing (its ``root_doc`` is
``contents``); it does now (see the last section). The other two are inert: ``PAGEURL``, a
placeholder ``improve-page.rst``'s script fills in, and ``lifecycle.dot.css``, an
``<?xml-stylesheet?>`` instruction inside an SVG that ``c-api/lifecycle.rst`` includes as
raw HTML.

Re-check at the revision that closed the Sphinx comparison
----------------------------------------------------------

At Guidedog revision ``95f6d72336ea8d66a55825faacc59e33a0246a4c`` (which links the names in
C signatures and expressions, warns about duplicate object descriptions, and adds the other
fixes the comparison with Sphinx's test roots found), the three HTML builds were run again,
clean, from the same sources and configuration: every diagnostic count above is unchanged,
and the link check finds CPython's internal links grown to 178,706 (the C API's signatures
now link their types) with the same 589 missing files and no missing anchor; Django's
245,481 and Flask's 8,539 links all arrive.

Re-check after html_additional_pages and the intersphinx cache fix
------------------------------------------------------------------

At Guidedog revision ``a55641e`` (which renders ``html_additional_pages`` and keeps a moved
inventory's link base in the intersphinx cache), built with ``-o:speed`` on the same host,
from copies of the same sources. CPython's ``conf.toml`` gained the table
``guidedog migrate`` now carries over from its ``conf.py``:

.. code:: diff

   +[html_additional_pages]
   +download = "download.html"
   +index = "indexcontent.html"

-  CPython HTML, clean (8.1 s while other jobs ran on the machine, 780 MB) and unchanged
   (0.8 s): the same 1,041 diagnostics both times. It writes 589 pages, the two more being
   ``index.html`` and ``download.html``, rendered from CPython's own
   ``tools/templates/indexcontent.html`` and ``download.html`` through its ``layout.html``.
   The link check finds 178,783 internal links and 2 missing files (the inert ``PAGEURL``
   and ``lifecycle.dot.css`` above), no missing anchor: the 587 links to ``index.html`` now
   arrive.
-  Flask PDF, clean (9.7 s, 697 MB, fetching the eight inventories) and then unchanged: the
   first build after the clean one keeps the book (``0 output(s) written, 1 unchanged``,
   0.22 s), with the same 12 diagnostics. Seven of Flask's eight inventories had moved (to
   ``/en/stable/``, ``/en/21/``, or ``/en/3.1.x/``); the cache now records each inventory's
   base, so every build links as the one that fetched it.

Re-check at revision 9bdcd20
----------------------------

At Guidedog revision ``9bdcd201be7c979f7614160019e3f20ea18ff7b2`` (no uncommitted changes to
tracked files; it adds the shared build memory budget, ``--budget`` 1024 MiB by default,
journaled publication of outputs, manifest, and environment, typed reader ids, stack guards,
and narrowed library exports), built with ``odin build cmd/guidedog -o:speed`` (binary
SHA-256 ``96f7d2c925ef653f9fb39cfe917e3941ec95099db810ac1ecaefc0e99c6da739``) on the same
host, all twelve builds were run again with the commands above, from fresh copies of the
same sources and the same ``conf.toml`` files (CPython's with ``[html_additional_pages]``),
one build at a time.

======= ============== ======================== ============================ ===========
Manual  Builder        Clean: time, peak memory Unchanged: time, peak memory Diagnostics
======= ============== ======================== ============================ ===========
CPython HTML           8.41 s, 531 MB           0.80 s, 138 MB               1,041
CPython PDF (136.9 MB) 78.8 s, 15.3 GB          1.62 s, 252 MB               1,047
Django  HTML           13.1 s, 320 MB           0.50 s, 93 MB                166
Django  PDF (52.8 MB)  29.2 s, 5.4 GB           0.69 s, 156 MB               166
Flask   HTML           7.84 s, 330 MB           0.12 s, 59 MB                12
Flask   PDF (6.4 MB)   11.4 s, 698 MB           0.23 s, 207 MB               12
======= ============== ======================== ============================ ===========

-  Every build exited 0; the diagnostics by builder, severity, and code are those of the TSV
   files, clean and unchanged alike. No build reported a budget or limit refusal.
-  Every unchanged build left every page and book untouched (same modification times); it
   rewrites only ``environment.json`` and ``<builder>.outputs.json`` in
   ``_build/.doctrees``, with the same bytes, as revision ``70708cd`` does.
-  Pages and links: CPython 589 pages and 178,783 internal links with the 2 inert missing
   files and no missing anchor; Django 678 pages and 245,481 links, Flask 79 pages and 8,539
   links, all arriving.
-  Peak memory of the HTML builds is lower (CPython 773 to 531 MB, Django 616 to 320 MB).
-  The clean HTML times are higher than in the first record, but not because of Guidedog:
   its CPU time is about the same (CPython 3.2 s user, 1.2 s system), and ``strace -c``
   shows 91% of the wall time in the ``fsync`` that publishes each output file (1,742 of
   them, 7.7 ms each on the host's ZFS that day). The binary of revision ``70708cd``,
   rebuilt and run on the same copy at the same time, took the same 10 s with the same
   number of ``fsync`` calls. Django's and Flask's clean builds also include fetching their
   inventories.

Re-check at revision 92bcd68
----------------------------

At Guidedog revision ``92bcd6839df3d6ba90ad81ac7fb30eb58e3ffc03``, the source of the binary.
It is ``9bdcd20`` plus these changes: each build owns an arena that ``project.release``
frees; outputs are written without flushing and then flushed together, several at a time,
before the journal; a damaged cached object is read again from its source; an unchanged
build writes no records; the CLI summary is reworded; and the library exports fewer names.
The binary was built with ``odin build cmd/guidedog -o:speed`` (SHA-256
``6b4e60fe71f886236930f4807a6c2e293c9ac3e4039713ce2765229fb33fba00``) on the same host. All
twelve builds were run again with the commands above, from the same copies of the sources
and the same ``conf.toml`` files, one build at a time, each manual from an empty ``_build``.
The CPython PDF build ran with nothing else running on the machine.

======= ============== ======================== ============================ ===========
Manual  Builder        Clean: time, peak memory Unchanged: time, peak memory Diagnostics
======= ============== ======================== ============================ ===========
CPython HTML           2.40 s, 521 MB           0.75 s, 137 MB               1,041
CPython PDF (136.9 MB) 77.0 s, 15.3 GB          1.51 s, 240 MB               1,047
Django  HTML           3.62 s, 330 MB           0.48 s, 93 MB                166
Django  PDF (52.8 MB)  28.6 s, 5.4 GB           0.72 s, 151 MB               166
Flask   HTML           8.33 s, 369 MB           0.09 s, 59 MB                12
Flask   PDF (6.4 MB)   8.44 s, 696 MB           0.20 s, 208 MB               12
======= ============== ======================== ============================ ===========

-  Every build exited 0. The diagnostics by builder, severity, and code match the TSV files
   exactly, for the clean and the unchanged builds. No build reported a budget or limit
   refusal, and no build wrote anything to stderr except the JSON diagnostics.

-  Unchanged builds now rewrite nothing. Every file and folder under ``_build`` has the same
   inode and modification time before and after the unchanged build. That includes
   ``.doctrees/environment.json``, ``<builder>.outputs.json``, and the cached objects, and
   it holds for all six builder runs (1,791, 1,150, 2,890, 1,435, 364, and 189 entries). A
   run without ``--diagnostics=json`` on the unchanged CPython HTML output prints
   ``write    0 output(s) written, 555 unchanged`` and
   ``✓ build succeeded, 1039 error(s), 2 warning(s). Output in _build/html``.

-  Pages and links are the same as at ``9bdcd20``. CPython has 589 pages and 178,783
   internal links; only the 2 inert missing files fail, and no anchor is missing. Django has
   678 pages and 245,481 links, and Flask has 79 pages and 8,539 links, all arriving.

-  Flushing. Under ``strace -f -c -e trace=fsync,fdatasync``, the clean CPython HTML build
   makes 1,817 ``fsync`` calls, 0.12 to 0.27 s in total (68 to 149 µs each). ``9bdcd20``
   made 1,742, one per file. The 75 extra are the output folders, which ``host.sync_all``
   now flushes so the new names in them are durable (``_build`` has 50 folders). To compare
   on the same day, the ``9bdcd20`` binary was rebuilt on the host. Its build takes 1,742
   ``fsync`` calls, 19.1 s in total (11 ms each), as it did then. Three clean CPython HTML
   builds each, taken in turns on the same copy:

   =========== ====================== ================
   Revision    Wall time              Peak memory
   =========== ====================== ================
   ``9bdcd20`` 8.80 s, 8.72 s, 8.06 s 543, 559, 538 MB
   ``92bcd68`` 2.29 s, 2.36 s, 2.55 s 534, 538, 527 MB
   =========== ====================== ================

   The clean HTML build is back to the 2.4 s of the first record, down from 8.41 s at
   ``9bdcd20``. Flushing unflushed files together lets the storage handle them in groups
   instead of one wait per file. CPU time is 3.7 s user and 2.1 s system, against 3.2 s and
   1.2 s at ``9bdcd20``. The extra system time most likely comes from the flushing threads.

-  Flask's clean HTML build is the only one that peaks higher (330 to 369 MB) and takes
   longer (7.84 to 8.33 s). Its CPU time is 0.75 s, so the rest of the time goes to fetching
   its eight inventories, which varies from run to run. Nothing else in that build changed.

Re-check at revision 3e5ed2e
----------------------------

At Guidedog revision ``3e5ed2e622e3c0d9736797cf41ddc1767c6b10dd`` (no uncommitted changes to
tracked files; it publishes through the spare output folder ``.<builder>.next``, plans a
commit before deciding it, starts worker threads through ``host.run_workers``, and refuses
memory before allocating it). The tree was copied fresh to the same host (Linux
7.0.0-28-generic, 32 cores, Typst 0.15.1), and the binary was built with
``odin build cmd/guidedog -o:speed`` (SHA-256
``e9fd0ea20d8bb139d212e45d187e1e052acc28f1a539ba88b33beb7641e99d6d``). The twelve builds
were run with the commands above, one at a time, from the same copies of the sources and the
same ``conf.toml`` files, each manual from an empty ``_build``.

-  The clean HTML builds of Django and Flask crashed: exit 70, ``host.crash``,
   ``signal SIGSEGV``, no diagnostics. The crash reporter caught the fault, and nothing was
   published. The other ten builds exited 0, with the diagnostics of the TSV files.
-  Repeated clean HTML builds with ``-j auto`` crash now and then: 2 of 25 for Django and 1
   of 25 for Flask. Under ``gdb``, a new reading thread faults in Odin's thread entry
   (``t.procedure(t)``, ``core/thread/thread_unix.odin:55``), while the main thread is still
   in ``pthread_create`` for the next thread, called from ``host.run_workers``
   (``lib/host/workers.odin:25``, from ``read_pending``, ``lib/project/read.odin:102``).
-  The cause: ``run_workers`` created and started each thread in turn
   (``thread.create_and_start_with_poly_data2``), and allocated each one's ``Thread`` from
   ``context.allocator``, the build's arena. ``read_pending`` gives the workers that same
   arena, through a mutex allocator, but ``run_workers`` allocates past the mutex. So the
   main thread allocated thread N+1 while threads 1 to N were already allocating from the
   arena, and the arena, which is not thread-safe, handed out corrupted memory. This came in
   with ``a1de07b``. Until then, the thread pool took the mutex allocator.
-  The fix, ``2bb9103``: ``run_workers`` creates every thread before it starts any, and
   destroys them only after every one has joined. So it allocates nothing while a worker
   runs. A test (``expect_no_allocation_while_workers_run``, in
   ``lib/host/crash_test.odin``) watches the allocator while the workers run. It fails
   before the fix and passes after.

The twelve builds were run again at revision ``2bb9103fc4285400cddb650f7f9b32f01e1606a7``
(``3e5ed2e`` plus the fix above and a fix to an undone commit that left ``.restore`` links
in ``.doctrees``). The binary was built the same way, SHA-256
``6c47130ede73bc885a4595ac7c87a7d5495b0eb014664fe278f5d183e2be2da1``.

======= ============== ======================== ============================ ===========
Manual  Builder        Clean: time, peak memory Unchanged: time, peak memory Diagnostics
======= ============== ======================== ============================ ===========
CPython HTML           2.65 s, 504 MB           0.68 s, 114 MB               1,041
CPython PDF (136.9 MB) 76.0 s, 15.3 GB          1.57 s, 246 MB               1,047
Django  HTML           2.63 s, 294 MB           0.47 s, 86 MB                166
Django  PDF (52.8 MB)  28.7 s, 5.4 GB           0.74 s, 160 MB               166
Flask   HTML           4.24 s, 334 MB           0.09 s, 57 MB                12
Flask   PDF (6.4 MB)   10.2 s, 698 MB           0.19 s, 210 MB               12
======= ============== ======================== ============================ ===========

-  Every build exited 0. The diagnostics by builder, severity, and code match the TSV files
   exactly, for the clean and the unchanged builds. No build reported a budget or limit
   refusal, and no build wrote anything to stderr except the JSON diagnostics.
-  Clean HTML builds with ``-j auto``, repeated from an empty ``_build``: 0 crashes in 101
   runs (CPython 16, Django 41, Flask 41, counting the table's). Before the fix, 3 of 50
   crashed.
-  Unchanged builds rewrite nothing. Every file and folder under ``_build`` has the same
   inode and modification time before and after the unchanged build, for all six builder
   runs (1,791, 1,150, 2,890, 1,435, 364, and 189 entries). A build that publishes into an
   empty ``_build`` renames the next folder into place, so these runs have no spare output.
   The spare output was checked on Django: a clean HTML build, then a build after one edit
   to ``index.txt``, leaves ``_build/.html.next`` (1,463 of 4,353 entries). An unchanged
   build after that prints ``write 0 output(s) written, 675 unchanged`` and leaves every
   entry, the spare output included, with the same inode and modification time.
-  Pages and links are the same as at ``92bcd68``. CPython has 589 pages and 178,783
   internal links; only the 2 inert missing files (``PAGEURL``, ``lifecycle.dot.css``) fail,
   and no anchor is missing. Django has 678 pages and 245,481 links, and Flask has 79 pages
   and 8,539 links, all arriving.
-  Peak memory and times are about those of ``92bcd68``. Flask's clean HTML time includes
   fetching its eight inventories.
