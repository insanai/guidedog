Incremental builds
==================

A second build of an unchanged project reads no document and writes no file, and a
change rebuilds exactly what it can affect. Digests decide this, and this page lists every input of every digest. The list comes from the crates that compute the digests (gd-conf, gd-env, gd-outputs, gd-resolve). When a digest gains an input, this page changes with it.

The model
---------

Guidedog's incremental build is a *verifying trace* store with *dynamic dependencies*
and *early cutoff*, in the terms of "Build Systems à la Carte" (Mokhov, Mitchell,
Peyton Jones). It is the same idea as rustc's red-green query system and salsa, at the
granularity of documents and output units.

* Each task (reading a document, writing an output unit) records the digests of the
  inputs it used. The next build recomputes those digests and reruns the task only when
  one differs (a *verifying* trace: values are not stored, only their digests).
* Some inputs are known only while the task runs: the labels, objects, documents and
  files a page's references look up. They are recorded as the task discovers them
  (*dynamic* dependencies) in a ``gd_env::Trace``, with the digest of each answer.
* A task whose inputs changed but whose result did not stops the change there (*early
  cutoff*). A document read again into the same object bytes has the same object
  digest, so no page that used it is written again. A label that moved within its page
  keeps its answer, so the pages that refer to it are kept.

There are two layers. The reading layer decides which documents are parsed again. The
writing layer decides which output units are written again. ``Generation::reuse``
(gd-outputs) is the single cutoff point of the writing layer.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Two layers, each with its own cutoff. Where the digests match, the work stops.
   :alt: A source document goes to the reading layer. If its digests match, the saved object is reused; otherwise the document is read again. The object then goes to the writing layer. If the unit's inputs and lookup answers match, the unit is kept; otherwise it is written again.

   digraph incremental {
     graph [rankdir=LR, bgcolor="transparent", pad="0.3", nodesep="0.35", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.15,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     source [class="gd-input", shape=note, fillcolor="#fbfaf3", label="Source and\ndependencies"];
     same_read [class="gd-choice", shape=diamond, fillcolor="#fff4e6", color="#ca8a04", fontcolor="#854d0e", label="Digests\nmatch?"];
     reuse [label="Reuse the object"];
     reread [label="Read again"];
     object [label="Object digest"];
     same_unit [class="gd-choice", shape=diamond, fillcolor="#fff4e6", color="#ca8a04", fontcolor="#854d0e", label="Inputs and\nanswers match?"];
     keep [class="gd-output", fillcolor="#eef4fa", label="Keep the unit"];
     write [class="gd-output", fillcolor="#eef4fa", label="Write the unit"];

     source -> same_read;
     same_read -> reuse [label="yes"];
     same_read -> reread [label="no"];
     reuse -> object;
     reread -> object;
     object -> same_unit;
     same_unit -> keep [label="yes"];
     same_unit -> write [label="no"];
   }

Digests
-------

A digest (``gd_conf::Digest``) is SHA-256 over parts, each followed by a zero byte, so
part boundaries count (``Digest::of_parts``, ``Digester``). A file's digest is that of
its bytes as one part, read in chunks (``gd_env::file_digest``).

The settings
------------

Project settings (``gd_conf::Loaded::settings``)
   conf.toml's text as read, then each ``-D`` override in order. Every output depends
   on it through the output settings.

Reading settings (``gd_env::reading_settings``)
   The engine's version and schema version; the executable stamp (the running
   program's size and modification time, so a new Guidedog never reuses what an older
   one read); ``language``, ``rst_prolog``, ``rst_epilog``, ``default_role``,
   ``primary_domain``, ``highlight_language``, ``today``, ``today_fmt``, ``version``,
   ``release``, ``project``, ``toc_object_entries_show_parents``, ``manpages_url``,
   ``intersphinx_resolve_self``; the flags ``smartquotes``,
   ``trim_footnote_reference_space``, ``todo_include_todos``, ``show_authors``,
   ``numfig``, ``math_number_all``, ``add_function_parentheses``, ``add_module_names``,
   ``toc_object_entries``, ``option_emphasise_placeholders``, ``trim_doctest_flags``,
   and whether ``sphinx.ext.intersphinx`` is on; ``source_suffix``, ``include_roots``;
   every declared object type (directive, role, index, display, first-word, program,
   target) and directive alias; every ``extlinks`` entry; the ``myst_*`` settings
   (extensions, heading anchors, words per minute, the dmath and linkify flags,
   footnote transition, CommonMark-only, the deprecated dmath and amsmath flags,
   substitutions, HTML meta, substitution delimiters, URL schemes, disabled syntax);
   autodoc (whether it is on, source paths, ``autoclass_content``, class signature,
   member order, the typehint settings, default options, and the docstring,
   inheritance, defaults, literal-type, backslash and type-comment flags); the odin
   API settings (folders, root, options, member order, generate and toctree flags,
   collections); the rust API settings (crates, root, options, member order, generate
   and toctree flags).

