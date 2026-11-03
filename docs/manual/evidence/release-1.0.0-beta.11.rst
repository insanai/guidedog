Release 1.0.0-beta.11
=====================

On 3 November 2026 we published `v1.0.0-beta.11 <https://github.com/insanai/guidedog/releases/tag/v1.0.0-beta.11>`_ as a normal GitHub release and marked it latest.
The annotated tag names commit ``0f91b9f37e147a0f5136041ba5ec1e1d39795970``.
We pushed the tag after both Verify and Website succeeded for that exact commit.

What the release holds
----------------------

The release merges the branch ``nisaba-source-maps``, which gives editors four interfaces to Guidedog (GDS 0017): editable source maps and source-preserving edits in the new crate ``guidedoc-edit``, source positions in preview HTML and PDF page maps, a dependency manifest beside every build's output, and ``guidedog capabilities``.
nisaba, a collaborative authoring application in a sibling repository, uses them.

It also fixes two faults that nisaba found in Guidedog.
With Graphviz 2.42, which many Linux distributions ship, a read left the text after the first graph in the scanner's global buffer, so the next diagram placed its first error a line late, or drew a graph from the diagram before.
We now empty that buffer after each read, and a test shows both effects fail without the fix and pass with Graphviz 2.42 and 16.1; release archives link 16.1, which was not affected.
``gd-pages`` read a refused push as a conflict only for some wordings: stock Git says "failed to update ref" and GitHub "cannot lock ref ...: is at X but expected Y".
``push`` now takes the commit the branch held and reads the branch again after a refusal it cannot place, and its pack writer is public, so nisaba no longer copies it.
A test that composes the public pack writer and push against a real ``git receive-pack`` fails without the fix.

Review before the release
-------------------------

Three independent reviews read the branch before we merged it, and we fixed every confirmed finding with a test that fails without the fix.
The edit crate panicked on some MyST comments and empty list items, took an RST title's underline as the next title's overline, kept quote markers and fence lines in the editable text of quoted code, lost the last line of an unclosed fence, and let through edits that a truncated map, a changed field name, a YAML comment or CRLF line endings made unsafe.
Preview positions were three bytes short after a byte order mark, named generated text such as ``<rst_epilog>`` as files, and landed on a label's empty anchor instead of its list or table.
The dependency manifest hid a page named ``dependencies``, left out what singlehtml's page, the search index and static files were made from, and listed markers and a refused template as files.
Fixing the manifest showed that Guidedog kept a document's page when a file it included was created later; it now records the missing file and reads the document again until the file appears.

Qualification
-------------

The exact commit's `Verify run <https://github.com/insanai/guidedog/actions/runs/41574915444>`_ passed on Linux x86-64 and macOS Apple silicon.
The workspace suites passed 2,204 tests on Linux and 2,203 on macOS, each with five ignored.
Formatting, Clippy with warnings denied, compilation, the generated tables, the source limits, the pinned music font, the music corpus and the music fuzz cases passed.
The tagged commit changes only Korean catalogs from its parent, so Verify skipped it by path, and we started this run by hand, as Release requires a Verify run for the exact commit.

The `Website run <https://github.com/insanai/guidedog/actions/runs/41574574688>`_ built and deployed all seven language editions with their manual PDFs.
It checked 922 HTML pages and 210,177 internal links with no missing files or anchors.
The first Website run for the release, on the parent commit, failed: four Korean messages wrote a particle straight after inline markup, which leaves the markup unterminated.
We escaped them as the other Korean messages do and built every edition locally with warnings treated as errors before pushing again.
All six translated manuals and website editions have zero fuzzy or untranslated messages.

Published archives
------------------

All sixteen jobs in the `release run <https://github.com/insanai/guidedog/actions/runs/41576086491>`_ passed.
The five native platform builds, fresh-runner archive smoke tests and Homebrew formula installation passed before publication.

We downloaded all ten published assets and verified their GitHub API digests and sizes, including ``SHA256SUMS`` itself.
All nine entries in that checksum manifest match.
The Linux x86-64 archive's build manifest names 1.0.0-beta.11 and the tagged commit, its program hash matches the archived executable, and the executable reports ``guidedog 1.0.0-beta.11`` with Graphviz 16.1.0 linked statically and Typst 0.15.1 embedded.
Its ``guidedog capabilities --json`` reports schema 1 and source map version 1.

GitHub's attestation API holds a provenance statement for the Linux archive's digest.
We did not verify the signature locally.

The published Linux x86-64 archive has SHA-256:

.. code-block:: text

   72e6fd1e4184c1b65e1344c043e7325b64ca4edef592404f5e8dd38dd398f375

Homebrew and documentation
--------------------------

The tap's `Update run <https://github.com/insanai/homebrew-guidedog/actions/runs/41578076514>`_ installed and tested the published formula on Apple silicon and Intel macOS.
It committed ``c9b333c14e576b8a8ecb57314a19e1ffe24356a7``.
The tracked formula matches the published ``guidedog.rb`` byte for byte.
The shared workflow example in :doc:`../howto/github-pages` pins beta.11 and its verified published Linux checksum for every language edition.
