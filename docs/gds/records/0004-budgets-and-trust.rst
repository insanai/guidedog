:gds-number: 0004
:gds-title: Memory budgets and what a build may read
:gds-state: committed
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-09-30
:gds-updated: 2026-10-15
:gds-discussion: No external discussion URL assigned
:gds-labels: ["architecture", "build", "security"]

Memory budgets and what a build may read
========================================


Abstract
--------


A Guidedog build holds one memory budget. What the host can size before it allocates is reserved from it first, with sizes summed under overflow checks: the sources a build reads, their included files, the files it copies, and the PDF the embedded Typst compiler hands back. A conversion's own memory is bounded by ``guidedoc::Limits``, which the host lowers to fit the budget. Values a template builds are sized before they are built. When the budget is reached, a build at a terminal asks whether to raise it or stop. A build without a terminal stops with a report that names a budget that fits, and never waits. Reading threads wait for each other's memory but never all at once, so the budget cannot deadlock. Memory that native code allocates for itself (Graphviz, tree-sitter, Typst) is named as outside the budget, with the operating system's limits as its bound. A build reads only what the project shares with its documents. ``--untrusted`` narrows that to the source folder, reads no file a graph names, and runs no Typst the project wrote. Typst runs in a root that holds only the folders the build may read. This record supersedes the budget and confinement parts of GDS 0002's "CLI resource policy".

Motivation and scope
--------------------


An external review found the budget bounding less than it claimed. Storage sizes could
overflow into negative values. Page rendering, to-do lists, and source maps allocated
outside any budget, or from a budget of their own. Files were read without being counted.
It also found the trust model narrower than documented. ``--untrusted`` still honoured the
project's ``include_roots`` (``["../.."]`` included a file outside the project). A graph's
``image`` read any file and wrote its absolute path into the drawing, even under
``--untrusted``. And Typst could read anything below the nearest folder holding the
source, build, and include folders, often a home folder.

In scope: the host's budget (``gd-host``), the project build (``gd-project`` and its crates), graphdog's
file access (``gd-graphdog``), and the Typst adapter's contract (``gd-typst``). Out of
scope: time limits, and a worker process for Typst (GDS 0002, first open decision).


Specification
-------------


Checked sizes
~~~~~~~~~~~~~


Sizes are summed with overflow checks (``checked_add``, ``checked_mul``). A size that cannot be represented is treated as the budget reached: the reservation is refused with the report ``host.budget``, "needs more working memory than can be counted".


One budget, reserved before allocation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


``gd_host::Budget`` is a limit and what is in use. A build has one (1 GiB, or ``--budget=MIB``, 1 to 1,048,576). ``guidedog convert`` has its own (``DEFAULT_BUDGET``, 256 MiB, or ``--budget``). A ``Ledger`` charges what one unit of work holds before the memory is allocated, and ``settle`` releases it when the unit frees it. A ledger settles itself when it is dropped, so a charge cannot outlive its work. What a build charges:

- a document's read: its source (read again by the reading thread, so a source is held only while it is parsed; the digest is of what was parsed), and its includes (``Files::with_ledger``; a refusal is ``ResourceStatus::Limit``, reported as ``core.include.limit``);
- a static file copied into the output, freed once written;
- the PDF the embedded Typst compiler hands back (``gd_typst::Options::reserve``).

A reservation the budget refuses is never allocated. A refused ``try_reserve`` for a size the budget allowed is a ``Memory`` problem ("OUT OF MEMORY": the budget allows it, the system could not provide it).


A conversion's memory is bounded by its limits
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


A conversion's tables grow as it reads, so their size is not known before it starts. They are bounded instead by ``guidedoc::Limits`` (``max_nodes``, ``max_text_bytes``, ``max_output_bytes``, and the rest; GDS 0003). ``gd_host::limits_within`` lowers a conversion's limits so that what they allow fits the budget. Reaching a limit is a ``Limit`` or ``Capacity`` result with a report that names the setting to raise. Growth that input drives uses ``try_reserve``, so a refusal by the system is a result too.

