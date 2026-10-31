GDS status review, 31 October 2026
==================================

We reviewed all fourteen records at the maintainer's request before preparing beta.8.
We distinguish implementation of a record's current supported contract from broader deferred work and from release qualification.
The lifecycle commands record promotion and transitions; Git commits alone do not establish a GDS state.

.. list-table:: Reviewed states and evidence
   :header-rows: 1
   :widths: 10 15 75

   * - GDS
     - State
     - Evidence and remaining scope
   * - 0001
     - committed
     - ``gd-discussion`` and ``tests/integration/tests/discussions`` implement numbering, validation, lifecycle transitions and recoverable transactions.
   * - 0002
     - published
     - The CLI and project implementation are present, including network preview. The hostile-native-Typst worker and native Typst chapter metadata contract remain unimplemented, so the broader record cannot be committed.
   * - 0003
     - committed
     - ``guidedoc`` and the reader and renderer crates implement the supported engine contract. Corpus, allocation, source-map and round-trip tests retain explicitly documented format limitations.
   * - 0004
     - committed
     - ``gd-host`` budgets, ``gd-conf`` confinement and project budget/trust tests implement the stated resource policy. Compiler process isolation remains outside this contract.
   * - 0005
     - committed
     - ``gd-odindoc`` and the Odin reader, domain and project integration fixtures implement the Odin documentation contract.
   * - 0006
     - committed
     - ``gd-project``, ``gd-outputs``, ``gd-site`` and ``gd-book`` implement the supported project, incremental-build and recoverable-publication behavior.
   * - 0007
     - committed
     - ``gd-discussion`` and the existing discussion review evidence establish reusable catalogs, validated workflow configuration and transactional lifecycle operations.
   * - 0008
     - committed
     - ``gd-pages`` and the plugin acceptance evidence establish publication, credentials, journals and recovery. No unrelated new publishing scope is inferred.
   * - 0009
     - committed
     - ``gd-plugins``, registry tests and the published compatibility evidence establish the native plugin API and registry contract.
   * - 0010
     - committed
     - Typed build-plugin implementations and integration tests cover transforms, assets, hooks and preservation of published output on plugin failure.
   * - 0011
     - committed
     - The built-in extension implementation and ``builtin-extensions-20261015`` evidence cover all nine extensions named by this record.
   * - 0012
     - committed
     - Published beta releases and unpacked-archive smoke tests establish one CLI with linked plugins, static Graphviz on Unix and bundled Graphviz on Windows.
   * - 0013
     - committed
     - The complete static publishing contract shipped in beta.6; beta.7 added qualified rich excerpts. The previous discussion state was stale. SSG behavioral, navigation, theme, mounted-site and 1,000-article tests, plus published release evidence, justify the transition.
   * - 0014
     - committed
     - The diagram implementation, RST/MyST fixtures, SVG/PNG export, confined file dependencies, offline package tests, real PDF builds and empty-PATH release smoke justify promotion and commitment.

We promoted the diagram draft to 0014, then moved 0013 and 0014 through accepted to committed with explicit implementation evidence.
We retained the eleven other committed states and kept 0002 published.
We corrected its obsolete packaging description to match the already committed GDS 0012.

Validation
----------

The Linux x86-64 workspace build and all 1,982 tests passed, with five intentionally ignored tests.
Clippy with warnings denied, formatting, all nine generated tables and the source checker passed; the checker reports six existing soft-width warnings and no limit violations.
The diagram compiler also passes without its embedded feature.
The full manual builds to an English PDF, and all six localized HTML editions build with warnings treated as errors.
The release smoke test exercises both diagram image formats with an empty PATH.
Platform CI and unpacked release-archive qualification are still required for the beta.8 release commit.

References
----------

* :doc:`../records/0001-gds-process`
* :doc:`../records/0002-guidedog-cli`
* :doc:`../records/0013-static-site-generation`
* :doc:`../records/0014-typst-diagrams`
* ``docs/manual/evidence/ssg-review.rst``
* ``docs/manual/evidence/release-1.0.0-beta.6.rst``
* ``docs/manual/evidence/release-1.0.0-beta.7.rst``
