reStructuredText block reader: coverage ledger
==============================================

This ledger tracks ``guidedoc-rst`` (ported from the Odin ``lib/readers/rst`` package,
without its ``inline/`` subpackage) against its baseline.

Baseline: the Docutils *reStructuredText Markup Specification* and *reStructuredText
Directives*, implemented independently (GDS 0003, "Reader conformance is explicit").
Inline markup, roles, and reference resolution are covered by :doc:`rst-inline-ledger`.

Status: **done** = implemented and tested (there are no partial or missing items). Test names refer to the
integration tests in ``crates/guidedoc-rst/tests/`` and the unit tests in its ``src/``.

Input and syntax details
------------------------

.. list-table::
   :header-rows: 1

   * - Construct
     - Status
     - Tests
   * - Line breaks ``\n``, ``\r\n``, ``\r``; final break starts no line
     - done
     - ``line_endings_are_normalized``
   * - Tab expansion to ``tab_width`` stops; spans map back through tabs
     - done
     - ``tabs_expand_to_tab_stops``, ``spans_are_exact_through_tabs_and_nesting``
   * - Form feed and vertical tab become spaces; trailing white space stripped
     - done
     - (all tests)
   * - UTF-8 byte order mark at the start of a source
     - done
     - ``byte_order_mark_is_skipped``
   * - Indentation, nested block parsing on column-cut line views
     - done
     - ``bullet_items_hold_blocks``, ``block_quotes_nest``
   * - Escaping in target names, URIs, field names, option values
     - done
     - ``target_uri_escaped_whitespace``, ``target_name_with_escaped_colon``, ``field_names_may_hold_escaped_colons``
   * - Tabs inside inline text expand to the tab stops of their source line (spans stay exact)
     - done
     - ``tabs_in_inline_text_expand_to_tab_stops``; conformance ``extra/tabs.rst``, ``extra/tabs-4.rst``
   * - Trailing Unicode white space (U+00A0, U+2000..U+200A, U+3000, ...) is stripped from lines and table cells
     - done
     - ``unicode_trailing_white_space_is_stripped``; conformance ``data/unicode.rst``, ``docs/ref/rst/mathematics.rst``


Body elements
-------------

