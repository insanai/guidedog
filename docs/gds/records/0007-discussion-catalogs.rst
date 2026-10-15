:gds-number: 0007
:gds-title: Discussion catalogs for any project
:gds-state: committed
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-10-12
:gds-updated: 2026-10-15
:gds-discussion: https://github.com/insanai/guidedog/issues/1
:gds-labels: ["library","cli","process"]

Discussion catalogs for any project
===================================

Abstract
--------

GDS 0001 gave Guidedog a way to propose, review, and record its own decisions. This
record makes that process a library and two programs that any project can use. A
*discussion catalog* is a folder of reStructuredText records under a prefix: ``gds`` for
Guidedog, ``sds`` for Sanin, ``eds`` for Enso. ``lib/discussion`` keeps catalogs;
``dsc`` runs it and builds with the installed ``guidedog``; ``dscc`` is ``dsc`` with
Guidedog built in. ``guidedog gds`` runs the same code on ``docs/gds``. A reader of any
catalog sees its metadata as Number, Title, Authors, Updated, and Labels, in HTML and
PDF.

Motivation and scope
--------------------

A prototype in Sanin, ``discussion-cli``, copied ``lib/gds`` to keep its own catalog.
Copies drift: a fix to recovery in one does not
reach the others, and each must be qualified again. Issue 1 asks for four things. One reusable
package. One command line, with a build that needs Guidedog installed and one that does
not. Readable metadata for every prefix. Explicit failures where the prototype could
report success with an incomplete plan or an empty record.

This record covers catalog management, the two programs, the metadata presentation,
and, since 0.2.2, each catalog's configuration of its lifecycle, record types, labels,
and numbering, and a bound on the memory a catalog needs. A catalog without
configuration follows GDS 0001.

Specification
-------------

Library
~~~~~~~

``lib/discussion`` uses only Odin's core library. Every procedure takes the prefix, or
``DEFAULT_PREFIX`` (``gds``), and names everything after it: fields ``:eds-number:``, the
lock ``.eds.lock``, the journal ``.eds-journal.json``, generated markers
``.. eds-generated-start``, problem codes ``eds.date``. A GDS catalog therefore reads and
writes exactly what it did before.

A catalog has a configuration, ``discussion.toml``: the prefix, a display name,
a default author, and its process. ``init`` writes it with ``conf.toml``, ``index.rst``,
``workflow.rst``, a record template, the generated index, and a process draft.
The schema-1 ``.discussion.json`` definitions written by 0.2.1 remain readable.
A catalog without either file is named by the
number field of its template, so existing catalogs need no change.

Records keep the GDS 0001 fields. ``last-updated`` may stand for ``updated`` (never
both), and a record may carry ``category``, ``status``, and ``summary``; ``status`` follows
the state. A metadata edit replaces only its field's line, so every other byte, line
endings included, stays as it was.

Programs
~~~~~~~~

``discussion/command`` is the command line. A ``Program`` says who runs it: its name,
version, the ``Renderer`` that builds, and for ``guidedog gds`` the fixed prefix and
folder. A ``Renderer`` publishes one target of a catalog (the HTML site, or PDF books)
from a request the library prepares after checking the catalog.

.. list-table:: Programs and renderers
   :header-rows: 1
   :widths: 22 30 48

   * - Program
     - Renderer
     - Builds with
   * - ``dsc``
     - ``discussion/external``
     - the installed ``guidedog``: ``--guidedog``, ``DSC_GUIDEDOG``, or ``PATH``
   * - ``dscc``
     - ``discussion/embedded``
     - Guidedog linked in, with what a release Guidedog links (Typst, Graphviz)
   * - ``guidedog gds``
     - ``discussion/embedded``
     - Guidedog itself, on ``docs/gds``

``dsc`` never looks for Typst or Graphviz. They are Guidedog's to find, and ``dsc`` hands
Guidedog only ``build``, the catalog, ``-W``, and the books as a quoted
``pdf_documents`` setting. The release puts ``dsc`` in the Guidedog archive and ``dscc``
in its own archive per platform. Both report Guidedog's version.

Configuration
~~~~~~~~~~~~~

A catalog's ``discussion.toml`` holds its prefix, name, default author, labels, record
types, numbering, and workflow. ``init`` writes every setting with a comment, and
``dsc config`` prints the settings in effect. It replaces 0.2.1's ``.discussion.json``,
which is still read. The defaults are GDS 0001's process, so a catalog without the file,
such as a Sanin catalog, behaves as before.

The workflow names the state of a new draft and the state promotion opens it in. Each
state lists the states the ``state`` command may move a record to (``next``), whether
a move into it needs evidence, and the text of a record's optional ``status`` there.
The evidence requirement also applies when promotion enters the open state.
Promotion is the only move from the draft state to the open state, because it gives the
number. Otherwise a draft may move only to states its own ``next`` lists, and a record
without a number may be only in those states. Every state change needs a reason.

