Sphinx layer: coverage ledger
=============================

This ledger tracks ``guidedoc-sphinx`` against its baseline. Test names refer to the unit tests in
``crates/guidedoc-sphinx/src/tests/`` and the integration tests in its ``tests/``;
``integration_tests.*`` names are the project host's tests.

Baseline: the Sphinx 9.1 documentation of directives, roles, domains, and their
options; node shapes from ``docs/manual/internals/ast.rst``, "Sphinx layer". Implemented
independently on the base reader's extension interface (guidedoc-rst's ``Extension``).

Status: **done** = implemented and tested; **partial** = implemented with a stated gap.
Tests are in ``src/tests/``. Every directive also accepts the options listed; an
unknown option is the base reader's ``rst.directive.option-unknown`` error.

Reader, configuration, interface
--------------------------------

.. list-table::
   :header-rows: 1

   * - Item
     - Status
     - Tests
   * - Reader id ``sphinx``, VERSION, CONFORMANCE Partial
     - done
     - ``reader_metadata``
   * - Document marked read by Sphinx (``value`` 1), so anchors add the ids of Sphinx's AutoNumbering (captioned figures, titled tables, captioned code blocks) and SortIds
     - done
     - ``integration_tests.sphinx_transforms_give_their_ids``
   * - ``Config`` embeds ``rst.Settings`` first; ``default_config()`` with Sphinx defaults
     - done
     - ``reader_metadata``
   * - ``rst_prolog`` / ``rst_epilog`` as registered sources: the prolog after the leading field lines (as ``_prepend_prologue``), the epilog after the document
     - done
     - ``prolog_and_epilog_surround_the_document``, ``prolog_follows_the_leading_fields``, ``rst_prolog_and_epilog_follow_sphinx``
   * - ``default_role``, ``primary_domain``, ``highlight_language``
     - done
     - ``default_role_from_config``, ``code_block_language_follows_highlight``, ``default_domain_directives``
   * - ``add_function_parentheses``, ``add_module_names``
     - done
     - ``function_parentheses_follow_config``, ``python_module_options_and_add_module_names``
   * - ``toc_object_entries``, ``toc_object_entries_show_parents``: each signature's ``toc`` name as its domain's ``_toc_entry_name`` makes it (Python, JavaScript, C, C++, reStructuredText, confval); ``:no-contents-entry:`` and ``:nocontentsentry:``
     - done
     - ``toc_entry_names_follow_each_domain``, ``toc_entries_can_be_left_out``; ``integration_tests.object_descriptions_have_toc_entries``
   * - ``extlinks``, ``manpages_url``, ``math_number_all``, ``docname``
     - done
     - ``extlinks_roles``, ``samp_file_abbr_manpage``, ``math_number_all_labels_every_equation``
   * - ``todo_include_todos``, ``show_authors``
     - done
     - ``todo_follows_todo_include_todos``, ``authors_follow_show_authors``
   * - ``numfig``
     - done (carried for the host; reading does not depend on it)
     - none
   * - ``smartquotes``, PEP/RFC base URLs, trimmed footnote space
     - done (in ``rst.Settings``)
     - base reader tests
   * - Extension interface: directive/role tables, lookup, begin/end hooks
     - done
     - guidedoc-rst's extension tests
   * - Markdown: ``myst_settings`` gives the commonmark reader these directives and roles (``markdown.rs``); nested content is Markdown; ``eval-rst`` parses reStructuredText; MyST's include options ``:relative-images:``, ``:relative-docs:``; list-table and table read Markdown lists and tables; resolution as for reStructuredText
     - done
     - ``markdown_*`` in ``markdown.rs tests``; ``integration_tests.myst_conformance``
   * - Built-in extension names (``extension_supported``), graphviz and doctest included
     - done
     - ``graphviz_directives``, ``doctest_directives``
   * - Content errors (unknown directive or role) keep the document under ``Policy.keep_documents_with_errors``
     - done
     - ``unknown_constructs_keep_the_document_under_the_policy``

Directives
----------

