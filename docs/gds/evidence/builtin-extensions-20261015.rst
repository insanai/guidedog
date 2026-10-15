Sphinx's built-in extensions, 15 October 2026
=============================================

Decision and scope
------------------

GDS 0011 asked Guidedog to support all of Sphinx's built-in extensions in its own way, without running Python. The nine missing extensions are now implemented as the record describes: ``sphinx.ext.napoleon``, ``autosummary``, ``apidoc``, ``coverage``, ``viewcode``, ``linkcode``, ``inheritance_diagram``, ``duration`` and ``imgconverter``. ``doctest`` stays display-only, as the record decides. With the eleven extensions supported before, every built-in extension of Sphinx 9.1 is handled.

Verification
------------

- ``cargo test --workspace`` passes 1,893 tests, none fail, 4 are ignored. The new tests include 18 end-to-end projects in ``tests/integration`` (HTML, text, and PDF for napoleon and inheritance diagrams).
- ``cargo clippy --workspace --all-targets``, ``cargo fmt --check`` and ``cargo run -p check`` (1,059 files, no limit violation) are clean.
- napoleon converts all 148 docstrings of Sphinx's own napoleon tests line for line as Sphinx does. A one-time comparison with Sphinx over about 12,700 real docstrings found no difference in output; 13 malformed docstrings are reported instead of converted.
- ``cargo xtask sphinxdiff`` against a pinned Sphinx 9.1.0 on Sphinx's test roots: ``test-ext-napoleon``, ``test-ext-autosummary-filename-map``, ``-template``, ``-module_all``, ``test-ext-apidoc`` and ``test-ext-imgconverter`` are identical in what the comparison checks. ``test-ext-autosummary`` and ``-recursive`` differ in one page each, for a module that calls ``sys.exit`` when imported: Guidedog never runs the module, so it documents it. In ``test-ext-viewcode`` every source link both builds produce agrees; the remaining differences come from the C domain and from Sphinx's ``conf.py`` hooks. ``test-ext-inheritance_diagram`` draws the same classes and links; Sphinx draws PNG image maps, Guidedog linked SVG.
- ``coverage`` writes ``python.txt`` identical to Sphinx's on ``test-ext-coverage``.
- Django's manual, built with ``linkcode``, has 1,464 source links to GitHub, and sampled links land on the definitions they name. Its 678 pages and Flask's 79 keep every internal link.
- The four release smoke tests pass with a minimal environment.

Limits
------

- autosummary approximates Docutils' parse of a docstring's first sentence by detecting inline markup that is opened and not closed.
- napoleon shows annotation types as the source writes them.
- viewcode and linkcode find files and lines for Python objects; objects of the C, C++, Rust and Odin domains carry no source location, so a linkcode template can use only their domain, module, name and branch.
- Diagrams are linked SVG; ``graphviz_output_format = "png"`` is read as SVG with a note.
