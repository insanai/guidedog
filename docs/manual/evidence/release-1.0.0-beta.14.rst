Release 1.0.0-beta.14
=====================

On 2026-11-23 we published `v1.0.0-beta.14 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.14>`_ as a normal GitHub release.
The annotated tag names commit ``9551f5fb234df80c79ff6de2780f65fbeaec3354``.
We prepared, pushed and tagged the release on the same day.

Purpose and scope
-----------------

HTML pages can show formulas as MathML Core, which needs no script.
The setting ``html_math_renderer``, ``guidedog build --math=mathml`` and ``guidedog convert --math=mathml`` select it.
MathJax stays the default, and its output is unchanged.

The ``math`` directive takes ``:notation: mathml`` for a formula written in MathML itself.
HTML writes it through an allowlist filter for MathML Core.
PDF translates the same filtered MathML to Typst math within bounds on size, element count and nesting.

``guidedog convert --embed-pictures`` draws graphs, Typst diagrams and inline scores into an HTML fragment as SVG ``data:`` images.
``guidedog convert --keep-going`` writes the output of a document with errors and still reports them.
Every LaTeX formula is checked once when a document is read, whatever the output is.
:doc:`../topics/mathematics` describes the renderers, the notation and the check.

Qualification
-------------

Before the push we ran the local gate on the tagged commit, on Linux x86-64.
Formatting, source limits, generated tables, the pinned Bravura files, the build and strict Clippy passed.
The workspace tests passed: 2,278 tests across 215 reports, with five ignored.
All 66 music scores passed, and so did the smoke test of the release build, which makes an HTML site and a PDF.
The whole website built locally; its link check covered 1,000 HTML pages and 217,443 internal links, with no missing files or anchors.
No manual or website catalog had a fuzzy or untranslated message.

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/45231455976>`_ passed on Linux x86-64.
On macOS Apple silicon it failed once and passed on a rerun of the same commit.
The failing test was ``an_installed_publisher_publishes_through_the_protocol``: it met ``plugins.busy`` where it expected ``plugins.registered``.
This is the race in the file locks that GDS 0019 describes and that 1.0.0-beta.15 fixes; the release itself was not changed for it.

The `Website run <https://github.com/insanai/guidedog/actions/runs/45231455588>`_ passed.

In the tagged `release run <https://github.com/insanai/guidedog/actions/runs/45237803897>`_ every build, the Windows and archive smoke tests, the source archive, the Homebrew job, publication and the refresh passed in the end.
The macOS Apple silicon build job, and then the Homebrew job, were cancelled four times in all, because GitHub assigned no macOS runner within the job's limit of 15 minutes.
We reran each cancelled job on the same run and did not move the tag.
The packages cover Linux x86-64 and ARM64, macOS Intel and Apple silicon, and Windows x86-64.

Published archives
------------------

The release carries ten assets: five platform archives, the source archive, ``guidedog.rb``, ``SHA256SUMS``, and the Graphviz 16.1.1 and Expat 2.7.1 source tarballs.
``SHA256SUMS`` lists these checksums:

.. code-block:: text

   92462230740310dfd55787c2bdb67a0543129e2b8e15bfce4365ffda46ec78c9  guidedog-linux-amd64.tar.gz
   843f45171c1c7144187dbfac8ea45f02060d1d94f5feca9cd7691c726e45ea51  guidedog-linux-arm64.tar.gz
   86b4ec5afc50dbfd07810707e951f3c70c495d5f92d30146dd07e7e2632f1c10  guidedog-macos-amd64.tar.gz
   2d048c591d541d0ae4fc6248dbaa162e2886ecedd700f69488896994d0ca4154  guidedog-macos-arm64.tar.gz
   b024b62f968e33fb9a47046a87ed07dded988ea0b1752c5a67ca4ef31a84b97a  guidedog-windows-amd64.zip
   72704c41392d6d452dc118bb5186d88253f7f4c4c4dc426d34bcc85487294fb2  guidedog-1.0.0-beta.14-source.tar.gz
   8bec0d0bf229b3d13f34a887b1e20f49d9adc6fe09b70796fe56e5c4ddb27d0c  guidedog.rb

The shared workflow in :doc:`../howto/github-pages` pins beta.14 and the published Linux x86-64 checksum for every manual edition.

Not yet checked
---------------

At the time of this record nobody has checked the website after publication.
The official tap's Update workflow has not been run or checked for beta.14 either.
This record does not cover a download of the published assets, a comparison of their digests, or their signed provenance.
