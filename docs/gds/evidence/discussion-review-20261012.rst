Discussion catalog review, 12 October 2026
============================================

Scope and decision
--------------------

The review covers the discussion library, its command layer, both renderers, the
shared TOML reader, and GDS 0007. The reusable library and renderer boundary are sound.
The catalog's process governs metadata, transitions, numbering, and validation before
publication. The implementation does not execute Python.

The published 0.2.2 release passed its qualification run. Fresh review still found
faults outside those tests. The corrections shipped in Guidedog 0.2.3 after full
candidate and tagged-release qualification. GDS 0007 is now an implemented design.

Findings and corrections
--------------------------

* Promotion bypassed an open state's evidence requirement. State changes and promotion
  now share the decision validator. Promotion accepts evidence on the command line,
  and the generated process diagram shows the requirement.
* An explicit ``types = []`` was treated as an absent setting. Reading now distinguishes
  presence from contents. An empty list is invalid; omission still chooses the defaults.
* Semantic configuration errors lost their source locations. Validation now receives
  the parsed source tree and selects the original setting, including workflow tables.
* A line read in one chunk could exceed the metadata line limit. Streaming reads and
  whole-record edits now share their preamble boundary and limits. A command cannot
  write oversized metadata that the next catalog load would refuse.
* Initialization could index an empty, non-nil label list. It now reports the missing
  vocabulary before selecting a label.
* Build allocation refusal could reach a renderer with incomplete arguments or return
  a shortened success result. Allocation watches now gate the renderer call as well
  as the returned result. A refusal sweep checks both the request and the result.
* Deeply nested TOML exhausted the stack. The shared value-reading boundary now refuses
  nesting beyond 64 levels, before another recursive frame is entered.
* The standalone TOML reader could accept a shortened string after its builder refused
  memory. Every decoded string write is checked, and the first failure remains a parse
  error even if later allocations succeed.
* Bare TOML keys and literal values borrowed the source despite the tree's ownership
  contract. The parser now copies its source into the caller's arena once. Releasing or
  changing the input buffer cannot invalidate the returned tree.
* The highlighter's fixed wall-clock cutoff measured shared-runner scheduling as well
  as scanning. An initial replacement using thread CPU time also varied on a macOS
  candidate runner. The final check counts dispatches and shared cursor/search work
  for 16 KiB and 64 KiB inputs, on every platform. A regression case demonstrates that
  repeated whole-line searches exceed the growth bound. The counters exist only in
  tests and are local to each test thread. Coverage and determinism remain checked.
  This probes the shared scanner paths; it is not a proof of every lexer's cost.

Verification
--------------

Heavy checks ran in an isolated checkout on ``insan@10.175.52.18``. The checkout uses
the pinned toolchain and the existing static native dependencies. Baseline
regression tests failed before the fixes. The deep-nesting baseline ended with status
139; the fixed parser returns a line-numbered diagnostic instead.

The affected library suites pass with leak checks and under AddressSanitizer.
All library and tool suites pass. All 171 integration tests pass. The source checker
reports zero violations over 696 files. The final candidate and tagged release both
passed every job: full Linux and macOS suites and integration tests under
AddressSanitizer; embedded-Typst builds; Linux amd64 and ARM64, macOS Apple silicon and
Intel, and Windows amd64 packages; fresh-platform HTML/PDF smoke tests; and Homebrew.
The Intel macOS archive ran on a real Intel Mac.

The published Apple silicon 0.2.3 archives were also downloaded independently, checked
against the release manifest, and run locally. Both ``dsc`` and ``dscc`` passed their
catalog lifecycle and HTML/PDF smoke tests with an empty ``PATH``.

The published Linux 0.2.2 archives were downloaded independently. Their SHA-256 sums
match both the release manifest and GitHub's asset digests. ``dsc`` with the packaged
Guidedog, and ``dscc`` alone, each passed the release smoke test with an empty ``PATH``:
initialization, drafting, promotion, a lifecycle decision, HTML, and PDF.

Memory measurement
--------------------

A temporary generator made 2,000 records of about 100 KB, plus the process draft.
The complete catalog contains about 200 MB of source. GNU ``time -v`` measured the
published Linux amd64 0.2.2 ``dsc check`` at 8,624 KiB peak RSS and 0.09 seconds elapsed.
The development binary used 8,640 KiB and 0.16 seconds. Both checked 2,001 records with
exit status zero. These checks followed index regeneration and used warm filesystem
caches. This supports the metadata-only design; memory still grows with the
number and size of metadata entries, not with the complete prose.

Operational details
---------------------

Do not redirect ``dsc config`` directly into ``discussion.toml``. A shell opens and
truncates the output before the command can read its input configuration. Export to
a temporary file, then replace the configuration after successful generation:

.. code-block:: sh

   dsc config > discussion.toml.new && mv discussion.toml.new discussion.toml

The 100,000-record bound is a supported limit, not a claim that this review exercised
100,000 records. Native libraries retain their own allocation boundaries. This review
does not claim complete Sphinx compatibility or a sandbox for hostile native inputs.

References
------------

* :doc:`../records/0007-discussion-catalogs`
* `Guidedog 0.2.2 <https://github.com/insanai/guidedog/releases/tag/v0.2.2>`_
* `0.2.2 release qualification <https://github.com/insanai/guidedog/actions/runs/38215998259>`_
* `0.2.3 candidate qualification <https://github.com/insanai/guidedog/actions/runs/38225096535>`_
* `0.2.3 tagged release <https://github.com/insanai/guidedog/actions/runs/38234710527>`_
* `Guidedog 0.2.3 <https://github.com/insanai/guidedog/releases/tag/v0.2.3>`_