.. list-table::
   :header-rows: 1

   * - Directive
     - Options
     - Status
     - Tests
   * - toctree
     - maxdepth, caption, name, class, hidden, includehidden, numbered, titlesonly, glob, reversed
     - done
     - ``toctree_entries_and_options``, ``toctree_without_options``
   * - only, ifconfig (content optional); sections inside
     - none
     - done
     - ``only_and_ifconfig``, ``sections_in_only``; ``integration_tests.sections_in_only_follow_their_expression``
   * - index
     - name; entry types single, pair, double, triple, see, seealso, ``!``
     - done
     - ``index_directive_entries``, ``index_entry_with_wrong_parts_warns``
   * - glossary
     - sorted; several terms, classifiers, comments
     - done
     - ``glossary_terms_and_ids``, ``glossary_problems_warn``
   * - productionlist
     - group; continuation lines; ``token``, ``~g:t``
     - done
     - ``productionlist_tokens``
   * - centered, rubric (heading-level, class, name), hlist (columns), acks
     - as listed
     - done
     - ``centered_rubric_hlist``, ``hlist_needs_a_bullet_list``
   * - seealso
     - class, name
     - done (base)
     - ``seealso_and_rst_class``
   * - rst-class
     - none
     - done
     - ``seealso_and_rst_class``
   * - versionadded, versionchanged, deprecated, versionremoved
     - none
     - done
     - ``version_directives``
   * - sectionauthor, moduleauthor, codeauthor
     - none
     - done
     - ``authors_follow_show_authors``
   * - tabularcolumns, highlight (linenothreshold, force), default-domain, program, py:currentmodule
     - as listed
     - done
     - ``state_directives_emit_nothing``, ``code_block_language_follows_highlight``
   * - default-role
     - none
     - done
     - ``default_role_accepts_sphinx_roles``
   * - code-block, sourcecode, code (content optional for code-block and sourcecode, as in Sphinx); the caption parsed as inline markup (a Caption child), as for literalinclude and graphviz
     - linenos, lineno-start, emphasize-lines, caption, name, class, dedent, force; code also number-lines
     - done
     - ``code_block_sphinx_options``, ``code_block_language_follows_highlight``, ``code_block_dedent``, ``code_block_bad_emphasis_is_refused``
   * - literalinclude
     - language, linenos, lineno-start, lineno-match, emphasize-lines, caption, name, class, dedent, lines, start-after, end-before, start-at, end-at, prepend, append, diff, pyobject, encoding, tab-width, force
     - done
     - ``literalinclude_selection``, ``literalinclude_pyobject_and_dedent``, ``literalinclude_diff``, ``literalinclude_tab_width_and_errors``
   * - math
     - label, name, class, nowrap, no-wrap
     - done
     - ``math_directive_labels``, ``math_number_all_labels_every_equation``
   * - todo, todolist
     - class, name
     - done
     - ``todo_follows_todo_include_todos``
   * - postlist (optional limit argument)
     - path, category, tags, author, series, year, sort (``date`` | ``-date``), style (``list`` | ``cards``), excerpts
     - done
     - ``body_edits_and_unused_templates_leave_other_pages_cached``
   * - graphviz, graph, digraph
     - alt, align, caption, layout, graphviz_dot, name, class; file argument
     - done
     - ``graphviz_directives``
   * - doctest, testcode, testoutput, testsetup, testcleanup (sphinx.ext.doctest; tests are not run): group argument; shown as Code_Block pycon / python / none, hidden (testsetup, testcleanup, ``:hide:``) as Comment
     - hide, options, pyversion, skipif, trim-doctest-flags, no-trim-doctest-flags; config ``trim_doctest_flags``
     - done
     - ``doctest_directives``

Roles
-----

.. list-table::
   :header-rows: 1

   * - Role
     - Status
     - Tests
   * - doc, ref, numref, any, term, keyword, token, download
     - done
     - ``standard_domain_roles``
   * - option, envvar (index entries + target)
     - done
     - ``option_and_envvar_roles``
   * - command, dfn, mailheader, makevar, mimetype, newsgroup, program, regexp
     - done
     - ``text_roles``
   * - kbd (compound keys, multi-word keys)
     - done
     - ``kbd_role_splits_keys``
   * - guilabel, menuselection (accelerators, ``-->``)
     - done
     - ``gui_roles_mark_accelerators``
   * - samp, file, abbr, manpage
     - done
     - ``samp_file_abbr_manpage``
   * - pep, rfc, index
     - done
     - ``pep_rfc_index_roles``
   * - math
     - done (base)
     - base tests
   * - eq, math:numref
     - done
     - ``math_directive_labels``
   * - extlinks roles
     - done
     - ``extlinks_roles``
   * - ``!``, ``~``, leading ``.``, explicit titles, context on each Xref
     - done
     - ``python_roles_process_links``, ``other_domain_roles``
   * - unknown roles stay errors
     - done
     - ``unknown_roles_are_still_errors``

Domains
-------

