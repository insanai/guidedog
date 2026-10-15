Work on a discussion
====================

A GDS explains one decision. Start with the problem and the proposed result.
We keep this catalog, and other projects keep theirs, with ``guidedog dsc``, run in the catalog's folder; see GDS 0007.
State the assumptions. Give a worked example. Name the alternatives.
Keep the implementation status separate from the proposal.

Create and inspect
------------------

.. code-block:: sh

   guidedog dsc new "Cache policy" --author="Ada" --labels=build
   guidedog dsc show cache-policy
   guidedog dsc list --state=prediscussion

The draft has an ``XXXXX`` number. Its filename ends in ``.rst``.
Metadata occupies the first field list. Authors and labels are JSON arrays.
Dates are real ISO calendar dates. Keep the title and the explanation in the record.

.. code-block:: rst

   :gds-number: XXXXX
   :gds-title: Cache policy
   :gds-state: prediscussion
   :gds-type: Standards Track
   :gds-authors: ["Ada"]
   :gds-created: 2026-10-01
   :gds-updated: 2026-10-01
   :gds-discussion: Local review
   :gds-labels: ["build"]

   Cache policy
   ============

   Abstract
   --------

   State the decision and its observable result.

A record needs an Abstract and References section.
Use RST tables, Graphviz diagrams, and math directives where they clarify the reasoning.
Do not hide normative prose in a typesetting include.

Review and promote
------------------

.. code-block:: sh

   guidedog dsc check
   guidedog dsc promote cache-policy --dry-run
   guidedog dsc promote cache-policy

Promotion assigns a permanent number and opens discussion.
It does not accept the design. Numbered records are not renumbered or reused.
The CLI updates the registry and reading order in the same recoverable transaction.
Review prose links after a filename changes; promotion does not rewrite them.

.. graphviz::
   :caption: A number identifies a record; a state expresses its review result.
   :alt: Drafts move to discussion, then accepted and published, then committed with evidence.

   digraph lifecycle {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f"];
     edge [color="#216553"];
     prediscussion -> discussion [label="promote"];
     discussion -> accepted;
     accepted -> published;
     published -> committed [label="implementation evidence"];
     discussion -> published;
     accepted -> committed;
     discussion -> abandoned [style=dashed];
   }

Record a decision
-----------------

.. code-block:: sh

   guidedog dsc state 4 --to=accepted --reason="Review converged"
   guidedog dsc state 4 --to=committed --reason="Implemented" --evidence="Tests and review"

A committed state requires evidence. The source history is not evidence by itself.
Do not claim another person's approval. The maintainer owns the state decision.

Publish and recover
-------------------

.. code-block:: sh

   guidedog dsc index
   guidedog dsc build --target=both
   guidedog dsc build guidedoc-engine --target=pdf
   guidedog dsc recover --rollback

HTML builds the whole discussion site. A selector limits separate PDFs.
A selected PDF preview uses ``_build/selected/<artifact>/pdf``.
It leaves the complete catalog in ``_build/pdf`` intact.
The project outputs live in ``_build/html`` and ``_build/pdf``.
An interrupted metadata edit needs explicit rollback. Recovery refuses a later edit
that conflicts with its journal. Inspect the conflict before retrying.

Read :doc:`records/0001-gds-process` for the process contract.
Read :doc:`records/0002-guidedog-cli` for the CLI contract.