.. list-table::
   :header-rows: 1

   * - Construct
     - Status
     - Tests
   * - Paragraphs
     - done
     - ``paragraphs_are_separated_by_blank_lines``
   * - Sections: underline, over+underline, inset titles, style order gives levels
     - done
     - ``underlined_titles_make_sections``, ``overlined_titles_may_be_inset``, ``over_and_under_style_differs_from_underline_only``
   * - Title underline/overline too short (warning), short adornment falls back to text
     - done
     - ``short_title_underline_warns``, ``short_underline_is_text``
   * - Inconsistent title level, incomplete/mismatched/missing title adornment (errors)
     - done
     - ``inconsistent_title_levels_are_errors``, ``incomplete_overline_title``, ``mismatched_overline``
   * - Title or transition inside a body element (error)
     - done
     - ``title_in_body_element_is_error``
   * - Sections continue across included files
     - done
     - ``included_sections_continue_the_hierarchy``
   * - Transitions; errors at start, end, and when adjacent
     - done
     - ``transitions_separate_body_elements``, ``transition_at_start_is_error``, ``transition_at_end_is_error``, ``adjacent_transitions_are_errors``
   * - Bullet lists ``* - + • ‣ ⁃``, tight flag
     - done
     - ``bullet_lists_use_every_bullet``, ``bullet_items_hold_blocks``, ``bullet_item_may_start_on_next_line``
   * - List item indentation fixed by the text on the marker line
     - done
     - ``list_item_text_fixes_the_indentation``
   * - Enumerated lists: arabic, alpha, roman, ``#``; formats ``1.`` ``1)`` ``(1)``; start value
     - done
     - ``enumerated_formats_and_sequences``, ``enumerated_list_start_value``
   * - Enumerated: sequence checks, second-line rule, i/I ambiguity, invalid roman
     - done
     - ``enumerated_sequence_break_starts_new_list``, ``enumerated_second_line_must_continue_sequence``, ``enumerated_needs_valid_second_line``, ``enumerated_ambiguity_prefers_alpha_in_sequence``, ``enumerated_first_i_is_roman``, ``enumerated_invalid_roman_is_text``
   * - Definition lists with classifiers (``:`` outside inline literals)
     - done
     - ``definition_lists_with_classifiers``, ``definition_term_keeps_inline_literal_colons``; classifier split skips inline literal and interpreted text spans before inline parsing
   * - Field lists (bibliographic fields are ordinary fields)
     - done
     - ``field_lists``, ``field_names_may_hold_escaped_colons``
   * - Option lists: short, long, DOS, ``=``/space/attached arguments, ``<a b>``, groups
     - done
     - ``option_lists_cover_every_form``, ``option_groups_and_next_line_descriptions``, ``option_like_line_without_description_is_text``
   * - Literal blocks: ``Para::``, ``Para ::``, lone ``::``, escaped ``\::``
     - done
     - ``literal_block_after_double_colon``, ``literal_block_marker_with_space``, ``literal_block_marker_alone``, ``literal_block_escaped_marker``
   * - Quoted literal blocks; inconsistent quoting (error); missing block (warning)
     - done
     - ``quoted_literal_block``, ``inconsistent_literal_quoting_is_error``, ``literal_block_missing_warns``
   * - Line blocks with continuation lines, empty lines, nesting
     - done
     - ``line_blocks_nest_by_indentation``
   * - Block quotes, attribution ``--``, ``---``, ``—``, several quotes in one block
     - done
     - ``block_quote_with_attribution``, ``block_quotes_nest``, ``attribution_dash_forms``
   * - Doctest blocks
     - done
     - ``doctest_blocks_keep_text``
   * - "Ends without a blank line" warnings; unexpected indentation (error)
     - done
     - ``list_without_blank_line_warns``, ``unexpected_indentation_is_error``
   * - Grid tables: row/column spans, header separator, cell blocks, UTF-8 cells
     - done
     - ``grid_table_with_header_and_spans``, ``grid_table_without_header``, ``grid_table_cells_hold_utf8``, ``grid_table_cell_paragraphs``
   * - Grid table errors: ragged edge, incomplete parse, two header separators
     - done
     - ``grid_table_ragged_edge_is_error``, ``grid_table_incomplete_is_error``, ``grid_table_two_header_separators_is_error``
   * - Simple tables: header, multi-line rows, column spans, growing last column
     - done
     - ``simple_table_with_header``, ``simple_table_column_spans``, ``simple_table_without_header``, ``simple_table_last_column_grows``
   * - Simple table errors: margin text, missing bottom, border mismatch
     - done
     - ``simple_table_text_in_margin_is_error``, ``simple_table_missing_bottom_is_error``, ``simple_table_border_mismatch_is_error``
   * - East Asian wide characters count two columns in tables and titles
     - done
     - ``wide_characters_fill_two_columns_in_grid_tables``, ``wide_characters_fill_two_columns_in_simple_tables``, ``wide_titles_need_underlines_of_their_display_width``
   * - Footnotes: ``[1]``, ``[#]``, ``[#label]``, ``[*]``, body on the next line
     - done
     - ``footnotes_take_every_label_form``, ``footnote_body_may_start_on_next_line``
   * - Citations
     - done
     - ``citations_use_simple_names``
   * - Hyperlink targets: external, multi-line URI, escaped white space, e-mail
     - done
     - ``external_target``, ``target_uri_spans_lines``, ``target_uri_escaped_whitespace``, ``target_email_becomes_email_link``
   * - Target names: escaped colons, backquoted phrases (also across lines)
     - done
     - ``target_name_with_escaped_colon``, ``target_name_quoted_phrase``, ``target_name_quoted_across_lines``
   * - Indirect targets (``name_``, ```phrase`_``), internal targets, anonymous ``.. __:`` and ``__``
     - done
     - ``indirect_targets``, ``internal_target_precedes_element``, ``anonymous_targets_both_forms``, ``escaped_trailing_underscore_is_a_uri``
   * - Malformed target (error)
     - done
     - ``malformed_target_is_error``
   * - Comments, empty comment ending a construct
     - done
     - ``comments_keep_their_text``, ``empty_comment_ends_a_list``
   * - Substitution definitions, name normalization, directive on the next line
     - done
     - ``substitution_replace``, ``substitution_names_are_normalized``, ``substitution_directive_on_next_line``
   * - Substitution names that span lines (joined with one space)
     - done
     - ``substitution_names_may_span_lines``; conformance ``extra/substitution-names.rst``
   * - Substitution errors: no directive, block directive, multi-paragraph replace
     - done
     - ``substitution_needs_a_directive``, ``substitution_rejects_block_directives``, ``substitution_replace_single_paragraph``


Directives
----------

.. list-table::
   :header-rows: 1

   * - Directive
     - Status
     - Tests
   * - attention, caution, danger, error, hint, important, note, tip, warning, seealso
     - done
     - ``specific_admonitions``, ``admonition_content_and_options``, ``admonition_needs_content``
   * - admonition (title, ``admonition-<id>`` class)
     - done
     - ``generic_admonition_gets_title_class``
   * - image (alt, height, width, scale, align, target, class, name, loading)
     - done
     - ``image_with_all_options``, ``image_uri_joins_lines``, ``image_target_reference``, ``image_vertical_align_is_error_in_block``, ``image_takes_no_content``, ``image_needs_uri``
   * - figure (figwidth, figclass, align, caption, legend, empty-comment legend; class directive and internal target before the caption apply to it)
     - done
     - ``figure_with_caption_and_legend``, ``figure_legend_without_caption``, ``figure_caption_must_be_paragraph``, ``figure_width_of_image_and_normalized_lengths``; ``:figwidth: image`` records Custom ``figwidth`` = ``image`` (renderers size the figure to the image); conformance ``data/standard.rst``
   * - topic (top level or sidebar only)
     - done
     - ``topic_with_title``, ``topic_not_allowed_in_body_elements``
   * - sidebar (optional title, subtitle, no nesting)
     - done
     - ``sidebar_with_subtitle``, ``sidebar_without_title``, ``sidebar_cannot_nest``
   * - line-block
     - done
     - ``line_block_directive_nests_lines``
   * - parsed-literal
     - done
     - ``parsed_literal_keeps_lines``
   * - code, code-block, sourcecode (language, number-lines, class, name)
     - done
     - ``code_with_language_and_numbers``, ``code_aliases``, ``code_accepts_one_argument``
   * - math (argument and blank-line groups)
     - done
     - ``math_argument_and_groups``
   * - rubric
     - done
     - ``rubric_with_class``
   * - epigraph, highlights, pull-quote
     - done
     - ``quote_directives_add_classes``
   * - compound, container
     - done
     - ``compound_and_container``
   * - table (title, class, name, align, width, widths)
     - done
     - ``table_directive_titles_a_table``, ``table_directive_needs_a_table``, ``table_directive_widths_must_match``, ``table_directive_auto_widths``
   * - csv-table (header, header-rows, stub-columns, widths, width, delim, quote, escape, keepspace, align, class, name, file, encoding)
     - done
     - ``csv_table_with_options``, ``csv_table_multiline_quoted_cell``, ``csv_table_delimiter_and_quote``, ``csv_table_escape_character``, ``csv_table_tab_delimiter``, ``csv_table_unclosed_quote_is_error``, ``csv_table_header_rows_exceed_data``, ``csv_table_and_raw_read_files``
   * - csv-table ``:url:``
     - done (refused)
     - ``csv_table_url_is_refused`` — the reader never fetches URLs (policy error)
   * - list-table (header-rows, stub-columns, widths, width, align, class, name)
     - done
     - ``list_table_with_header_and_stub``, ``list_table_uneven_rows_is_error``, ``list_table_needs_nested_lists``
   * - contents (title, depth, local, backlinks, class)
     - done
     - ``contents_options``, ``contents_defaults``
   * - sectnum, section-numbering (depth, prefix, suffix, start)
     - done
     - ``sectnum_options``
   * - header, footer
     - done
     - ``header_and_footer``
   * - target-notes
     - done
     - ``target_notes_directive``
   * - meta
     - done
     - ``meta_fields``, ``meta_needs_fields``
   * - include (literal, code, number-lines, encoding, tab-width, start/end line, start-after, end-before, class, name)
     - done
     - ``include_parses_in_place``, ``included_nodes_have_spans_in_their_source``, ``include_literal_and_code``, ``include_line_and_text_ranges``, ``include_inside_a_body_element``
   * - include ``:parser:``
     - done (refused)
     - ``include_problems_are_reported``
   * - include refusals, cycles, depth, disabled policy, missing provider
     - done
     - ``include_problems_are_reported``, ``include_cycles_are_errors``, ``include_depth_is_bounded``, ``include_without_provider_or_policy_is_refused``
   * - raw (format list, file, encoding, class; url refused)
     - done
     - ``raw_block_content``, ``raw_url_is_refused``, ``raw_needs_content``, ``csv_table_and_raw_read_files``, ``file_and_content_together_is_error``
   * - class (next element, skipping targets and comments; content; dangling error)
     - done
     - ``class_applies_to_next_element``, ``class_skips_targets_and_comments``, ``class_with_content_applies_to_each``, ``class_names_are_identifiers``, ``class_without_following_element_is_error``
   * - role (custom, derived from standard or custom roles; class, format, language)
     - done
     - ``custom_roles_are_defined``, ``role_with_unknown_base_is_error``, ``role_format_needs_raw_base``
   * - default-role
     - done
     - ``default_role_sets_role``, ``default_role_unknown_is_error``
   * - title
     - done
     - ``title_directive_sets_title``
   * - replace, unicode (codes, comments, ltrim/rtrim/trim), date (strftime codes)
     - done
     - ``substitution_unicode_codes``, ``substitution_unicode_trim``, ``substitution_date_formats``, ``date_without_today_warns``, ``replace_outside_substitution_is_error``
   * - image and raw in substitution definitions
     - done
     - ``substitution_image_with_target``, ``substitution_raw``
   * - Dialect directives, roles, hooks, and default substitutions (``Extension``)
     - done
     - ``a_dialect_adds_directives_roles_hooks_and_substitutions``, ``unknown_options_of_a_handler_are_reported``
   * - restructuredtext-test-directive
     - done (unknown)
     - ``test_directive_is_unknown`` — treated as an unknown directive
   * - unknown directives (error with extension-layer hint)
     - done
     - ``unknown_directive_is_error``
   * - option checking: unknown, duplicate, invalid value, non-field option line
     - done
     - ``unknown_option_is_error``, ``duplicate_option_is_error``, ``invalid_option_values_are_errors``, ``option_block_must_be_fields``
   * - argument and content checks
     - done
     - ``image_needs_uri``, ``code_accepts_one_argument``, ``image_takes_no_content``, ``admonition_needs_content``
   * - case-insensitive directive names
     - done
     - ``directive_name_is_case_insensitive``


Document structure and settings
-------------------------------

Settings reach the reader as ``guidedoc_rst::Settings`` through
``ReadOptions::extension``; none means ``Settings::default()``. A dialect passes its
``Settings`` (and its ``Extension``) to ``read_with`` directly.

.. list-table::
   :header-rows: 1

   * - Construct or setting
     - Status
     - Tests
   * - Document title and subtitle (``ReadOptions::promote_title``); header, footer, sectnum, target-notes, raw, and meta before the title do not block it
     - done
     - ``lone_section_becomes_the_title`` (inline), conformance ``docs/ref/rst/*.rst``
   * - Bibliographic fields (``docinfo``, default on): Field_List{Class docinfo} after title and subtitle; author, authors, organization, address, contact, version, revision, status, date, copyright recognized by the document language, case-insensitively, as Field{Custom docinfo = key}; authors split by ";" or ","; one paragraph per author from paragraphs or a bullet list; address Preformatted; other fields get a class from their name
     - done
     - ``bibliographic_fields_become_docinfo``, conformance ``extra/docinfo-*.rst``, ``data/standard.rst``, ``latex_docinfo.rst``
   * - Dedication and abstract fields become Topic{Class dedication / abstract} with the language's title, after the list
     - done
     - ``bibliographic_fields_become_docinfo``, conformance ``docs/ref/rst/mathematics.rst``
   * - RCS keywords (``$Date: 2002/08/27 13:04:05 $`` to 2002-08-27, ``$RCSfile: x,v $``, ``$Keyword: value $``)
     - done
     - ``bibliographic_fields_become_docinfo``, conformance ``extra/docinfo-keywords.rst``
   * - Bibliographic problems: not one paragraph, empty, repeated dedication/abstract, authors not splittable (warnings ``rst.docinfo.*``; the field stays ordinary)
     - done
     - conformance ``extra/docinfo-problems.rst``
   * - Section subtitles (``sectsubtitle``): a lone subsection's title becomes Paragraph{Class section-subtitle} second in the section; its content moves up a level
     - done
     - ``lone_subsection_becomes_section_subtitle``, conformance ``extra/section-subtitles*.rst``, ``standalone_rst_*.rst``
   * - Transitions that end a section move up to follow it
     - done
     - conformance ``data/standard.rst``
   * - Tab width (``tab_width``, else ``ReadOptions::tab_width``, else 8)
     - done
     - conformance ``extra/tabs-4.rst``
   * - ``:widths:`` gives tables class colwidths-auto or colwidths-given
     - done
     - ``table_directive_auto_widths``, ``csv_table_with_options``
   * - Lengths lose the space before the unit; units em ex ch rem vw vh vmin vmax px in cm mm Q pt pc %; a unitless figwidth gets px
     - done
     - ``figure_width_of_image_and_normalized_lengths``, conformance ``length_units.rst``
   * - Image in a substitution definition: alt defaults to the substitution name
     - done
     - conformance ``data/embed_images.rst``, ``data/latex-problematic.rst``
   * - Class directive before a footnote or citation classes that element
     - done
     - conformance ``rst_html5_tuftig.rst``
   * - include ``:start-after:`` / ``:end-before:`` text spanning lines
     - done
     - conformance ``footnotes.rst``
   * - math directive argument keeps relative indentation
     - done
     - conformance ``data/math_experiments.rst``
   * - Differential conformance with Docutils 0.22 (98 sources, all match; see :doc:`rst-conformance`)
     - done
     - ``tests/conformance.rs``


Hosts: another markup using these directives and roles
------------------------------------------------------

.. list-table::
   :header-rows: 1

   * - Property
     - Status
     - Tests
   * - ``Host`` (``src/host.rs``): ``body``, ``inline``, ``items`` callbacks; ``embed_begin``/``embed_end`` run the reader around a host that reads the main source; ``run_hosted`` checks and runs a directive the host split into parts; ``directive_shape`` describes one
     - done
     - ``a_host_runs_directives_whose_content_it_parses``; the Sphinx layer's Markdown tests
   * - Nested content of a hosted directive is parsed by the host (``parse_body``, ``inline``, the class directive's classes, list-table items); reStructuredText inside it (``parse_blocks``) is reStructuredText again
     - done
     - ``markdown_directives_parse_markdown_content``, ``markdown_eval_rst_and_math``
   * - The table directive gives its title and options to the host's table (``table_decorate``)
     - done
     - ``markdown_table_and_figure_directives``
   * - ``Parser::apply_host_role`` runs a role on literal content (no backslash escapes), as for interpreted text
     - done
     - ``markdown_roles_are_sphinx_roles``
   * - Without a host, every call is the reader's own: suites and Docutils conformance unchanged
     - done
     - all tests here; ``tests/conformance.rs``


Resources, limits, and allocation
---------------------------------

.. list-table::
   :header-rows: 1

   * - Property
     - Status
     - Tests
   * - Every construct reads, and rereads identically in a reused workspace
     - done
     - ``every_construct_reads_repeatedly`` (the Odin reader's no-allocation guarantee is not kept: see the crate README)
   * - Stack budget bounds nesting below ``max_depth``; huge depth limits still end in a ``Limit``
     - done
     - ``deep_lists_past_a_huge_depth_limit_hit_the_stack_limit``, ``stack_budget_bounds_nesting_below_max_depth``
   * - Depth limit on nesting (quotes, lists) is a result
     - done
     - ``deep_block_quotes_hit_the_depth_limit``, ``deep_lists_hit_the_depth_limit``
   * - Work limit bounds table scanning
     - done
     - ``large_grid_table_respects_the_work_limit``
   * - Node limit is a result
     - done
     - ``node_limit_is_a_result``
   * - Random markup terminates without internal or capacity failures
     - done
     - ``random_markup_terminates``


Shape notes for renderers
-------------------------

- Unicode bullets cannot be stored in ``ListInfo::delimiter`` (one byte): the delimiter is
  0 and the list carries attribute Custom ``bullet`` with the character.
- A block-level image with ``:target:`` is wrapped as Paragraph > Link/Reference > Image,
  because Link and Reference are inline kinds that cannot stand at block level.
- Section subtitles are Paragraph{Class section-subtitle} until the core allows Subtitle
  in Section; bibliographic fields carry Custom ``docinfo`` (English key); ``:figwidth: image``
  is Custom ``figwidth`` = ``image``.
- Directive nodes keep attribute Name for the directive name; ``:name:`` becomes Custom
  ``name`` (the resolver reads it there).
- csv-table and list-table without ``:widths:`` get equal relative widths (``1 1 …``).
- Unicode ``:ltrim:``/``:rtrim:`` (and ``:trim:``) become Custom attributes ``ltrim``/``rtrim``
  with value ``1`` on the SubstitutionDefinition.

