MyST Markdown conformance data
==============================

Reference data for tests/myst_conformance, which compares the document trees the
"myst" reader builds (with the Sphinx layer's directives and roles, when present) with
the Docutils trees myst-parser 0.16.1 builds for the same input.

fixtures/*.md are the test fixtures of myst-parser 0.16.1
(tests/test_renderers/fixtures/ in the v0.16.1 release of
https://github.com/executablebooks/MyST-Parser), unchanged. Each case is a title line,
the Markdown input between "." lines, and the expected Docutils tree in pseudo-XML, or
the expected warnings for reporter_warnings.md and mock_include_errors.md.

sphinx/ holds the source directories of myst-parser's Sphinx build tests
(tests/test_sphinx/sourcedirs/, text files only) and, as <directory>/<document>.xml,
the document trees those tests expect after Sphinx reads the document. Their conf.py
files are kept as data and never run; sphinx.txt transcribes the settings they make.
fixtures.txt lists how the harness reads each fixture file.

The files are (c) 2020 ExecutableBookProject, under the MIT License in LICENSE. They are
used as data only: no Python runs, and no code of myst-parser or markdown-it is used.
