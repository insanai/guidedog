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
stops growing them once they fit the largest page. The readers and renderers keep their working storage in the same workspace (see below), so a repeated conversion allocates nothing at all for most reader and renderer pairs. Each rendering worker owns its own workspace; there is no shared mutable engine state and no global pool.

What a conversion returns borrows the workspace: ``Document<'ws>`` and the output
``&str`` cannot outlive the next conversion, and a stale or foreign handle is a compile
error rather than a run-time check. A value that must outlive the conversion is copied
into an owned ``String`` or ``Box<str>`` by the code that keeps it. Shared immutable data
(an included file read once and used by several documents) is an ``Arc<str>``.

The Odin implementation planned slabs, sized arenas before a session, grew them on a
``Capacity`` result, and paired every owner with a release call. None of that remains:
``Drop`` releases, and ``Vec`` growth replaces the plans. The names that were dropped are
listed in :doc:`engine-api-map`.

Working state between conversions
---------------------------------

Readers and renderers are stateless descriptors, but parsing and rendering need working storage: line tables, delimiter stacks, inline item lists, anchor tables, table layouts. We keep that storage in the workspace between conversions instead of building it afresh each time. The workspace holds a ``guidedoc::Scratch`` store, a pool of values per Rust type, and the contexts lend it out:

.. code-block:: rust
   :class: sketch

   #[derive(Default)]
   struct TextWork {
       out: String,
       frames: Vec<Frame>,
   }

   fn render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
       let mut work: TextWork = ctx.take_scratch(); // what the last render left
       work.out.clear(); // clear, keep the capacity
       // ... render with work ...
       ctx.put_scratch(work); // the next render takes it again
       Ok(())
   }

Taking a value moves it out of the store, so it borrows nothing and the adapter keeps using its context freely. A nested parse of the same reader (a directive body, a table cell) takes another value of the same type, and every value given back is kept, so each nesting level finds its own next time. Once a type's pool exists, taking and putting back do not allocate. A value that is not given back (an early return on a failure) is simply dropped; the next conversion makes a new one.

Three small tools make the storage reusable:

- ``Scratch::take`` and ``Scratch::put`` (``ReadContext::take_scratch``, ``RenderContext::take_scratch``) keep one struct per adapter: the HTML renderer's anchor table and walk stack, the Typst renderer's marks, the reStructuredText reader's line pools and grid tables, the resolver's name tables.
- ``guidedoc::NameMap`` replaces ``HashMap<String, V>`` for names that are rebuilt every conversion (reference targets, link definitions, slugs, substitutions). It copies every key into one shared ``String``, so clearing it keeps the storage; a ``HashMap`` would free each key and allocate it again.
- ``guidedoc::recycle`` keeps a buffer of borrowed values, such as ``Vec<Line<'s>>`` whose lines borrow one conversion's source, as an empty ``Vec<Line<'static>>`` between conversions. The standard library collects an emptied vector into one of a same-sized type in place, so no allocation happens.

The anchor table follows the same idea: ``build_anchors_into`` rebuilds a table in the storage of the last one, and the table finds ids through an open-addressing index over its own id text rather than a map with ``String`` keys. Sorting uses ``sort_unstable``, which does not allocate, wherever the keys are distinct.

``tests/integration/tests/allocations.rs`` counts the allocations of a repeated conversion for every reader and renderer pair and fails if a count grows or exceeds its recorded ceiling. Most pairs allocate nothing. The few allocations that remain are deliberate:

- A pool warms up over the first conversions. Buffers lent from a pool (the text renderer's layout strings, the reStructuredText writer's inline buffers) are taken in a different order by different uses, so a buffer may still grow once in the second conversion; from the third on it fits.
- Lossy output owns its text. When the reStructuredText writer meets something reStructuredText cannot express, such as an image inside a Markdown paragraph, it reports a note whose arguments the report owns, and it builds the substitution definition's options as owned strings. These paths are rare and bounded by the document.
- Output that grows past the largest document seen so far allocates once more, as every ``Vec`` does. So do new sources: an included file read for this conversion is an ``Arc<str>`` that the conversion owns.

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
