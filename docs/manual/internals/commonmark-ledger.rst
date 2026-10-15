Markdown reader: coverage ledger
================================

Baseline: CommonMark 0.31.2 (the ``commonmark`` reader), and the Markdown that Sphinx
projects write with myst-parser 0.16.1 (the ``myst`` reader): its Syntax Guide, Optional
Syntax, and Sphinx configuration pages. Implemented independently; myst-parser's test
fixtures serve as reference data only (:doc:`commonmark-conformance`). Node shapes:
:doc:`ast`, "Markdown (MyST) mapping". The crate is ``crates/guidedoc-commonmark``.

Status: **done** = implemented and tested; **partial** = implemented with a stated gap.
Tests are the crate's unit tests and ``tests/`` (``spec.rs``, ``reader.rs``, ``myst.rs``,
``myst_conformance/``); the Sphinx layer tests the reader with its directives and roles.

Reader and settings
-------------------

.. list-table::
   :header-rows: 1
   :widths: 60 10 30

   * - Item
     - Status
     - Tests
   * - Two readers: ``READER`` (``commonmark``: pure CommonMark 0.31.2, CommonMark's
       tree, no settings; 652/652 examples) and ``MYST_READER`` (``myst``: defaults as
       myst-parser 0.16.1)
     - done
     - ``reader_ids_name_their_profiles``, ``settings_default_as_myst_parser``,
       ``commonmark_spec_examples``
   * - ``Settings`` through ``ReadOptions::extension`` (``read_extension``), refused by
       the ``commonmark`` reader; none means ``Settings::default()``
     - done
     - ``reader_ids_name_their_profiles``
   * - ``commonmark_only``: CommonMark syntax, sections from headings
     - done
     - ``syntax_can_be_disabled``
   * - ``enable_extensions``, default dollarmath as in 0.16.1
     - done
     - ``optional_typography_and_links`` and others
   * - ``disable_syntax``, markdown-it rule names; unknown names warn
     - done
     - ``syntax_can_be_disabled``
   * - ``url_schemes`` (default http, https, mailto, ftp; ``None`` for every scheme)
     - done
     - ``links_resolve_as_myst_does``
   * - ``heading_anchors``; ``heading_slug_func`` reported as unsupported
     - done
     - ``heading_anchors_link_sections``
   * - ``substitutions``, ``sub_delimiters``, ``html_meta``, ``footnote_transition``,
       ``dmath_*``, ``linkify_fuzzy_links``, ``words_per_minute``, ``docname``,
       ``env_config``, ``today``
     - done
     - ``substitutions_evaluate_names_and_filters``, ``dollar_math_follows_its_settings``,
       ``front_matter_fields_meta_and_errors``, ``footnotes_collect_in_reference_order``
   * - Extension interface (``Extension`` trait): shape, directive, role, table, begin,
       end, resolve; ``Host::parse_body``, ``parse_inline``, ``list_items``,
       ``include_paths``, ``set_classes_for_next``, per-read extension state
     - done
     - Sphinx layer

Core syntax (on in ``myst``, off with ``commonmark_only``)
----------------------------------------------------------

.. list-table::
   :header-rows: 1
   :widths: 60 10 30

   * - Construct
     - Status
     - Tests
   * - Headings make sections (level jumps warn and nest one deeper); nested headings
       are rubrics
     - done
     - ``headings_make_sections_and_nested_headings_rubrics``
   * - Directives: :literal:`\`\`\`{name}`, ``~~~{name}``, ``:::{name}`` (colon_fence);
       argument or first content line by the directive's shape
     - done
     - Sphinx layer; conformance
   * - Directive options: ``:key: value`` lines and ``---`` YAML blocks; a blank line
       after them dropped; non-scalar values refused
     - done
     - ``yaml_scalars_follow_myst_option_parsing``
   * - Roles ``{name}`content```, any number of backticks, content across lines
     - done
     - ``roles_and_directives_need_handlers``
   * - Targets ``(name)=``, comments ``%``, block breaks ``+++``
     - done
     - ``targets_comments_and_block_breaks``
   * - Front matter: fields, bibliographic Markdown values, ``html_meta``,
       ``substitutions``; errors reported
     - done
     - ``front_matter_fields_meta_and_errors``
   * - Footnotes ``[^label]``: manual and automatic numbers, definitions anywhere, at the
       end in reference order, unreferenced dropped, duplicates warned
     - done
     - ``footnotes_collect_in_reference_order``
   * - GFM tables with alignment
     - done
     - ``tables_definition_and_field_lists``
   * - Links: URLs of allowed schemes, documents by path (with anchors), targets by
       name, heading anchors
     - done
     - ``links_resolve_as_myst_does``, ``heading_anchors_link_sections``
   * - ``figure-md``; ``sub-ref`` role (wordcount-words, wordcount-minutes, today,
       version, release)
     - done
     - ``colon_fences_and_markdown_figures``; conformance ``basic/content``
   * - Reference definitions shared by the document and nested content, in reading
       order; duplicates warned
     - done
     - conformance
   * - Included Markdown: front matter skipped; ``:relative-images:``,
       ``:relative-docs:``
     - done
     - Sphinx layer

Optional syntax (``enable_extensions``)
---------------------------------------

All of amsmath, colon_fence, deflist, dollarmath, fieldlist, html_admonition,
html_image, linkify, replacements, smartquotes, and tasklist are **done** (tests in
``tests/myst.rs``). substitution is **partial**: Jinja expressions are limited to names,
dotted names, strings, ``+``/``~``, parentheses, and the filters and methods ``upper``,
``lower``, ``title``, ``capitalize``, ``trim``/``strip``, ``replace``, ``default``, and
``format``.

Resources and limits
--------------------

.. list-table::
   :header-rows: 1
   :widths: 60 10 30

   * - Property
     - Status
     - Tests
   * - Hostile CommonMark and MyST input ends in a result within bounded time
     - done
     - ``hostile_inputs_end_in_results``, ``hostile_myst_inputs_end_in_results``
   * - Nesting depth bounded (``commonmark.nesting``, builder depth, YAML depth 64)
     - done
     - ``hostile_inputs_end_in_results``, ``deep_flow_collections_are_refused``

Known gaps
----------

-  Jinja in substitutions: loops, conditionals, arithmetic, and filters beyond those
   listed are reported (``myst.substitution``) and expand to nothing.
-  A substitution whose value is a directive, used inline, stays inline text (a
   paragraph cannot hold blocks in the Guidedoc tree); myst-parser nests the directive.
-  ``dmath_double_inline``: ``$$...$$`` inside a paragraph is inline Math.
-  ``myst_update_mathjax`` and ``myst_mathjax_classes`` concern the HTML output, not
   reading.
-  Reading allocates: the reader builds its per-read skeleton on the heap, as the engine's design allows.