Numbers have at least ``width`` digits (default 4), and grow past it: 10000 follows
9999, up to nine digits, and records sort by value, not by text. Promotion numbers from
``start``, up to ``limit`` when one is set. Both govern new numbers only, so changing
them never invalidates a record. Changing ``width`` would, and the check names the
record and the setting. A mistake in the file is reported with its line and the fix,
and unknown settings are refused rather than ignored.

The process draft ``init`` writes describes the configured lifecycle, with a table of
the states and a diagram of the moves, so it is never out of step with the catalog.

Memory
~~~~~~

``load`` reads each record a chunk at a time. It keeps only the metadata, at most
64 KiB, and which of the required sections the record has. It stops reading once it knows both.
The record a command changes is read whole, up to 16 MiB. A catalog costs about a
kilobyte a record, may hold 100,000 records, and its metadata may total 256 MiB. A
catalog of 2,000 records of 100 KB is checked with 17 MB of memory, where 0.2.1 needed
211 MB, and as fast. Duplicate numbers and slugs are found in linear time.

Metadata presentation
~~~~~~~~~~~~~~~~~~~~~

The leading field list of a page is a discussion's metadata when its first field has a
prefix of 2 to 16 lowercase letters and the list carries that prefix's ``number``,
``title``, or ``reserved-through``. Its known fields show as Number, Title, State, Type,
Authors, Created, Updated, Discussion, Labels, Reserved through, Category, Status, and
Summary; authors and labels show as plain lists. Unknown fields, fields of another
prefix, and later field lists keep their names. The source is not changed.

Rationale and alternatives
--------------------------

*Keep* ``lib/gds`` *as a compatibility layer.* Rejected: it would be a second copy of the
API to keep in step, which is the problem this record removes. ``guidedog gds`` stays
as a few lines that run the shared command.

*One program with a build flag.* Odin cannot make an import conditional, and a
program that links Guidedog cannot drop it at run time. Two small ``main`` packages over
one command package keep ``dsc`` free of the engine.

*Detect allocation failure with a guard at the command only.* Guidedog's commands do
this, and it stays. But a library others call must say for itself when its result is
incomplete, so each public procedure watches its own allocations (below).

Security and operational considerations
---------------------------------------

Commands that change records hold the catalog's kernel lock. ``apply`` checks that each
file still has the contents the plan was made from, and that each change is whole: a
record that parses and has its sections, an index with one generated region. Only then
does it write a journal of the originals and replace each file atomically.
``recover --rollback`` restores the originals and refuses to overwrite a later edit.

Each public procedure works through a *watch*: an allocator, kept in the caller's
memory, that refuses everything once memory has been refused. A procedure whose watch
refused returns ``discussion.memory`` and nothing it made. Its check is plain code after
the work: in Odin, a deferred assignment to a named result does not change what a
procedure returns. Each staging file's name is kept without allocating, so it is removed
even when memory has run out.

Backwards compatibility
-----------------------

GDS records, numbers, locks, journals, generated markers, and problem codes are
unchanged; the repository's catalog passes ``guidedog gds check`` and ``index --check``
before and after. The ``gds`` Odin package is removed: callers use ``discussion`` with the
default prefix. ``replace_field`` returns a problem beside the text, and accepts only
literal values, not Typst expressions. Catalogs made by the Sanin prototype need no
change.

Acceptance criteria and implementation status
---------------------------------------------

Implemented in Guidedog 0.2.1:

* ``lib/discussion`` and its tests: every prefix, memo fields, field edits that keep
  every other byte, init, discovery, transitions, generated regions;
* refusal sweeps: ``replace_field``, ``index_plan``, and ``new_plan`` with ``apply`` refuse
  each of their allocations in turn, and return either ``discussion.memory`` or the
  same result as with all the memory they want; an apply leaves the catalog as it was,
  as planned, or recoverable to as it was;
* interrupted applies after every file, each recovered to the original catalog;
* ``dsc`` integration tests, and release smoke tests of ``dsc`` with the packaged
  ``guidedog`` and of ``dscc`` with an empty ``PATH``;
* metadata presentation tests in HTML, text, and the PDF book's Typst source.

The sweeps found and fixed three faults before release: an unchecked allocation that
would have crashed ``new``, staging files left behind, and the deferred-assignment
pattern above.

Implemented in Guidedog 0.2.2:

* ``discussion.toml``: reading with line-numbered problems, rendering, and a round trip
  of the defaults; a configured workflow with its own types, evidence, start and limit;
  numbers past their width and their order; ``dsc config`` and ``init --config``;
* ``load`` under a measured bound: 40 records of 1 MiB load within 2 MiB;
* the refusal sweeps over configuration and loading, which found two more unchecked
  allocations before release.

Open questions
--------------

Should promotion rewrite prose links to a renamed record? Should ``dsc`` offer ``serve``?
Should a reason be optional for some moves?

Implementation in Rust
----------------------

