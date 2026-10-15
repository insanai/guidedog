Comparison with Sphinx over its test roots
==========================================

We build a project with Sphinx and with Guidedog and compare the two HTML outputs. Here is one line of the last record:

::

    test-numfig@numfig=1: pages=4/0 ids=58/0 links=62/0 numbers=40/0 numrefs=10/0 inventory=25/0 warnings=3/0 identical=4

Sphinx's test root ``test-numfig``, built with ``-D numfig=1``, has four pages. Both builders wrote them, and on each page the 58 ids, the 62 internal links, the 40 numbers and the 10 ``:numref:`` texts are the same. So are the 25 entries of ``objects.inv`` and the 3 places where the builders warned. All four pages are identical in what we compare.

Sphinx is the comparison, not the specification. Where the two differ, either we fixed Guidedog or the difference is explained below.

Running the comparison
----------------------

``cargo xtask sphinxdiff-roots`` builds 42 of Sphinx 9.1.0's own test roots (those whose features Guidedog has natively) with a pinned ``sphinx-build`` and with Guidedog, and compares each pair. It builds ``test-numfig`` twice: as Sphinx's tests build it by default (``numfig`` off) and with ``numfig`` on. The pinned Sphinx (9.1.0, with Docutils 0.22.4 and Pygments 2.19.2) lives in a separate verification environment; Guidedog never runs Python.

::

    python3 -m venv ~/verify-venv
    ~/verify-venv/bin/pip install sphinx==9.1.0 docutils==0.22.4 pygments==2.19.2
    cargo xtask sphinxdiff-roots ~/verify-venv/bin/sphinx-build ~/sphinx-9.1.0/tests/roots /tmp/sd tests/integration/data/sphinxdiff/results.txt

The roots folder is ``tests/roots`` of Sphinx's source distribution (``sphinx-9.1.0.tar.gz``). The task builds ``target/release/guidedog`` with Cargo, unless ``--guidedog PATH`` (or ``GUIDEDOG``) names one. For each root it copies the project to ``/tmp/sd/<root>/src``, turns its ``conf.py`` into ``conf.toml`` with ``guidedog migrate``, and builds it with ``sphinx-build -b html -D html_theme=basic`` and with ``guidedog build -b html --diagnostics=json``. The folder keeps both outputs, both logs, ``status.txt`` with both exit statuses, and ``diff.txt``, the full comparison.

The record ``tests/integration/data/sphinxdiff/results.txt`` holds the last run: the date, the host, Sphinx's version, Guidedog's revision, the command, and one line per root.

The run of 15 October 2026 checked the Rust implementation against the same pinned Sphinx 9.1.0. It agrees with the earlier record on every root but one. In ``test-images`` Sphinx 9.1.0 now reports one warning where the record had two, and the two builds still agree. The run also found one regression in the port, which we fixed before recording it: the reader had relabelled errors in a document's content (an invalid directive option, an invalid option block) as warnings, where Docutils and Sphinx report them as errors. Now the severity stays as the reader reports it, the build still succeeds, and ``-W`` fails it, as with Sphinx.


One comparison
--------------

``cargo xtask sphinxdiff`` compares two builds we made ourselves:

::

    cargo xtask sphinxdiff build/sphinx build/guidedog -v --warnings sphinx.log guidedog.log docs

The first folder is Sphinx's output with the basic theme, whose body is ``<div class="body">``. The second is Guidedog's, whose body is ``<article class="gd-article">``. ``-v`` lists every difference. ``--warnings`` adds sphinx-build's log (its standard error), Guidedog's ``--diagnostics=json`` log, and the project folder both read. The last line is a summary for scripts: ``summary:`` followed by ``name=compared/differing`` pairs, or ``summary: sphinx=failed`` when Sphinx wrote no pages.

What is compared
----------------

Per root, as ``name=compared/differing``. A count that repeats is counted as often as it appears, so a link written twice where Sphinx writes it three times differs by one.