Values a template builds are sized before they are built. ``gd-jinja`` works out the size of a string or list repetition, ``range``, ``join``, ``~``, padding filters, and ``format`` widths first. Past ``max_output`` that is a ``Limit`` error that names the template line. A size that cannot be reserved is a ``Memory`` error. So ``{{ "x" * 60000000 }}`` is refused before 60 MB is asked for.

``gd-odindoc`` bounds a file it parses by its own ``Limits::max_file_bytes`` and by its parser's nesting. ``gd-gettext`` validates a compiled catalog once at load and serves its strings from the file's own buffer.

Other bookkeeping (saved state, page layouts, the search index, inventories) is bounded by these limits and by the operating system rather than charged allocation by allocation. Metering every allocation would require every library to survive a refused allocation, and Rust's global allocator ends the process instead. Publication keeps that safe: outputs are staged and switched as whole generations (GDS 0006), so a process that ends early leaves the previous generation published.


When the budget is reached
~~~~~~~~~~~~~~~~~~~~~~~~~~


A budget's policy (``Budget::with_policy``) says what it does with a reservation it cannot hold, and that no unit will make room for. With no policy, it is refused. The policy is called one at a time, while the budget's other reservations wait, and it never reserves from the budget it decides about. It answers ``RefusalChoice::Raise(bytes)``, and the reservation is tried again, or ``RefusalChoice::Stop``, and later refusals do not ask again.

The command line sets the policy. ``--memory=ram`` stops. Without it, a build whose standard input and standard error are terminals asks. It offers a budget that fits the free memory when what is needed does (twice the old one, or a quarter more than needed, in 64 MiB steps). A build without a terminal never asks. It stops with the budget's report, whose hint names a concrete value such as ``--budget=256`` (``budget_hint``, ``suggest_budget``, ``fitting_budget``). ``guidedog serve`` keeps an answer for its later rebuilds.

Before a build starts, the budget is compared with the memory the machine has free (``available_memory``: ``/proc/meminfo`` on Linux, or the ``GUIDEDOG_AVAILABLE_MIB`` override). A budget larger than that is asked about at a terminal and reported as a warning without one. The free memory is used for hints only.


Waiting without deadlock
~~~~~~~~~~~~~~~~~~~~~~~~


Units that run at once join the budget (``Budget::join`` returns a ``Unit`` that leaves when dropped): the reading threads of ``-j``. A reservation the budget cannot hold now waits while another joined unit is still running, since that one releases what it holds when it finishes. When every other joined unit is waiting too, nothing would be released. So the reservation is refused, the unit's document fails with the budget's report, and its release lets the others go on. A unit that has not joined, or is alone, never waits. So no unit waits for itself, and waiting units never wait for each other. A budget too small for the work refuses some of it and says so.


Outside the budget, and its bound
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


Native code allocates for itself: Graphviz while it lays out (its input is bounded by the document, and its output is counted), tree-sitter while it parses, and Typst while it typesets, in Guidedog's process or in the ``typst`` program. Guidedog does not claim to bound these. A separate limit was considered and rejected. ``RLIMIT_AS`` counts reserved address space rather than memory used (runtimes reserve far more than they touch). macOS does not enforce it. It cannot separate the embedded compiler from Guidedog. The bound is therefore the operating system's, for the whole build: a cgroup (``systemd-run --user --scope -p MemoryMax=...``), a job object on Windows, or a container. The compatibility guide says so. A worker process with its own limits remains GDS 0002's first open decision.


What a build may read
~~~~~~~~~~~~~~~~~~~~~


A trusted build reads below the source folder and the folders ``conf.toml`` opts into
(``include_roots``, ``autodoc_source_paths``, ``odin_autoapi_dirs``, the crates of ``rust_autoapi_crates``), through ``gd_host::confined``,
which follows links. Two more readers go through it:

