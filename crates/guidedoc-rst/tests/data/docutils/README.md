# reStructuredText conformance corpus

Sources for `tests/rst_conformance_test.odin`, which compares Guidedoc's document trees
with those of Docutils 0.22. `corpus.txt` lists every source and the settings it is read
with.

## Origin and license

The layout mirrors the Docutils 0.22.4 source distribution, so relative includes and
image paths resolve as they do there:

- `docs/header2.rst`, `docs/ref/rst/*.rst` (the reStructuredText specification:
  `restructuredtext.rst`, `directives.rst`, `roles.rst`, `introduction.rst`, and the
  rest of that directory), and `docs/user/rst/*.rst`: Docutils documentation.
- `test/functional/input/*.rst` and `test/functional/input/data/*.rst`: the inputs of
  Docutils' functional tests (images, style sheets, and SVG files omitted).
- `include/isonum.txt`, `include/s5defs.txt`: standard definition files of the
  reStructuredText parser (`<name>` includes).
- `include/html-roles.txt`: standard definition file for HTML roles.

Docutils is placed in the public domain by its authors (see Docutils' `COPYING.rst`),
and so are these files, except `include/html-roles.txt`, which is © 2025 Günter Milde
under the 2-Clause BSD license stated in the file.

`extra/*.rst` are small sources written for Guidedoc to exercise the reader's settings
(smart quotes per language, implicit PEP and RFC references, bibliographic fields,
section subtitles, tabs, character-level markup, and more). They are public domain too.

## Expected trees

`expected/<source>.xml` is the Docutils XML of each source, produced by
`tools/rstref` with the options its settings name, using a Docutils-compatible native
program (no Python):

    odin run tools/rstref -- /path/to/docodin tests/data/docutils [source...]

The trees are committed; tests never run the program. Regenerate only after changing
`corpus.txt` or a source.

## Verification against Python Docutils

`tools/verify/docutils-corpus.sh` regenerates every tree and `ids.txt` with a pinned
Python Docutils (the `docutils` command, in a separate verification environment;
Guidedog never runs Python) and compares them byte for byte with the committed files:

    tools/verify/docutils-corpus.sh ~/verify-venv/bin/docutils /tmp/dc \
        tests/data/docutils/VERIFICATION.txt

`VERIFICATION.txt` is the record of the last run: date, host, Docutils and Python
versions, Guidedog revision, command, result, and the SHA-256 of every regenerated file
(check them with `sha256sum -c` against the committed files). On 30 September 2026,
with Docutils 0.22.4 on Python 3.13.5, all 98 trees and `ids.txt` were identical.

`ids.txt` lists the ids of each source's HTML page from Docutils 0.22.4's html5 writer,
with Sphinx's `auto_id_prefix = "id"`: the fragments a link can reach. A page that shows
system messages has a second line, `SOURCE (system messages): ...`, with the ids of the
messages, which Guidedoc does not write. It was written by Python Docutils:

    odin run tools/rstref -- --ids /path/to/docutils tests/data/docutils

For the documentation build and review workflow, see the [manual project](../../../docs/manual/README.md). These files are verification data, not a Python publication pipeline.