- ``pages``: the HTML pages each builder wrote. ``identical`` counts the pages where nothing below differs.
- ``ids``: the ``id`` attributes in each page's body. The theme's chrome is left out.
- ``links``: the internal link targets of each page's body, as ``page#fragment`` from the site's root.
- ``numbers``: section numbers and the numbers of figures, tables and code blocks.
- ``numrefs``: the texts of Sphinx's ``:numref:`` links. Each must be the text of a Guidedog link to the same target.
- ``inventory``: the entries of ``objects.inv``.
- ``warnings``: the warnings and errors per source document and severity. The lines are listed in ``diff.txt`` but not counted: Docutils and Sphinx often name the first line of a directive, Guidedog the line at fault (the option, the role, the underline). Guidedog's notes about the configuration (``config.*``) are left out, since ``guidedog migrate`` writes the configuration.

``sphinx=failed`` means Sphinx's own build stopped with no pages.

Remaining differences, explained
--------------------------------

``test-directive-include``
   The root includes a file that does not exist, which stops Sphinx ("Sphinx error!", no pages). Guidedog reports the missing include and the missing root document and writes the other pages. Sphinx's tests use this root for single documents, not for full builds.

``test-numbered-circular``
   ``sub`` lists ``index``, which lists ``sub`` in a numbered toctree. Sphinx stops with a Python recursion error. Guidedog warns about the cycle (``build.toctree.cycle``) and about ``index`` being numbered twice (``build.toctree.renumbered``), and builds.

``test-prolog``
   ``markdown.md`` is read by a parser the root writes in Python (``prolog_markdown_parser``), which Guidedog does not run. So Guidedog has no ``markdown`` page and warns where the toctree lists it. With ``myst_parser`` in ``extensions`` Guidedog reads the page (the ``sphinx`` end-to-end tests check ``rst_prolog`` and ``rst_epilog`` there).

``test-toctree-maxdepth``
   ``qux.rst``, which no toctree includes, has a second numbered toctree listing ``foo`` and ``bar``. Both builders warn twice that they are numbered already. Sphinx takes numbered toctrees in the arbitrary order of a Python set, so between runs it warns at ``index.rst`` or at ``qux.rst``. Guidedog takes them in reading order and always warns at ``qux.rst``.

``test-footnotes``
   In the ``contents`` directive's entry for a title that holds a citation reference, Docutils copies the reference with its id. So Sphinx's page has ``id="id1"`` twice and a link inside a link. Guidedog writes the title's text in the entry, so the id stays unique.

``test-search``
   Guidedog warns that ``.. raw:: latex`` is left out of the HTML page (``html.raw.format``). Sphinx drops it silently. Guidedog reports content a page does not show; ``.. only:: latex`` around the block avoids the warning.

History
-------

The first comparison (29 September 2026) found differences in ``test-domain-cpp`` (ids and links), ``test-numfig`` (ids and links), and ``test-images``. We fixed them before the record's first run. The record's comparisons of numbers, numref texts and warnings, and the counting of repeated links, found the differences fixed in these commits: ``:numref:`` to a labelled figure, table or code block, explicit numref formats, and their warnings; figure numbers across toctree documents; section numbers of toctree documents among subsections; toctree warnings (lines, repeated entries, cycles through the root, documents numbered twice); glob patterns listing a document twice; ``:any:`` for Python and JavaScript; C function parameters; links in C and C++ signatures and expressions; C++ roles naming another kind of declaration; duplicate object descriptions; ``extlinks_detect_hardcoded_links``; ``contents`` as the root document; the lines of missing images and duplicate declarations; ``-D name=0|1``.

The Odin implementation made that record with its own ``tools/sphinxdiff``. The Rust port keeps the comparison as ``cargo xtask sphinxdiff`` with the same rules and the same summary, so a new record can be read line by line against the old one.

The reStructuredText reader is checked against Docutils itself in :doc:`rst-conformance`.
