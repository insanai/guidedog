Release 1.0.0-beta.16
=====================

On 2026-11-27 we published `v1.0.0-beta.16 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.16>`_ as a normal GitHub release.
The annotated tag names commit ``335ab226b5c4f429cd19a6e93416963df240caa4``.
We prepared, pushed and tagged the release on the same day.

Purpose and scope
-----------------

This release builds the editions of a site in parallel, within the memory of the machine.
``guidedog ssg build`` runs each edition in its own ``guidedog build`` child process.
A new edition starts only when its weight fits beside the weights of the running editions, within 90% of the memory available when the build began, and the memory available now still has room for it.
An edition weighs its last measured peak plus 25%, or 1 GiB for HTML and 3 GiB for PDF before its first measurement.
An edition heavier than the capacity runs alone.
No edition starts after an edition before it in plan order has failed, so a parallel build reports the same first failure as a sequential one and publishes the same site.
``--jobs N`` limits the number of editions at once; ``--jobs 1`` builds them one at a time, as before.

Linux reads the available memory from ``/proc/meminfo`` and the cgroup v2 limits, macOS from ``vm_stat``, and Windows from ``GlobalMemoryStatusEx``.
A child reports its peak through the new ``build --peak-memory-file`` option.
A build now also locks the records folder of its output until its commit has ended, so two builds of the same output cannot interleave; the second fails with ``build.busy``.

GDS 0020 states the model, the invariant and the theorems, and ``docs/gds/proofs`` holds their Lean proofs.
:doc:`../reference/cli` and :doc:`../howto/static-publishing` describe the option and the behaviour.

Qualification
-------------

We ran the local gate on Linux x86-64 (32 processors, 128 GB) on ``335ab22``, the tagged commit.
Formatting, source limits, the build and strict Clippy passed.
The workspace tests passed in 216 reports.
The scheduler's tests enumerate every interleaving of plans of up to four editions, more than 100,000 states, and check the invariant and the theorems in each.
``cargo xtask proofs`` verified 30 theorems with Lean 4.19: they build, no proof is open, and no theorem uses an axiom beyond ``propext``, ``Classical.choice`` and ``Quot.sound``.
The whole website, with its seven languages, built locally with warnings as errors in 11.5 seconds.

We checked the macOS and Windows code by compilation and Clippy for ``aarch64-apple-darwin``, ``x86_64-pc-windows-msvc`` and ``x86_64-pc-windows-gnu``, but could not run it locally.
The `Verify run <https://github.com/insanai/guidedog/actions/runs/45966551375>`_ of ``335ab22`` passed on Linux x86-64 and macOS Apple silicon.
Verify has no Windows job, so before tagging we ran a `Windows candidate <https://github.com/insanai/guidedog/actions/runs/45968944933>`_ of the Release workflow on ``main``; its build and smoke test passed, and nothing was published.

Every job of the tagged `release run <https://github.com/insanai/guidedog/actions/runs/45986452706>`_ passed on its first attempt, and no job was cancelled.
The packages cover Linux x86-64 and ARM64, macOS Intel and Apple silicon, and Windows x86-64.
The smoke test of each package builds a site of three editions, and each built them in parallel with the memory it read from its own system:

.. list-table::
   :header-rows: 1

   * - Package
     - Editions at once
     - Capacity
   * - Linux x86-64
     - 3
     - 13.1 GiB
   * - Linux ARM64
     - 3
     - 12.6 GiB
   * - macOS Intel
     - 3
     - 8.4 GiB
   * - macOS Apple silicon
     - 2
     - 2.8 GiB
   * - Windows x86-64
     - 3
     - 12.1 GiB

On macOS Apple silicon the memory available admitted only two editions at once, and the third waited for one of them to end.

Published archives
------------------

The release carries ten assets.
``SHA256SUMS`` lists these checksums:

.. code-block:: text

   c41685e1aa74886180fc681d81f2ac890a2ca103d97dab43ee1577f803efc798  guidedog-linux-amd64.tar.gz
   33bf708660f5119245879bd6120f0fd05b0d23a0799f7475512a58c27fed64b2  guidedog-linux-arm64.tar.gz
   9ef44a80f2c95ad39d046c7e857d51bb04ab3f3c1854bfc6d96a5bada061ba13  guidedog-macos-amd64.tar.gz
   ab0cf7a1107738588feec2d96fd8d0d656da36d4187a18f0f7701f79a67fa9ca  guidedog-macos-arm64.tar.gz
   55a81a6e331e4ca3ea2ae50c0575b4d46164f4c68f774d815f894c2a2555bdf9  guidedog-windows-amd64.zip
   87cdb95a79ae054ba5c9d18554053b87015ed4471bf0bbab7473a8a2e841e370  guidedog-1.0.0-beta.16-source.tar.gz
   5c457b98f511437a1d5d7083cd2d7252c92671a32c02e9be0d5a7315eb473fba  guidedog.rb

We downloaded ``SHA256SUMS`` and the Linux x86-64 archive.
The archive matched its line in ``SHA256SUMS`` and the digest GitHub reports for the asset, and its ``guidedog --version`` reported ``1.0.0-beta.16``.

The shared workflow in :doc:`../howto/github-pages` pins beta.16 and the published Linux x86-64 checksum for every manual edition.

Homebrew tap
------------

We ran the official tap's `Update workflow <https://github.com/insanai/homebrew-guidedog/actions/runs/45990794260>`_ with the tag ``v1.0.0-beta.16``.
It verified the formula, installed and tested it on Apple silicon and Intel macOS, and committed ``b7b38bb``.
The tap's formula now names version ``1.0.0-beta.16`` and the four beta.16 archive URLs.

Not yet checked
---------------

This record does not cover a download of the other assets, or their signed provenance.
