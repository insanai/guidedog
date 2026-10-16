PDF pagination review
=====================

On 16 October 2026 we investigated `issue 1 <https://github.com/insanai/guidedog/issues/1>`_: a captioned table with 120 rows compiled successfully but overflowed the bottom of a PDF page.

Root cause
----------

The renderer wraps captioned tables in ``figure(kind: table, ...)`` and captioned code listings in ``figure(kind: raw, ...)``. Typst makes figures unbreakable by default. The book template already made the table and code block bodies breakable, but an unbreakable outer figure still prevented them from continuing onto another page. This also affected standalone conversion, which has no book template.

The `Typst figure documentation <https://typst.app/docs/reference/model/figure/#figure-behaviour>`_ describes this distinction between breakable contents and a breakable containing figure.

Fix and scope
-------------

The shared Typst preamble makes table and raw-code figures breakable. Both book composition and standalone conversion use this preamble. Existing copies of the book template inherit the fix without being replaced. Image figures retain their existing layout behavior.

A project template can still override the generated defaults deliberately. When we customize a container around a table or listing, every enclosing block must allow page breaks if its contents can exceed a page.

Regression checks
-----------------

Before the fix, the table and listing tests failed because their first and last entries appeared on the same laid-out page. The standalone conversion test failed for the same reason.

After the fix, the tests verify all 120 entries exactly once across multiple pages. Tables repeat their header on every table page. The checks cover captioned and uncaptioned tables, captioned listings, the built-in book template, a copied quickstart template and standalone conversion. They use the embedded compiler's laid-out pages rather than checking for a particular generated rule.

Independent PDF check
---------------------

We built the same inputs with the published 1.0.0-beta.1 executable and the corrected working tree, then inspected the PDF text coordinates with PyMuPDF 1.28.2. A table in the old PDF had 120 text labels but only 44 distinct row positions: the remaining labels overlapped the final row. A captioned listing exposed only 85 of its 120 lines. Counting text labels alone would miss the table defect.

The corrected PDFs contain all 120 entries at distinct, non-overlapping positions. The table spans three content pages with repeated headers, and the listing spans two content pages. The copied-template table has the same result as the built-in template. The independent check also verifies that consecutive entry rectangles do not overlap vertically.
