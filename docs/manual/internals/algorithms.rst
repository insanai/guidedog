Invariants before algorithms
============================

A useful invariant says what stays true while the algorithm runs.
It also says what the caller may rely on when the algorithm stops.
Write it before writing the loop.

Preorder subtree intervals
--------------------------

Assume a tree is emitted in preorder. Every node is emitted before its children.
Assume each subtree is contiguous. Let :math:`e_i` be the first index after node
:math:`i` and all its descendants.

.. math::
   :label: subtree-interval

   \operatorname{subtree}(i) = [i,e_i), \qquad i < e_i \le N.

**Claim.** A traversal can skip the subtree by advancing to :math:`e_i`.

**Proof.** Contiguity puts every descendant before :math:`e_i`.
The exclusive-end definition puts every later node at or after :math:`e_i`.
Advancing there therefore skips exactly the subtree.
The validator checks the assumptions before a renderer relies on them.

.. graphviz::
   :class: graphviz-adaptive
   :caption: A parent's interval encloses its children's intervals.
   :alt: Document zero contains heading and paragraph subtrees, each with a text leaf. Five preorder nodes occupy nested half-open intervals.

   digraph intervals {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3", nodesep="0.4", ranksep="0.4"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.2,0.12"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10];

     d [label=<<B>0: Document Root</B><BR/><FONT POINT-SIZE="11">Interval [0, 5)</FONT>>, fillcolor="#e0eee9"];
     h [label=<<B>1: Heading Node</B><BR/><FONT POINT-SIZE="11">Interval [1, 3)</FONT>>, fillcolor="#edf5f2"];
     p [label=<<B>3: Paragraph Node</B><BR/><FONT POINT-SIZE="11">Interval [3, 5)</FONT>>, fillcolor="#edf5f2"];
     t [class="gd-input", label=<<B>4: Paragraph Text</B><BR/><FONT POINT-SIZE="11">Leaf [4, 5)</FONT>>, fillcolor="#fbfaf3"];

     ht [class="gd-input", label=<<B>2: Heading Text</B><BR/><FONT POINT-SIZE="11">Leaf [2, 3)</FONT>>, fillcolor="#fbfaf3"];
     d -> h;
     h -> ht;
     d -> p;
     p -> t;
   }

A complete scan visits each node once. Its traversal cost is :math:`O(N)`.
Skipping a subtree takes one index update. Payload work still costs what it uses.
Do not confuse a constant-time skip with constant-time rendering.

Bounded emission
----------------

A renderer runs once, into an emitter that holds an output limit :math:`c`
(``Limits::max_output_bytes``). Let :math:`m` be the number of bytes the rendering
produces. The emitter keeps counting after the limit is passed, without storing more:

.. math::

   m \le c \quad \Longrightarrow \quad \text{the output is returned}, \qquad
   m > c \quad \Longrightarrow \quad \text{a } \texttt{Capacity} \text{ failure that names } m.

The failure therefore reports the exact size that would have fitted, and a caller
that raises the limit to :math:`m` succeeds on the next run. ``check_render`` runs the
same renderer in check mode and returns :math:`m` and any support problems without
keeping output. The Odin engine measured first and wrote second. One run with a
counting emitter gives the same guarantee for half the work.

A failure returns no valid partial artifact. The builder and the emitter poison
themselves on the first failure, so every later call returns that failure and ``?``
carries it out. The host owns publication, so incomplete bytes cannot replace a
previous output.

Cache validity
--------------

A cache key must cover the data that can change a result.
Sources alone are insufficient. Includes, templates, configuration,
reference lookups, resources, plugin identities, and adapter versions can affect the
output. Reuse is allowed only when the recorded dependencies still agree.
A missing output invalidates reuse even when its key matches.

Guidedog records, for every output unit, the digests of what it read and the answers
to the lookups it made (a verifying trace with dynamic dependencies). A rebuild
recomputes each answer and stops early where the answers agree. One place, the
generation's reuse check, decides whether a unit is written again. :doc:`incremental` lists
every digest input.

Tests cover the bounds rather than counting on them. A counting allocator checks that
a conversion in steady state allocates nothing. Limit tests check that oversized
input becomes a ``Capacity`` or ``Limit`` result, never a panic or a silently shortened
document.
