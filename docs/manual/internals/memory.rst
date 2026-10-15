Memory and ownership
====================

Guidedog's memory design rests on Rust ownership, not on allocators. Every buffer has
one owner; borrows end before the owner changes; dropping a value releases it. On top of
that, two separate mechanisms bound the work a document can cause: ``Limits`` inside a
conversion, and a coarse budget for what the host holds.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Limits bound a conversion; the budget bounds what the host holds.
   :alt: The host budget counts sources, includes and outputs before they are read. Each conversion runs in a reused workspace bounded by Limits.

   digraph memory_architecture {
     graph [rankdir=TB, bgcolor="transparent", pad="0.22", nodesep="0.3", ranksep="0.35"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];
     budget [label=<<B>Host budget</B><BR/><FONT POINT-SIZE="11">--budget; reserve before reading</FONT>>];
     held [label=<<B>What the host holds</B><BR/><FONT POINT-SIZE="11">Sources, includes, PDF output</FONT>>];
     limits [label=<<B>Limits</B><BR/><FONT POINT-SIZE="11">Nodes, text, output, depth, stack</FONT>>];
     work [class="gd-output", shape=cylinder, fillcolor="#eef4fa",
           label=<<B>Workspace per worker</B><BR/><FONT POINT-SIZE="11">Vec and String buffers, reused</FONT>>];
     budget -> held [label="accounts for"];
     budget -> limits [label="lowers"];
     limits -> work [label="bound"];
   }

Ownership replaces handles
--------------------------

A ``Workspace`` owns the node tables, the text pool, the source map, the diagnostics,
and the output buffer of one conversion at a time. ``clear()`` empties its ``Vec`` and
``String`` buffers and keeps their capacity, so a worker that converts many documents
stops allocating once its buffers have grown to the largest page. Each rendering worker
owns its own workspace; there is no shared mutable engine state and no global pool.

What a conversion returns borrows the workspace: ``Document<'ws>`` and the output
``&str`` cannot outlive the next conversion, and a stale or foreign handle is a compile
error rather than a run-time check. A value that must outlive the conversion is copied
into an owned ``String`` or ``Box<str>`` by the code that keeps it. Shared immutable data
(an included file read once and used by several documents) is an ``Arc<str>``.

The Odin implementation planned slabs, sized arenas before a session, grew them on a
``Capacity`` result, and paired every owner with a release call. None of that remains:
``Drop`` releases, and ``Vec`` growth replaces the plans. The names that were dropped are
listed in :doc:`engine-api-map`.

Limits bound the work
---------------------

``guidedoc::Limits`` holds the bounds of one conversion: nodes, text bytes, output
bytes, reports, nesting depth, work steps, stack bytes, and includes. Input-driven growth
uses ``try_reserve``, so an oversized document produces a ``Capacity`` or ``Limit``
result that names what ran out, never a panic or an abort. ``--max-depth``,
``--max-nodes`` and ``--stack-kib`` set these bounds for a build; changing one
invalidates cached reading and output.

Recursion is bounded the same way. Workers run on threads with a known 8 MiB stack
(``gd_host::run_workers``, ``run_with_stack`` for the main command), and the stack limit
of a conversion must fit inside it, so depth and stack capacity agree by construction.

The budget is coarse
--------------------

``gd_host::Budget`` counts what the host holds that it can size before allocating:
sources, included files, and the PDF the embedded Typst compiler returns. A ``Ledger``
reserves bytes before the read and releases its charges when it is dropped, so a charge
cannot outlive the storage it pays for. Rendering units that wait for room join the
budget through a guard (``Budget::join``) and wait for each other without deadlock.
``limits_within`` lowers a conversion's ``Limits`` so they fit the budget.

A refused request reports the operation that needed the memory and suggests a
``--budget`` that fits, with the machine's free memory as advice where it is known
(``GUIDEDOG_AVAILABLE_MIB`` overrides the probe, which otherwise reads
``/proc/meminfo``). A probe that cannot learn the free memory says so; an unknown amount
is never treated as zero.

The Odin implementation metered every allocation, could spill working memory to
memory-mapped files (``--memory=disk``), and scanned cgroup limits. Rust's global
allocator aborts on exhaustion instead of returning a refusal to each caller, and the
engine's growth is already bounded by ``Limits``, so per-allocation metering and disk
spilling are not part of the Rust design.

Reports outlive workspaces
--------------------------

A report shown after a conversion ends owns its title, source excerpt, and hint: a
``gd_report::Report`` holds ``String`` values, never a borrow of a workspace. Source
spans use byte offsets internally. User-facing columns count Unicode scalars. Terminal
markers account for display width separately.

The remaining boundary
----------------------

Limits and the budget cover Guidedog's own work. Native code manages its own memory:
Graphviz (linked through ``gd-graphdog``), the tree-sitter parsers, and the embedded
Typst compiler run in the host process, outside these bounds. The ``typst`` program
backend and installed plugins run as child processes with their own address spaces.
These are different isolation boundaries.

Large PDF books can consume much more memory than the host budget. A CPython
validation of the Odin implementation measured about 14.5 GiB peak RSS including Typst.
Treat that measurement as workload evidence, not a universal upper bound, and use
operating-system limits when a hard bound is needed. See :doc:`../errors` for the
user's choices when the budget is reached.
