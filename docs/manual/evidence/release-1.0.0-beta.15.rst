Release 1.0.0-beta.15
=====================

On 2026-11-23 we published `v1.0.0-beta.15 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.15>`_ as a normal GitHub release.
The annotated tag names commit ``827e39ce199c8598001a67571eeb9ba83eee6d97``.
We prepared, pushed and tagged the release on the same day.

Purpose and scope
-----------------

This release fixes a race in the file locks.
A command could be told that the plugin registry, a publisher's state, a discussion catalog or a site folder was busy when no command held its lock.
On Unix a lock belongs to the open file, and a child process holds a copy of every descriptor from its creation until it starts its program.
A holder that only closed its lock file had not released the lock while such a copy lived.
A holder now unlocks before it closes, which releases the lock for every copy at once.

One lock, ``gd_plugins::files::lock_exclusive``, now serves the registry, ``gd-pages``, ``gd-discussion``, both locks of ``gd-ssg`` and the example publisher.
Problem codes, messages, exit statuses and lock file names are unchanged.
GDS 0019 states the model and the theorems, and ``docs/gds/proofs`` holds their Lean proofs.
:doc:`../internals/plugins` shows how a publisher uses the lock.

Qualification
-------------

We ran the local gate on Linux x86-64 on commit ``a5c400a``, the release as we prepared it.
The tagged commit differs from ``a5c400a`` by one change, in one test of ``crates/gd-plugins/src/files/lock/tests.rs``, described below; we did not run the local gate again on the tagged commit.
Formatting, source limits, generated tables, the pinned Bravura files, the build and strict Clippy passed.
The workspace tests passed: 2,290 tests across 215 reports, with five ignored.
All 66 music scores passed, and so did the smoke test of the release build, which makes an HTML site and a PDF.
``cargo xtask proofs`` verified 13 theorems with Lean 4.19: they build, no proof is open, and no theorem uses an axiom beyond ``propext``, ``Classical.choice`` and ``Quot.sound``.
The whole website built locally; its link check covered 1,022 HTML pages and 219,077 internal links, with no missing files or anchors.
No manual or website catalog had a fuzzy or untranslated message.

The test binary of ``crates/gd-plugins/tests/publishers.rs`` is the one whose registry test failed before this release.
We ran it 320 times on ``a5c400a`` without a failure, after 400 runs without a failure on the same code before the last commits were ordered.

The `Verify run <https://github.com/insanai/guidedog/actions/runs/45264316973>`_ of ``a5c400a`` failed on macOS Apple silicon.
The failing test was the new ``files::lock::tests::closing_alone_leaves_the_lock_with_a_copy``.
That test shows the defect: it closes a lock file without unlocking while a copy of the descriptor lives.
Its last assertion expected the lock to be free at the instant the last copy closed.
Other tests of the same binary start child processes, and such a child held one more copy for a moment, so the test met the very leak it demonstrates.
The library was not at fault and did not change.
Commit ``827e39c`` makes the test wait for the release instead of expecting it at once.

The `Verify run <https://github.com/insanai/guidedog/actions/runs/45266645806>`_ of ``827e39c``, the tagged commit, passed on Linux x86-64 and macOS Apple silicon.
Every job of the tagged `release run <https://github.com/insanai/guidedog/actions/runs/45269255083>`_ passed on its first attempt, and no job was cancelled.
The packages cover Linux x86-64 and ARM64, macOS Intel and Apple silicon, and Windows x86-64.

Published archives
------------------

The release carries ten assets.
``SHA256SUMS`` lists these checksums:

.. code-block:: text

   c8d2eb5e7a5b7ad9c3aee7678036296fe28f0c82c4ee52dcde98ce9ceb143c1b  guidedog-linux-amd64.tar.gz
   f726a4005dd9e62ff443070753f26ae65fa258eb708afd3a9f86aa61725faba8  guidedog-linux-arm64.tar.gz
   729a467939c2f3fb41f1f388cfef7354bc6f113a1ef3b10e6ab27f0a49d97444  guidedog-macos-amd64.tar.gz
   ead82ce0a302633cef1e4a73999b8affc8b16ba5051277e2efbb470aebe15a0c  guidedog-macos-arm64.tar.gz
   bc608ceec1d192fdc5dec310bbceecb9019a7aa281237ed3f854cbd80928fc94  guidedog-windows-amd64.zip
   8ecfadd54c2633f710f47945616741e652b61fe0b4f9da9505d3e09a3fd48771  guidedog-1.0.0-beta.15-source.tar.gz
   e5924ed58cbd1392ffb0c042a001e862cce30f3c64b9b6073af321c43317df15  guidedog.rb

The shared workflow in :doc:`../howto/github-pages` pins beta.15 and the published Linux x86-64 checksum for every manual edition.

Not yet checked
---------------

At the time of this record nobody has checked the website after the publication of beta.15 or of beta.14.
The official tap's Update workflow has not been run or checked for either release.
This record does not cover a download of the published assets, a comparison of their digests, or their signed provenance.
