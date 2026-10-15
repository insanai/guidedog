About this manual
=================

This manual describes the implemented tool and its deliberate limits.
The separate GDS project records proposals, decisions, and review history.
A proposal's state is not changed by making its typography better.

The organization follows a practical documentation pattern:
learn with tutorials, understand with topic guides, act with practical tasks,
and check details in reference pages. Internals explain the invariants and proofs.

The prose favors small examples, named assumptions, and direct statements.
The aim is an explanation that another person can test.
The design uses restrained color, strong headings, serif reading text,
and accessible navigation. It does not imitate an author's words or layout.

This project was created with ``guidedog quickstart`` and is built by Guidedog.
RST holds the prose. Graphviz holds the diagrams. LaTeX math notation holds equations.
Typst remains only where a PDF typesetting template is required.

Implementation
--------------

Guidedog is written in Rust. Each responsibility lives in its own crate: the Guidedoc engine, one crate per reader and renderer, and supporting libraries that know nothing of the engine. The engine borrows a caller-owned workspace, so a document cannot outlive the buffers it points into. Extensions are traits, and limits are explicit values that end work with a result instead of a panic. :doc:`internals/index` describes the design.

Besides reStructuredText and Markdown projects, Guidedog documents Rust crates (:doc:`rust`), Python modules (:doc:`autodoc`) and Odin packages (:doc:`odin`) from their source. Plugins can be separate programs or linked into a custom binary (:doc:`plugins`), and a guided workflow publishes a site on GitHub Pages (:doc:`howto/github-pages`).

Build evidence and qualification
--------------------------------

The conformance tests and their expected outputs (:doc:`internals/rst-conformance`, :doc:`internals/commonmark-conformance`) run with every test suite. Large-manual measurements are recorded in the repository's ``docs/manual/evidence`` folder.

Qualification checks (via ``guidedog plugin check``) show that plugins work
across process boundaries. Linked plugins remain trusted code.

That evidence supports the released tool and its documented surface. It is not a claim
of universal Sphinx equivalence. See :doc:`compatibility` for the retained limits.

Documentation maintenance commands are in :doc:`howto/maintain-docs`.
