Plugin design acceptance, 14 October 2026
=========================================

Decision and scope
-------------------

The maintainer requested a final implementation review and asked that completed
records become committed designs. GDS 0008, 0009 and 0010 meet their shipped scope.
Their acceptance covers the implemented publishing service, explicit registration
and typed source, HTML and asset hooks. It does not freeze every beta source API
or accept features reserved for a later proposal.

GDS 0008 provides local GitHub Pages publication and a separate Actions route.
A repository binding limits the selected destination. Native credential stores
hold imported tokens. Git receive-pack checks the expected parent. Journals and
explicit Accepted, Live and Unknown outcomes prevent an uncertain push from being
reported as a verified deployment. Device login accepts an explicit registered
client ID; a maintainer-operated app is a deployment decision, not shipped authority.

GDS 0009 provides an ordinary typed descriptor, explicit owner lifetimes, frozen
snapshots, bounded process requests, pinned machine registrations and discovery
metadata. The registry is not an installer. Third-party admission, automatic
installation and a stable dynamic-library ABI remain outside the accepted scope.

GDS 0010 adds private build sessions and typed source, HTML and asset services.
Installed plugins run in child processes. Registration checks interoperability;
the host still validates every runtime response. Child RSS is not charged to the
host budget, and the process is not an operating-system sandbox. Directly linked
callbacks remain trusted native code. Mutable doctree plugins, origin maps,
installed directives and custom builders still need separate designs.

Implementation evidence
-------------------------

The published `Guidedog 0.6.2 release
<https://github.com/insanai/guidedog/releases/tag/v0.6.2>`_ names commit
``b5f9b060df48604083a6af42c5f6d1470e24f92d``. Its
`tagged release qualification
<https://github.com/insanai/guidedog/actions/runs/38434657070>`_ completed successfully.
The GitHub API was checked again on 14 October 2026 before recording this decision.

The detailed repository report is
``docs/manual/evidence/plugin-review-20261014.md``. It records the 176 integration
tests, 36 library/tool suites, allocation-refusal and AddressSanitizer checks,
five packaged platforms, native credential-store checks and twelve large-manual
builds. It links the candidate, release, Homebrew and publication runs. These are
observed qualification results, not a proof against every possible native fault.

The relevant implementation lives in ``crates/gd-plugins``, ``crates/gd-env``,
``crates/gd-pages``, ``crates/gd-git`` and the command entry points. The worked build
plugin is ``crates/gd-plugins/examples/build_plugin``; the publishing example is
``crates/gd-plugins/examples/publisher.rs``. The manual distinguishes the services and their
ownership, diagnostics and cache contracts.

Record 0002 remains published
------------------------------

GDS 0002 still describes obligations beyond the shipped project model: hard
isolation for hostile native Typst and native Typst chapter modules. Neither
follows from a successful plugin release. GDS 0006 supersedes its project
surface; the current source-language ledgers and Windows release checks are
complete, but these remaining obligations prevent marking 0002 committed.

Documentation review
----------------------

The renewed visual review found that several diagrams confused allocation and
process boundaries, placed asset collection after HTML hooks, or implied an
atomic directory exchange on every filesystem. The corrected manual describes
setup-time asset collection, native allocations within the host, installed child
processes and the recoverable two-rename fallback. It also distinguishes semantic
reference checks from HTTP link probes and documents real language-suffixed PDFs.

The new theme tutorial starts with ``guidedog quickstart``. Its downloadable
files are the files exercised by the integration test. HTML block inheritance,
partials, macros, relative links and incremental template edits are checked
alongside directory HTML, single-page HTML and a modular Typst book wrapper.
Adaptive Graphviz styling is an opt-in addition after 0.6.2, independent of the
accepted plugin contracts. Its light DOT attributes remain the PDF master.