Build settings (``gd_env::build_settings``), saved as ``Environment::settings``
   The reading settings; the translation digest; whether the build is untrusted; the
   engine limits (depth, nodes, stack and the rest, from ``--max-depth``,
   ``--max-nodes``, ``--stack-kib``); the plugin digest. An environment saved with other
   build settings, of another ``ENVIRONMENT_VERSION``, or under ``-E`` is not used:
   every document is read.

Translation digest (gd-intl)
   Whether documents are translated; ``language``; the locale folders; the additional
   gettext targets; the translation progress classes; whether fuzzy translations are
   allowed; ``figure_language_filename``; the compact mode; every translated interface
   label; and, per text domain, which catalog files exist (their paths, not their
   contents: a document depends on its own catalog through its dependencies).

Plugin digest (``gd_env::PluginRun::digest``)
   Per selected plugin, in order: its ID; its identity (an installed program's
   registered SHA-256, or for a linked plugin the digest of its ID, version, interface
   version and the executable stamp); its session's cache key; the language and the
   builder; its configuration file's path and digest; each dependency it declared, path
   and digest (a file that cannot be read stops the build); and for HTML builders each
   asset it generated, path and bytes.

Inventory digest (``gd_resolve::Intersphinx::digest``)
   Per loaded inventory: its name, target URI, base URL and the digest of its bytes.

Output settings (``gd_env::output_settings``)
   The project settings; the builder's name; the ``-t`` tags, sorted; ``-n``; whether
   the build is untrusted; the engine limits; the executable stamp; the plugin digest.

The reading layer
-----------------

``read_documents`` (gd-read) keeps a document's ``DocInfo`` from the previous
environment, without parsing it, when all of these hold:

* the environment was saved with the same build settings (see above);
* the digest of the source text read now equals ``DocInfo::digest`` (a generated page:
  the digest of its generated text);
* the previous read did not fail;
* every dependency's file digest is unchanged (``dependencies_changed``); a file that
  cannot be read counts as changed;
* its object file is still in the doctrees folder.

A document's dependencies are every file it was made from besides its source: included
files (``included = true``, so a document included by another is part of that page),
files it quotes (``literalinclude``, ``csv-table``, graphs), the Python, Odin and Rust
sources its autodoc, odindoc and rustdoc directives read, and the translation catalog
it was translated with. A generated API page's dependencies are the files its text was
made from (``Generated::deps``; for a Rust page, ``gd_rustdoc::Crate::inputs``: the module
files, ``Cargo.toml`` and the files ``include_str!`` docs name). A document whose package or object was not found is
read again every build.

Within one build each dependency is hashed once: the reading threads share a memo of
file digests, seeded with the digests the generated pages were just made with.

The Rust API pages are not made again when nothing they come from changed.
``<doctrees>/rust-api.json`` (gd-read, ``rustcache.rs``) keeps each crate's pages and
reports. They are keyed by the executable stamp, the crate's path and the options, with the
digest of the listing of the files below the crate's folder (``target`` and hidden
folders left out) and the digest of every input. A crate whose listing and inputs are
unchanged keeps its pages; any other is parsed again. The record is a memo, checked
before use: a missing or damaged one only costs the parse.

A reused object is checked when it is loaded: its bytes must have the digest
``CachedFile::digest`` records. A missing, damaged or foreign object is *stale*: the
document is read again from its source, with a ``build.object.stale`` warning, and its
new object is used. Objects and search words are content-addressed
(``objects/<docname>.<digest16>.gdo``, ``search/<docname>.<digest16>.txt``). Once
written, they never change. Those the published environment no longer names are
swept after the commit.