- Graphs. ``gd-graphdog``'s ``Options::files`` decides what a graph may read. After parsing and before layout, it shows the
  caller every file name a graph gives through ``image``, ``shapefile``, ``imagepath``,
  ``fontpath``, and ``<IMG SRC>`` in HTML-like labels (``label``, ``xlabel``, ``headlabel``,
  ``taillabel``), and sets the path the caller returns, or refuses the graph
  (``ErrorKind::FileRefused``). The build resolves a name against the document's folder, accepts a
  PNG, JPEG, GIF, or SVG file inside the readable folders whose path needs no escaping,
  and leaves ``imagepath`` and ``fontpath`` out. After rendering, each path in the drawing is
  replaced by the image as a ``data:`` URL, so pages and books show it without a copy and no
  path reaches the output. A drawing that would still show a path is refused. A graph
  that may name files is drawn every time rather than taken from the graph cache.
- Typst. The book's paths stay relative to ``book_root``, the nearest folder holding the
  source folder, the include roots, and the graphs folder (never the output folder,
  which Typst does not read). Typst runs in ``typst_root``: a private temporary folder of
  links, at the same places, to only those folders, removed afterwards (the links, never
  what they lead to). A path in the book, its template, or its preamble then reaches those
  folders or nothing. The same holds for the embedded compiler, whose reads are relative
  to the root it is given. If the restricted root or any link cannot be made,
  the build stops with ``typst.root`` before compiling. It never falls back to
  the common ancestor. The hint explains temporary-directory permissions and
  Windows Developer Mode, or how to keep inputs inside one source root.


--untrusted
~~~~~~~~~~~


Before discovery, ``gd_conf::confine_untrusted`` removes from the build's configuration, each with a
warning (``build.untrusted``), ``include_roots`` and every file or folder setting that leads
outside the source folder: ``html_static_path``, ``html_extra_path``, ``templates_path``,
``locale_dirs``, ``pdf_font_paths``, ``autodoc_source_paths``, ``odin_autoapi_dirs``,
``html_logo``, ``html_favicon``, ``pdf_logo``, and ``mathjax_config_path``, and leaves the crate folders of ``rust_autoapi_crates`` unread. No build plugin runs. The build then reads
only below the source folder. Ignoring with a warning was chosen over refusing the build,
so a project from elsewhere still builds, visibly without what it cannot have.

The configuration confined is the build's own copy (``build_config``), never the loaded
project's. A program (or ``guidedog serve``) that builds one project several times gets
an untrusted build's confinement for that build only. A trusted build afterwards reads
the include roots again (``a_trusted_build_after_an_untrusted_one_is_not_confined``).

A graph under ``--untrusted`` reads no file. A graph naming one is shown as code with a
warning.

The book runs only Typst that Guidedog generates. ``pdf_preamble`` and
``_templates/book.typ`` are left out with a warning. Raw Typst is already omitted by the
policy, and ``pdf_packages`` is ``offline``. The generated Typst escapes everything a
document says (text, math, strings), so a document cannot add Typst code. It names only
images and graphs that passed confinement.


The Typst adapter's contract
~~~~~~~~~~~~~~~~~~~~~~~~~~~~


``gd-typst`` states what it guarantees instead of "not an untrusted-input sandbox". Typst
reads only paths below its root (a link below the root is followed), fonts from
``font_paths`` and the system's, and packages from the network unless ``offline``. Its memory and time are its own. Typst code reaches everything below its
root, so a caller compiling Typst it does not trust gives a root holding only what that
code may read.


Rationale and alternatives
--------------------------


One budget per build, rather than one per session, is what bounds a build. A page no
longer escapes it by opening a session of its own, and ``--budget`` means the whole build.
Charging a ledger before allocating keeps the rule "reserve, then allocate" for memory
that is not a session's. Re-reading a source in its thread keeps parallel reads from
holding every pending source at once.

Bounding a conversion by its limits, rather than metering each allocation, keeps the
rule "reserve, then allocate" where a size is known first and states a bound everywhere
else. Metering every allocation would need every library behind the meter to survive a
refused allocation. A coarse budget with exact limits gives the user the same choice
(raise it, or stop) with far less machinery.

Waiting only while another unit runs replaces "wait only when holding nothing", which
reading could not keep once sources and includes are counted. A unit always holds its
source while its session grows.