GDS 0011 moves the implementation to Rust. ``crates/gd-discussion`` ports
``lib/discussion`` and its command line; ``docs/manual/internals/discussion.rst`` records
the design. Prefixes, fields, ``discussion.toml``, the journal format, generated regions,
commands and exit statuses are unchanged; this catalog is checked by the Rust ``dsc``.

- Programs: ``dsc`` and ``dscc`` (``cmd/dsc``, ``cmd/dscc``) and ``guidedog gds`` call
  ``gd_discussion::command::run`` with their own ``Program``. The ``Renderer`` becomes a
  ``Builder`` trait; ``GuidedogProgram`` runs the installed ``guidedog`` without a shell.
  ``dscc`` will implement ``Builder`` over ``gd-project`` in process; until then it also
  uses ``GuidedogProgram``. Building older Typst catalogs belongs to that builder.
- Types instead of conventions: ``Prefix`` is a validated newtype; ``apply`` and
  ``recover`` require the ``Lock`` (released on drop), so an unlocked write does not
  compile; one request type per operation (``NewDraft``, ``Decision``, ``BuildOptions``,
  ``Init``); ``replace_field`` takes a typed ``FieldValue``.
- Memory: the bounds above stay (64 KiB of metadata per record, 4 KiB lines, 256 MiB per
  catalog, reading stops once both required sections were seen). The allocator watch and
  ``discussion.memory`` are gone, because Rust aborts on allocation failure; the refusal
  sweeps became interruption sweeps. They stop ``apply`` after every file and check that
  ``recover`` restores the catalog byte for byte.
- Problems carry a line, an excerpt with a caret and a "Did you mean" suggestion, rendered
  by ``gd-report``.

Implementation revision, 2026-10-15: the discussion commands became a plugin of guidedog. ``guidedog dsc`` (``init``, ``new``, ``list``, ``show``, ``promote``, ``state``, ``index``, ``check``, ``build``, ``recover``, ``config``) replaces the ``dsc`` and ``dscc`` programs and ``guidedog gds``. ``gd_discussion::plugin()`` contributes the commands through the gd-plugins command interface, and ``cmd/guidedog`` links it beside the Pages plugin. A build asks the host to build the catalog as a Guidedog project, warnings as errors, with the books as ``pdf_documents``; older Typst catalogs compile with gd-typst inside gd-discussion. Guidedog's own catalog has no command of its own: its prefix, workflow, labels and numbering are in ``docs/gds/discussion.toml``, and we run ``guidedog dsc`` in ``docs/gds``. The external builder (``GuidedogProgram``, ``--guidedog``, ``DSC_GUIDEDOG``) is gone. Records, fields, journals, locks and exit statuses are unchanged.

Review history
--------------

2026-10-12: drafted with the implementation of issue 1.

2026-10-12: revised for 0.2.2: each catalog configures its lifecycle, types, labels,
and numbering in ``discussion.toml``; numbers grow past their width; ``load`` keeps
metadata only.

2026-10-12: independent review found gaps in promotion evidence, empty type lists,
metadata limits, build allocation refusal, and TOML nesting. The working implementation
has regression fixes. Semantic configuration errors now retain source lines, and
initialization refuses an empty vocabulary safely. These changes are not in the
published 0.2.2 archives. This record remains in discussion pending qualification
and release of the corrections. See :doc:`../evidence/discussion-review-20261012`.

2026-10-12: the corrections shipped in 0.2.3 after candidate and tagged-release
qualification on every release platform. Independent published-archive smoke tests
also passed. The design is adopted and implemented; the open questions remain
outside this implementation's acceptance criteria.

2026-10-15: the implementation moved from Odin to Rust (GDS 0011). Added
"Implementation in Rust"; the decisions, evidence and lifecycle above are kept as
they were recorded.

References
----------

* GDS 0001, the discussion process.
* Issue 1, "Add a reusable discussion CLI with CLI-only and complete builds".
* ``lib/discussion/README.md``, the library's cheat sheet.
* :doc:`../workflow`, the commands of this catalog.

.. rubric:: Lifecycle event

::

   2026-10-12: prediscussion -> discussion. Assigned a permanent number and
   opened for discussion.

.. rubric:: Lifecycle event

::

   2026-10-12: discussion -> published. The maintainer-requested review adopted
   the reusable catalog design after qualification.

.. rubric:: Lifecycle event

::

   2026-10-12: published -> committed. The implementation and review
   corrections shipped in Guidedog 0.2.3.
   docs/gds/evidence/discussion-review-20261012.rst;
   https://github.com/insanai/guidedog/actions/runs/38225096535;
   https://github.com/insanai/guidedog/actions/runs/38234710527; published dsc
   and dscc HTML/PDF smoke tests.

.. rubric:: Implementation revision, 2026-10-15

The implementation moved from Odin to Rust; see GDS 0011 and the section
"Implementation in Rust". This is an implementation revision. It does not change
this record's review state or assert new acceptance evidence.