The writing layer
-----------------

Every output is a set of units, each with a key: a document's page (its docname), the
search index (``search``), a book (``book:<file>``), the files every page shares
(``SITE_UNIT``, the empty key). The manifest records per unit the digest of its inputs,
the files it made, the assets it needs, its lookups with the digest of each answer, and
its reports. ``Generation::reuse(key, inputs, answer)`` keeps a unit when, under no
``-a``:

* the manifest has the unit with the same inputs digest;
* every recorded lookup, answered again now (``gd_resolve::lookup_answer``), has the
  digest recorded with it;
* every file it made is still in the output folder.

A kept unit reports its stored diagnostics again, so a build's result never depends on
what an earlier build left behind. A unit written again whose bytes match the file in
place only records the file, so unchanged files keep their times.

Unit inputs
~~~~~~~~~~~

Document settings (text, dummy, and the base of the HTML ones)
   The output settings; the translation digest; the inventory digest; every document's
   docname and title (a page may test whether any page exists); and in toctree order,
   per document: docname, anchor key, parent, number of toctrees, and per section its
   title, anchor and number. A change of navigation (a title, a section, a toctree)
   therefore rewrites every page, which shows it in its sidebar and links.

HTML settings (html, dirhtml)
   The document settings; the digest of ``mathjax_config_path``; the built-in layout
   template; every template file used, path and digest.

A document's page
   The HTML settings (or the document settings for text and dummy); the document's
   source digest and object digest; its figure numbers; each dependency's digest; and,
   for a page with a ``todolist``, the source digest of every document that has todos.

The search index (``search``)
   The output settings and the language; per document that was read: docname, title,
   URL, search-words digest, and per object its full name, type and anchor.

A book (``book:<file>``)
   The output settings; whether there is one book; the Typst options; the backend's
   version and whether it is embedded; the composed Typst source; and for every file
   the book reads (sorted) and the Typst program: path, size and modification time, or
   that it is missing.

The site unit
   Written every build; its files are compared byte for byte, so only changed ones are
   staged.

Lookups and their answers
~~~~~~~~~~~~~~~~~~~~~~~~~

A page notes each lookup once (``gd_env::Trace``); the answer's digest covers:

``Label``, ``Term``, ``Object``, ``Suffix``
   The target's docname, anchor, title, label kind, object type, item number, whether
   the object is an alias, the section number a section label shows, and the document's
   anchor key; or that there is none. (The section number is part of the answer because a page that showed "Section 2.3" must be rewritten when the section is renumbered.) A suffix lookup
   (``prefix\0suffix\0role``) answers with the object that wins now: an object before
   an alias, then the first by name.
``Doc``
   The document's title, or that there is none.
``File``
   Whether the file exists.
``Image``
   The file the image resolves to for this page (language variants included), its
   target in the output, and whether it exists.
``Modules``
   Whether the project has a module index.
``Cpp``
   The digest of every C++ declaration.

Publication
-----------

The environment and the manifest are published with the output they describe, in one
commit (:doc:`publication`). A build that fails, including one that ``-W`` fails,
publishes nothing. The previous output, manifest and environment stay. So the next build
reads and writes again what this one did, and reports the same diagnostics.

Measurements
------------

The manual (85 documents, 16 documented crates) with ``guidedog build -b html``, release
build, median of several runs on a 16-thread Linux machine, before and after the
changes above (the Rust page record, the shared digest memo, the crates documented in
parallel) and the parallel stale-object check:

=====================  =======  ======
Build                  Before   After
=====================  =======  ======
Cold (no output)       1.00 s   0.52 s
No-op (nothing read)   0.75 s   0.13 s
One document changed   0.78 s   0.17 s
=====================  =======  ======

Before, 82% of a no-op build's instructions (callgrind) went to parsing every crate with
tree-sitter for the generated Rust pages. Another 6% went to hashing every crate file again for
each API page that depends on it. What remains of a no-op build is mostly
the check of every reused object (hashing 7 MB, on the reading threads), loading and
saving the environment's JSON, and the site unit (inventory and index).

Possible later work: ``guidedog serve`` keeping the environment, tables and object store
in memory between rebuilds; a (size, modification time) prefilter before hashing a
source.
