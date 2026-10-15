Evidence records
================

**These records concern the Odin implementation of Guidedog** (releases up to 0.6.3), which
this Rust port replaces. They are kept unchanged as history: commands such as
``odin build``, paths under ``lib/`` and ``native/``, and test names like ``*_test.odin``
refer to that implementation. User-facing behaviour (commands, options, ``conf.toml`` keys,
directives, roles, output) carries over, so the comparisons and baselines remain useful; the
build, test, memory and sanitizer details do not describe the Rust code. New evidence for
the Rust implementation gets new, dated files beside these.

.. list-table::
   :header-rows: 1
   :widths: 2 3

   * - Record
     - What it shows
   * - :doc:`verification`
     - The documentation build record of 1 October 2026
   * - :doc:`manuals`
     - CPython, Django and Flask built as HTML and PDF (30 September 2026); per-manual
       diagnostics in ``cpython-diagnostics.tsv``, ``django-diagnostics.tsv`` and
       ``flask-diagnostics.tsv``
   * - :doc:`beta-review-20261011`
     - The beta release review: fixes, verification, support scope
   * - :doc:`i18n-privacy-20261011`
     - Documentation privacy and translation review
   * - :doc:`plugin-review-20261013`
     - Native publisher review (0.6.1)
   * - :doc:`plugin-review-20261014`
     - Build plugin review (0.6.2)
   * - :doc:`visual-docs-review-20261014`
     - Visual documentation review
   * - :doc:`release-0.6.3-20261014`
     - Release checks for 0.6.3

.. toctree::
   :hidden:

   verification
   manuals
   beta-review-20261011
   i18n-privacy-20261011
   plugin-review-20261013
   plugin-review-20261014
   visual-docs-review-20261014
   release-0.6.3-20261014
