reStructuredText inline markup and resolution: coverage ledger
==============================================================

This ledger tracks ``guidedoc-rst-inline`` (ported from the Odin
``lib/readers/rst/inline`` package) against its baseline.

Baseline: the Docutils *reStructuredText Markup Specification* (sections "Inline
Markup", "Hyperlink Targets", "Footnotes", "Citations", "Substitution Definitions")
and *reStructuredText Interpreted Text Roles*. Status is **done** (with the test that
covers it, in ``crates/guidedoc-rst-inline/src/tests/``), **deviation** (implemented
differently on purpose, with the reason), and there are no unimplemented items.
Tests this crate does not have are tests of the block reader (``guidedoc-rst``,
which drives these transforms from reStructuredText) or of the RST conformance suite.

Recognition rules
-----------------

.. list-table::
   :header-rows: 1

   * - Rule
     - Status
     - Tests
   * - 1\. start-string at text start or after white space, ``' " ( [ { < - / :``, or non-ASCII Ps/Pi/Pf/Pd/Po
     - done
     - ``start_and_end_contexts``, ``recognition_rules_reject_markup_inside_words``
   * - 2\. start-string followed by non-white space
     - done
     - ``recognition_rules_reject_markup_inside_words``
   * - 3\. end-string preceded by non-white space
     - done
     - ``unterminated_markup_warns_and_stays_text``, ``escaped_white_space_inside_markup``
   * - 4\. end-string at text end or before white space, ``- . , : ; ! ? \ / ' " ) ] } >``, or non-ASCII Pe/Pi/Pf/Pd/Po
     - done
     - ``start_and_end_contexts``
   * - 5\. start-string between matching brackets or quotes is text (international quote pairs)
     - done
     - ``quoted_start_strings_are_text``, ``unicode_brackets_and_quotes``
   * - 6\. end-string separated from start-string by at least one character
     - done
     - ``unterminated_markup_warns_and_stays_text`` (``****``)
   * - 7\. backslash disables recognition, except the end-string of inline literals
     - done
     - ``backslash_escapes``, ``inline_literals_keep_backslashes``
   * - leftmost start-string wins; first valid end-string ends it; markup does not nest
     - done
     - ``nested_looking_markup``, ``emphasis_and_strong``
   * - text after a construct or a failed start-string counts as a text start
     - done
     - covered by the scanner's ``base``; ``hyperlinks_split_by_problematic_markup``
   * - escapes: backslash-space and backslash-newline removed, other escaped characters literal
     - done
     - ``backslash_escapes`` (``H\ :sub:``2``\ O``, joined lines)
   * - interpreted text may end after an escaped space; emphasis may not
     - done
     - ``escaped_white_space_inside_markup``
   * - unterminated start-string: warning ``rst.inline.unterminated``, kept as text
     - done
     - ``unterminated_markup_warns_and_stays_text``
   * - Unicode categories Ps/Pe/Pi/Pf/Pd/Po (and Pc, which is no context)
     - done
     - ``punctuation_categories_come_from_unicode_data``; ``punctuation_table.rs`` was generated from DerivedGeneralCategory-17.0.0.txt (the Odin ``tools/punctuation``); quote and bracket pairs for rule 5 stay in ``chars.rs``
   * - white space is Python's ``str.isspace`` (Zs, or bidi WS/B/S): U+200B, U+200E/F, U+FEFF are not white space, U+001C-U+001F are
     - done
     - ``white_space_is_pythons_isspace``; ``SPACES`` in ``punctuation_table.rs`` comes from UnicodeData-17.0.0.txt
   * - character-level inline markup (``character_level_inline_markup``): no context is needed around start- and end-strings
     - done
     - conformance ``extra/character-level.rst``

Inline constructs
-----------------

.. list-table::
   :header-rows: 1

   * - Construct
     - Node
     - Status
     - Tests
   * - ``*emphasis*``
     - Emphasis
     - done
     - ``emphasis_and_strong``
   * - ``**strong**``
     - Strong
     - done
     - ``emphasis_and_strong``
   * - :literal:`\`\`literal\`\``
     - Code, backslashes kept
     - done
     - ``inline_literals_keep_backslashes``
   * - :literal:`\`interpreted\`` with default role
     - per role; "" = title-reference
     - done
     - ``default_role``
   * - :literal:`:role:\`text\`` and :literal:`\`text\`:role:`
     - per role
     - done
     - ``role_names_ignore_case_and_may_follow_the_text``
   * - both prefix and suffix role
     - warning ``rst.inline.multiple-roles``
     - done
     - ``interpreted_text_problems``
   * - role together with ``_`` suffix
     - warning ``rst.inline.role-reference``
     - done
     - ``interpreted_text_problems``
   * - ``name_``, ``name__`` (simplename rule)
     - Reference (+Anonymous)
     - done
     - ``simple_references``
   * - :literal:`\`phrase\`_`, :literal:`\`phrase\`__`
     - Reference (+Anonymous)
     - done
     - ``phrase_references``
   * - :literal:`\`text <uri>\`_`
     - Link{External, name}
     - done
     - ``embedded_uris_and_aliases``
   * - :literal:`\`text <uri>\`__`
     - Link{External} without name
     - done
     - ``embedded_uris_and_aliases``
   * - :literal:`\`<uri>\`_` (text from URI)
     - Link
     - done
     - ``embedded_uris_and_aliases``
   * - white space in embedded URIs removed, escaped white space kept
     - done
     - ``embedded_uris_and_aliases``
     - 
   * - embedded email address
     - Link{Email}
     - done
     - ``embedded_uris_and_aliases``
   * - :literal:`\`text <name_>\`_`
     - Reference{name}; the text defines an indirect target
     - done
     - ``embedded_uris_and_aliases``, ``embedded_uris_define_targets``
   * - :literal:`\`text <name\\_>\`_` and URI-shaped aliases are URIs
     - Link
     - done
     - ``embedded_uris_and_aliases``
   * - :literal:`_\`inline target\``
     - Inline_Target > inlines
     - done
     - ``inline_targets``
   * - ``[1]_``, ``[#]_``, ``[#label]_``, ``[*]_``
     - Footnote_Reference
     - done
     - ``footnote_and_citation_references``
   * - ``[CIT2002]_``
     - Citation_Reference
     - done
     - ``footnote_and_citation_references``
   * - ``|name|``
     - Substitution_Reference
     - done
     - ``substitution_references``
   * - ``|name|_``, ``|name|__``
     - Reference > Substitution_Reference
     - done
     - ``substitution_references``
   * - standalone absolute URI (known schemes, RFC 2396 characters, last-character rule)
     - Link{External}
     - done
     - ``standalone_hyperlinks``
   * - standalone email address
     - Link{Email}
     - done
     - ``standalone_hyperlinks``
   * - URIs are not recognized across a problematic start-string
     - done
     - ``hyperlinks_split_by_problematic_markup``
     - 
   * - line breaks
     - Soft_Break; spaces inside literals and roles
     - done
     - ``plain_lines_match_the_placeholder_shape``, ``roles_turn_line_breaks_into_spaces``
   * - source spans for every node
     - done
     - ``spans_point_at_the_source``, ``lines_keep_their_own_offsets``
     - 
   * - unknown URI scheme
     - deviation
     - Docutils then ignores every standalone URI in that text chunk; Guidedog skips only that candidate (``unknown:thing`` stays text, later URIs still link).
     - 
   * - line breaks inside inline literals and code
     - deviation
     - Become spaces: inline code renders on one line. Math and raw text keep them (LaTeX comments end at line breaks), as Docutils does.
     - 
   * - problematic nodes
     - deviation
     - Failed markup stays as text next to a report; there is no "problematic" node kind.
     - 
   * - implicit PEP/RFC references in running text (``pep_references``, ``rfc_references``, ``pep_base_url``, ``rfc_base_url``): "PEP n" and "RFC n" as whole words, across a line break
     - Link{External}
     - done
     - ``implicit_pep_and_rfc_references``; conformance ``extra/implicit-references*.rst``
   * - ``trim_footnote_reference_space``: white space and line breaks before footnote and citation references removed
     - Text trimmed
     - done
     - ``implicit_pep_and_rfc_references``; conformance ``extra/implicit-references.rst``
   * - smart quotes (``smart_quotes`` On or Alt, ``language``, ``smart_quotes_locales``): quotation marks by language (en de fr es it nl sv fi da pl cs ru ja zh zh-tw; own CLDR-based table), apostrophes, "--" en dash, "---" em dash, "..." ellipsis; literals, code, math, raw, literal blocks, comments, options untouched; escaped marks stay; class ``language-xx`` selects a language
     - Text
     - done
     - ``smart_quotes_follow_the_language``; conformance ``extra/smartquotes-*.rst``, ``cyrillic.rst``, ``misc_rst_html5.rst``, ``latex_leavevmode.rst``, ``rst_html5_tuftig.rst``, ``standalone_rst_latex.rst``

Standard roles
--------------

.. list-table::
   :header-rows: 1

   * - Role (aliases)
     - Node
     - Status
     - Tests
   * - emphasis
     - Emphasis
     - done
     - ``standard_roles``
   * - strong
     - Strong
     - done
     - ``standard_roles``
   * - literal
     - Code, escapes processed
     - done
     - ``standard_roles``
   * - code
     - Code, backslashes kept; Language from a derived role
     - done
     - ``standard_roles``, ``custom_roles``
   * - math
     - Math, backslashes kept
     - done
     - ``standard_roles``
   * - subscript (sub)
     - Subscript
     - done
     - ``standard_roles``
   * - superscript (sup)
     - Superscript
     - done
     - ``standard_roles``
   * - title-reference (title, t)
     - Title_Reference
     - done
     - ``standard_roles``
   * - pep-reference (PEP)
     - Link to ``https://peps.python.org/pep-NNNN/``, text "PEP n"
     - done
     - ``pep_and_rfc_roles``
   * - rfc-reference (RFC), with ``#section``
     - Link to ``https://datatracker.ietf.org/doc/html/rfcN``, text "RFC n"
     - done
     - ``pep_and_rfc_roles``
   * - invalid PEP / RFC number
     - errors ``rst.role.pep``, ``rst.role.rfc``
     - done
     - ``pep_and_rfc_roles``
   * - abbreviation (ab), acronym (ac)
     - Abbreviation
     - done
     - ``standard_roles``
   * - raw used directly
     - error ``rst.role.raw``
     - done
     - ``raw_role_needs_a_format``
   * - raw-derived role with ``:format:``
     - Raw_Inline{Format}
     - done
     - ``raw_role_needs_a_format``
   * - custom role (``.. role:: name``)
     - Role{text = name; Class}
     - done
     - ``custom_roles``
   * - derived role ``name(base)`` with ``:class:`` (default class = role name)
     - base node + Class
     - done
     - ``custom_roles``
   * - role names case-insensitive
     - done
     - ``role_names_ignore_case_and_may_follow_the_text``, ``standard_roles_are_found_by_alias``
     - 
   * - unknown role
     - error ``rst.role.unknown``, source kept as text
     - done
     - ``interpreted_text_problems``, ``default_role``
   * - PEP URL trailing slash
     - deviation
     - Follows the project direction (``.../pep-0008/``); Docutils' default template has no slash.
     - 
   * - ``code`` role class "code"
     - deviation
     - Not added; the Code kind already says it.
     - 
   * - specialized roles named-reference, anonymous-reference, footnote-reference, citation-reference, substitution-reference, target, uri-reference
     - the construct each names
     - done
     - ``specialized_roles_become_their_constructs`` (``resolve/specialized.rs``); Docutils reports them as not implemented, see the RST conformance notes

Resolution
----------

.. list-table::
   :header-rows: 1

   * - Rule
     - Status
     - Tests
   * - explicit external target
     - done
     - ``external_and_internal_targets``
   * - internal target points at the next element (skips targets, comments, substitution definitions; ascends)
     - done
     - ``external_and_internal_targets``
   * - chained internal targets share the element
     - done
     - ``external_and_internal_targets``
   * - internal target with no following element points at itself
     - done
     - ``internal_target_at_the_end_points_at_itself``
   * - indirect targets followed to the end of the chain
     - done
     - ``indirect_targets_follow_chains``
   * - indirect target cycle: error ``rst.target.cycle``
     - done
     - ``indirect_target_cycles_and_dangling_names``
   * - indirect target to unknown name: error ``rst.target.unresolved``
     - done
     - ``indirect_target_cycles_and_dangling_names``
   * - anonymous references paired in order with anonymous targets
     - done
     - ``anonymous_references_pair_in_order``
   * - anonymous count mismatch: error ``rst.reference.anonymous``
     - done
     - ``anonymous_mismatch_is_an_error``
   * - embedded URI / alias references define targets
     - done
     - ``embedded_uris_define_targets``
   * - inline internal targets
     - done
     - ``names_and_inline_targets``
   * - ``:name:`` option (Name attribute; Custom "name" on Directive nodes)
     - done
     - ``names_and_inline_targets``
   * - section titles are implicit targets for their Section
     - done
     - ``sections_are_implicit_targets``
   * - explicit names win over section titles (note ``rst.target.implicit``)
     - done
     - ``explicit_names_win_over_section_titles``
   * - repeated section titles are not referenceable (note; error ``rst.reference.ambiguous`` on use)
     - done
     - ``repeated_section_titles_are_ambiguous``
   * - duplicate explicit targets: error ``rst.target.duplicate``, unless same URI
     - done
     - ``duplicate_explicit_targets``
   * - unresolved reference: error ``rst.reference.unresolved`` quoting the reference
     - done
     - ``unresolved_references_are_errors``
   * - Referenced flag on targets, footnotes, citations, inline targets
     - done
     - ``external_and_internal_targets``, ``footnotes_are_numbered_and_referenced``
   * - auto-numbered footnotes get the lowest numbers not used as names, in order
     - done
     - ``footnotes_are_numbered_and_referenced``
   * - ``[#label]_`` refers to a labelled auto footnote; ``[#]_`` to unlabelled ones in order
     - done
     - ``footnotes_are_numbered_and_referenced``
   * - ``[*]_`` refers to symbol footnotes in order; value = 1-based symbol ordinal
     - done
     - ``footnotes_are_numbered_and_referenced``, ``footnote_symbols_repeat``
   * - manual ``[1]_`` refers to the footnote labelled 1
     - done
     - ``footnotes_are_numbered_and_referenced``
   * - too many ``[#]_`` / ``[*]_``: error ``rst.footnote.references``; unknown labels unresolved
     - done
     - ``footnote_reference_problems``
   * - citations by label
     - done
     - ``citations_resolve_by_label``
   * - substitutions: case-sensitive match, then case-insensitive
     - done
     - ``substitutions_resolve_case_sensitively_first``
   * - undefined substitution: error ``rst.substitution.undefined``
     - done
     - ``substitutions_resolve_case_sensitively_first``
   * - duplicate definition: error ``rst.substitution.duplicate``, last one wins
     - done
     - ``substitution_duplicates_keep_the_last``
   * - circular substitution: error ``rst.substitution.cycle``
     - done
     - ``substitution_cycles_are_errors``
   * - nesting beyond ``limits.max_expansion``: error ``rst.substitution.depth``
     - done
     - ``substitution_nesting_is_bounded``
   * - title promotion (DocTitle rule; comments, targets, substitution definitions, title/meta directives ignored)
     - done
     - ``lone_section_becomes_the_title``, ``title_and_meta_directives_do_not_block_promotion``
   * - subtitle promotion
     - done
     - ``lone_subsection_becomes_the_subtitle``
   * - no promotion with several sections, leading body, or trailing elements
     - done
     - ``promotion_needs_a_single_section``
   * - promotion only with ``promote_title``
     - done
     - ``promotion_is_optional``
   * - references and targets follow the promoted title
     - done
     - ``references_to_the_title_follow_it``
   * - duplicate explicit targets: both names unusable, warning ``rst.target.duplicate``; names from embedded URIs only noted; indirect targets and aliases to one name agree
     - done
     - ``duplicate_explicit_targets``, ``duplicate_targets_quote_their_first_lines``; conformance ``extra/duplicate-targets.rst``
   * - numbers of unlabelled auto footnotes as names (:literal:`\`2\`_`, and ``[2]_`` finds such a footnote, or any target named 2)
     - done
     - ``unlabelled_footnote_numbers_are_names``; conformance ``data/footnote-targets.rst``
   * - chained targets: an internal target followed by another target takes its destination; before a comment, substitution definition, footnote, or citation it points at itself
     - done
     - ``chained_target_takes_the_uri``, ``external_and_internal_targets``; conformance ``extra/target-before-comment.rst``
   * - contents directive: its title (or the language's word for contents) is an implicit name
     - done
     - conformance ``docs/user/rst/demo.rst``
   * - messages quote the smallest span: a section's title, a target's or footnote's first line; an explicit target just before a section of the same name is not reported
     - done
     - ``shadowed_section_name_quotes_the_title``, ``target_before_its_section_is_not_reported``
   * - section names: from the title's text, literal, math, and raw text and substitution reference names as written
     - done
     - conformance ``data/section_titles.rst``

Bounds and allocation
---------------------

.. list-table::
   :header-rows: 1

   * - Property
     - Status
     - Tests
   * - parse buffers reused across inline contexts (``InlineState`` keeps them)
     - done
     - every parse; no per-call buffer allocation in steady state
   * - resolution uses growing ``Vec``/``HashMap`` tables
     - deviation
     - The Odin counted-and-sized scratch tables and the "no allocator calls" test are gone; ``every_construct_in_one_document`` keeps the document of that test.
   * - work charged through ``Builder::spend``; limit is a result
     - done
     - ``work_limit_is_a_result``
   * - text pool growth is bounded by ``Limits::max_text_bytes``; a ``Capacity`` result
     - done
     - ``text_limit_is_a_result``
   * - linear-time recognition on hostile input (cached end-string searches, simplename, scheme, email, URI tail memos)
     - done
     - ``hostile_input_stays_linear``
   * - symbol-table probe limit (``rst.resolve.limit``)
     - deviation
     - Rust maps have no probe limit, so the message is never reported.