Embedding a graph's images, rather than copying them next to the drawing, makes one
drawing serve pages and books alike, and keeps paths out of the output. Links for the
Typst root, rather than copies, cost nothing for large image folders.


Security and operational considerations
---------------------------------------


A build can be refused for memory it would have used before. The report names what
needed how much and the flag to raise. Under ``--untrusted``, native memory and time
remain unbounded by Guidedog. The guide tells users to add the system's limits.


Backwards compatibility
-----------------------


Graphs keep drawing. Those that named files outside the readable folders, used
``imagepath``, or named non-image files now show as code with a warning. Books whose
template or preamble read files outside the readable folders now fail with Typst's "file
not found". ``--untrusted`` builds lose the settings listed above, with warnings.
``--untrusted`` also leaves the crate folders of ``rust_autoapi_crates`` unread and runs no build plugin. A build that used to finish over its budget now stops at the refused reservation, with the budget's report.


Acceptance criteria and implementation status
---------------------------------------------


Implemented, with tests: the budget (``gd-host``: ``waiting_units_never_deadlock``, ``policies_raise_or_stop``, ``refusal_hints_name_a_budget_that_fits``, ``no_budget_is_claimed_to_fit_zero_free_memory``, ``limits_are_lowered_to_the_budget``, ``sizes_read_as_text_never_round_down_to_the_budget``); graphdog's file access (``gd-graphdog``: ``the_caller_decides_every_file_a_graph_names``, ``the_caller_can_refuse_the_output``); builds (``tests/integration/tests/projects/limits.rs``: one budget bounds reading and rendering, files are read within it, templates render within it, and builds take the conversion limits; ``tests/integration/tests/sphinx/trust.rs``: links, ``--untrusted`` roots, books, and graphs). ``book_root_failure_never_widens_readable_access`` (``gd-book``) forces a root preparation failure after a successful link and checks that no wider root is returned. The same integration suite runs against the embedded compiler and the external compiler.

Refusing the copy of an embedded compiler's PDF returns the memory problem, exit 3, with numeric budget advice. It does not misreport successful compilation as a launch failure. Runtime hints distinguish host budgets, conversion limits, and native compiler memory. ``tests/integration/tests/gates/hints.rs`` checks that every hint gives directions.

Not counted: memory native code allocates for itself (Graphviz while it lays out, tree-sitter while it parses, Typst while it typesets), bounded by the operating system as described above. Outside any build's budget, because no build exists yet: ``conf.toml``, read when a project loads, at most 1 MiB.


Open questions
--------------


- Should a build refuse, rather than ignore, ``include_roots`` under ``--untrusted``, for
  projects whose documents cannot be read without them?
- Should the graph cache key drawings that embed files by the files' digests, rather
  than draw them every time?


Review history
--------------


2026-09-30: drafted with the implementation, after the external review. Same day, after
the second review: reservations are refused before anything is allocated, values a
template builds are sized first, and a build at its budget asks, or follows ``--memory``,
whether to raise it.


References
----------


- GDS 0002, Guidedog: CLI resource policy (superseded in part by this record).
- GDS 0003, the Guidedoc engine: capacities and budgets.
- Graphviz 16.1, ``lib/common/utils.c`` (``safefile``) and ``lib/gvc/gvusershape.c``.
- Typst 0.15.1, the CLI's ``--root`` and ``typst-kit``'s file resolution.

.. rubric:: Lifecycle event

::

   2026-10-11: prediscussion -> discussion. Assigned a permanent number and opened
   for discussion.

.. rubric:: Lifecycle event

::

   2026-10-11: discussion -> accepted. Maintainer-requested implementation-status
   reconciliation; current supported contract reviewed and deferred scope stated
   explicitly.

.. rubric:: Lifecycle event

::

   2026-10-11: accepted -> committed. Current supported design implemented; beta
   review fixes and limitations are recorded. Library and integration
   suites; embedded-backend validation;
   docs/manual/evidence/beta-review-20261011.md. Deferred features are not
   claimed as implemented.
