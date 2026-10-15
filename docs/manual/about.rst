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

Guidedog 0.7 is a port of the original Odin implementation to Rust. The port keeps
the user-facing behavior: the ``guidedog`` commands and options, ``conf.toml`` keys,
directives, roles, and output. It redesigns the inside the way a Rust program is
written: ownership instead of arenas and allocation budgets, traits instead of
procedure tables, and one crate per responsibility. :doc:`internals/index` describes
the result. :doc:`internals/engine-api-map` maps the Odin engine's names to the
Rust ones.

The port adds Rust API documentation (:doc:`rust`), plugins linked into a custom
binary (:doc:`plugins`), and a guided GitHub Pages workflow
(:doc:`howto/github-pages`). Odin remains a documented language: the Odin domain and
its API pages are unchanged (:doc:`odin`).

Build evidence and qualification
--------------------------------

Release 0.6.3 of the Odin implementation passed its integration and library suites,
AddressSanitizer checks, five-platform builds and smoke tests, and twelve
large-manual builds across CPython, Django, and Flask. Diagnostic and link results
matched their established baselines. Those records are kept in the repository's
``docs/manual/evidence`` folder as history. The Rust implementation keeps the
conformance tests and expected outputs (:doc:`internals/rst-conformance`,
:doc:`internals/commonmark-conformance`). Its own large-manual measurements are
recorded as new evidence.

Qualification checks (via ``guidedog plugin check``) show that plugins work
across process boundaries. Linked plugins remain trusted code.

That evidence supports the released tool and its documented surface. It is not a claim
of universal Sphinx equivalence. See :doc:`compatibility` for the retained limits.

Documentation maintenance commands are in :doc:`howto/maintain-docs`.