.. list-table::
   :header-rows: 1

   * - Domain: object types
     - Status
     - Tests
   * - Every object directive: titles in its content begin sections (a title context of their own), which get no table of contents entry
     - done
     - ``sections_in_object_descriptions``; ``integration_tests.object_descriptions_have_toc_entries``
   * - py: module, currentmodule, function, data, exception, class, attribute, property, method, staticmethod, classmethod, decorator, decoratormethod, type
     - done
     - ``python_function_signature``, ``python_class_nesting``, ``python_other_objects``; a module declared first in a section gives the section its id (MoveModuleTargets, host): ``integration_tests.module_targets_name_their_section``
   * - py options: no-index/noindex, no-index-entry, no-contents-entry, no-typesetting, module, type, value, async, abstractmethod/abstract, classmethod, staticmethod, final, canonical, annotation, platform, synopsis, deprecated
     - done; canonical noted as an alias the project registers, resolves, and lists in objects.inv (priority -1, not searched); on py:type, shown as the aliased type
     - ``python_class_nesting``, ``python_other_objects``, ``object_options_and_duplicates``, ``python_module_options_and_add_module_names``, ``python_canonical_names``; ``integration_tests.canonical_names_are_aliases``
   * - py signatures: prefixes, parameters, defaults, annotations (Xref), ``*args``, ``**kw``, ``/``, ``*``, return annotation, optional brackets
     - done
     - ``python_function_signature``, ``python_signature_fallbacks``
   * - py roles: data, exc, func, class, const, attr, type, meth, mod, obj
     - done
     - ``python_roles_process_links``
   * - Info fields (DocFieldTransformer) in every object's content: py param/parameter/arg/argument/keyword/kwarg/kwparam with type/paramtype, var/ivar/cvar with vartype, raises/raise/exception/except, returns/return, rtype, ``:meta:`` dropped; c (function, macro) param/parameter/arg/argument with type, retval/retvals, returns, rtype; cpp tparam, and for functions param, throws/throw/exception, retval, returns; js (function, method, class) argument/arg/parameter/param with type/paramtype, throws, returns, rtype; other fields kept with the name capitalized; types from ``:type x:`` or ``:param T x:``; collapsing groups of one; Python's splitting at brackets, commas, ``or``, ``of``, ``|``, ``...`` and after ``Literal``; ``~``, ``.``, ``typing.`` and None as parse_reftarget; group names translated (``Config::labels``)
     - done
     - ``info_field_list*`` in ``docfields.rs tests``
   * - c: member, var, function, macro, struct, union, enum, enumerator, type, alias, namespace, namespace-push/-pop; a function's named parameters (``params``, Sphinx's functionParam); "Struct.member" names; anonymous entities (``@name``, shown as "[anonymous]", found through their parent); every object the scope of its content, as CObject.before_content makes it, so references there find a function's parameters; references looked up from the scope they are in outward
     - done
     - ``c_declarations``, ``alias_and_namespace``, ``c_parameters_and_qualified_members``, ``c_anonymous_entities``; ``integration_tests.c_symbols_in_the_inventory``, ``integration_tests.c_anonymous_entities_in_the_inventory``
   * - c roles: member, data, var, func, macro, struct, union, enum, enumerator, type, expr, texpr; the names in C signatures (types, qualifications, parameters other than their own names) and in expr and texpr are ``c:identifier`` cross-references looked up from the declared symbol or the enclosing object outward, text when nothing is found (not a member after ``.`` or ``->``; C keywords never)
     - done
     - ``other_domain_roles``
   * - cpp: class, struct, union, function, member, var, type, concept, enum, enum-struct, enum-class, enumerator, alias, namespace, namespace-push/-pop; declarations parsed as Sphinx's DefinitionParser parses them (templates, operators, declarators, expressions, attributes; the fallback scan for expressions it cannot parse), with the ids of every id version (1 to 4, the older ones while free in the document), display names ("[anonymous]"), symbol paths, and template and function parameter names; names in signatures as ``cpp:identifier`` cross-references from the declared symbol
     - done
     - ``cpp_declarations_and_ids``, ``cpp_ids_of_every_version``
   * - cpp roles: any, class, struct, union, func, member, var, type, concept, enum, enumerator, expr, texpr; targets parsed as the C++ domain parses them, expr and texpr parsed as an expression or else a type and described as signatures are, with their names linked (a role naming a declaration of another kind warns, as ``_check_type`` does, and still links) ("()" after func and any targets), shorthand names and whole function declarations (``cpp:name``, ``cpp:rooted``, ``cpp:tdecls``, ``cpp:query``, ``cpp:own``), which the host looks up in the project's symbol tree as Sphinx does
     - done
     - ``other_domain_roles``; ``integration_tests.cpp_symbols_in_the_inventory``
   * - js: module, function, method, class, data, attribute; roles func, meth, class, data, attr, mod
     - done
     - ``javascript_objects``, ``other_domain_roles``
   * - rst: directive, directive:option (type), role; roles dir, role
     - done
     - ``rst_domain_objects``, ``other_domain_roles``
   * - std: program, option/cmdoption, envvar, describe, object, confval (type, default)
     - done
     - ``std_domain_objects``
   * - odin (Guidedog's own domain, ``src/domains/odin.rs``): package (synopsis, platform, deprecated, imports), currentpackage, procedure/proc, procgroup, struct, union, enum, enumerator, field, bitset, bitfield, type, const, var, foreign; options no-index, no-index-entry, no-contents-entry, no-typesetting, package, private, deprecated, availability, foreign; fields and enumerators nested as "Type.member"; ids ``odin-<package>.<name>`` and ``odin-package-<package>``; entries "name (procedure in core:fmt)"; later signatures of one directive (other targets) without id or entry
     - done
     - ``odin_procedure_signature``, ``odin_types_nest_their_members``, ``odin_other_objects``, ``odin_fields_and_no_index``
   * - odin signatures as Odin writes them: attributes, ``::`` values, procedure short form ``name(params)``, calling conventions, parameters with names, types, defaults, ``using``, ``#c_vararg``, ``..``, ``$T`` (never links, nor T after it, nor a nesting type's), named results, tags, where clauses, procedure groups, records' polymorphic parameters, union variants; type names as ``odin:type`` Xrefs with ``odin:package`` and ``odin:type`` context, import aliases (``:imports:``) followed to their package; built-in types and keywords as text
     - done
     - ``odin_procedure_signature``, ``odin_short_forms_and_polymorphic_names``
   * - odin roles: pkg, proc, type, const, var, field, enumerator, obj (``~``, leading ``.``, aliases followed); info fields param/parameter/arg/argument with type, result (named results), returns/return, rtype; toc names per ``toc_object_entries_show_parents``; odin:autopackage, autoproc, autoprocgroup, autotype, autoconst, autovar through ``Config::autodoc`` (domain "odin")
     - done
     - ``odin_roles_and_toc_entries``, ``odin_fields_and_no_index``, ``odin_generating_directives_need_a_host``
   * - default-domain lookup of directives and roles, std fallback
     - done
     - ``default_domain_directives``, ``default_domain_changes_role_lookup``
   * - Declared object types (Config.object_types: add_object_type, add_crossref_type) and directive aliases
     - done
     - ``declared_object_types_follow_sphinx``
   * - Duplicate ids fall back to Sphinx's prefix-N / idN
     - done
     - ``object_options_and_duplicates``

Known gaps
----------

- Python defaults and annotations are kept as written; Sphinx re-prints them through
  Python's unparser (quotes and spacing may differ). Type parameter lists (``[T]``) are
  shown as text.
- ``::`` literal blocks carry no Language; the highlight setting applies to code-block,
  literalinclude, and code only.
- Info fields: a use's content of several blocks follows its paragraph, where Sphinx
  nests the blocks inside it. C and C++ types and C++ exceptions link their names, as
  :c:expr: and :cpp:expr: do. ``python_use_unqualified_type_names`` is not read.
- Sections inside ``only``/``ifconfig`` are read as Sphinx reads them where sections are
  allowed; when the content moves up to its first title's level, text after the directive
  follows it there, where Sphinx leaves such text in the deeper section before it. In a
  body element (a list, a quote) and in Markdown, titles inside are errors.
- In Markdown, section titles inside ``eval-rst`` are errors: the two readers keep their
  own sections.

MyST conformance with the Sphinx layer
--------------------------------------

``crates/guidedoc-sphinx/tests/myst_conformance/`` runs guidedoc-commonmark's
differential harness with this layer's ``MystExtension``: every case must match
myst-parser 0.16.1 or be a documented difference (see :doc:`commonmark-conformance`).

- Fixtures: 218 cases, 215 match, 3 documented differences.
- Sphinx builds: 10 cases, 8 match, 2 documented differences.

Covered elsewhere
-----------------

- Reading allocates per read by design, so no test counts allocator calls. The extension interface for other dialects is covered by guidedoc-rst's own extension tests, and bounded C++ parsing by guidedoc-cpp's limit tests.
