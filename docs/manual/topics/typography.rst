Typography for reading
======================

A page should make its structure visible before the reader studies it.
A strong title, a quiet navigation column, and a measured text column do most of the work.
Color should mark a relation or an action. It should not carry the meaning alone.

The web page
------------

The custom layout has three regions: project navigation, the document,
and the current page's outline. Small screens use a navigation drawer.
The reading edition bundles Libertinus Serif and Libertinus Sans.
Titles use a bold serif face. Navigation, labels, and controls use its sans companion.
The compressed WOFF2 files are served with the site.
There are no font-service requests.

.. graphviz::
   :class: graphviz-adaptive
   :caption: Three-region reading layout for HTML documentation.
   :alt: Navigation column on left, main document text column in center, local section outline on right.

   digraph reading_layout {
     graph [rankdir=LR, bgcolor="transparent", pad="0.22", nodesep="0.18", ranksep="0.22"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2", color="#216553",
           fontname="Helvetica", fontcolor="#16382f", fontsize=12, margin="0.18,0.1"];
     edge [color="#216553", fontname="Helvetica", fontcolor="#16382f", fontsize=10, arrowsize=0.7];
     nav [class="gd-input", fillcolor="#fbfaf3", width=1.2,
          label=<<B>Navigation</B><BR/><FONT POINT-SIZE="11">Project tree</FONT>>];
     doc [width=1.2, label=<<B>Document</B><BR/><FONT POINT-SIZE="11">Reading text</FONT>>];
     toc [class="gd-input", fillcolor="#fbfaf3", width=1.2,
          label=<<B>Outline</B><BR/><FONT POINT-SIZE="11">This page</FONT>>];
     nav -> doc -> toc [style=invis];
   }

The built-in theme follows the operating system by default, with light and dark choices;
Guidedog's own documentation website defaults to the light reading edition.
The theme control remembers the reader's choice on the same browser origin.
Printed HTML uses light colors.
Content and navigation remain usable without JavaScript.
The theme control, enhanced search, and paragraph line breaking are progressive enhancements.

Every initialized project (``guidedog quickstart``, ``guidedog ssg init``, and ``guidedog dsc init``)
copies ``_static/typography.js`` beside ``_static/guidedog.css`` and ``_static/guidedog.js`` so a
project owns and can edit its scripts directly. On desktop reading measures (``40rem`` and wider),
English prose uses paragraph-wide **Knuth-Plass line breaking** in the browser across paragraphs,
list items, definitions, admonitions, block quotes, and footnotes, while leaving tables,
navigation trees, metadata headers, and figure captions ragged.
The script measures the loaded font, builds boxes, glue, and hyphenation penalties,
and chooses breaks for the whole block. Frank Liang's pattern method supplies English
hyphenation candidates, and break penalty widths are discarded after a hyphenated break so
following lines are measured accurately. Inline mathematics (after MathJax finishes),
footnote references, superscripts, and subscripts are measured as inline atoms; links keep
their markup, and inline code literals may break cleanly after ``/`` or ``::``.
The original TeX patterns and the libraries' BSD notices are recorded in ``NOTICE``.

The enhancement preserves emphasis and the exact source text.
It recalculates when the reading measure changes. Copying within or across typeset
blocks gives the source text, without visual line divisions or added hyphens.
Narrow screens (below ``40rem``) and printing restore ordinary layout so mobile browsers
wrap naturally and print engines can paginate paragraphs.

Other languages, narrow measures, JavaScript-disabled browsers, and paragraphs for which
no suitably spaced solution exists use native browser layout with ``text-wrap: pretty``
(without greedy CSS justification or browser ``hyphens: auto``).
That CSS property alone is not a promise of Knuth-Plass optimization.

The PDF page
------------

The book and standalone article templates use the same Libertinus family as the web edition. They set justified paragraphs, optimized line breaking, and language-aware hyphenation. They also control spacing, running heads, captions, code blocks, and table rules. The font, hyphenation cost, and paragraph measure work together. A forced line break is rarely the right repair for a loose paragraph.

Typst optimizes the PDF paragraphs. The web enhancement optimizes eligible prose.
They share the source, fonts, and visual hierarchy. Their line breaks can differ.
See the `Typst text contract <https://typst.app/docs/reference/text/text/>`_
and the `browser hyphenation contract <https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/hyphens>`_.

Change the design
-----------------

Edit ``_static/editorial.css`` for the documentation project's visual tokens. Edit ``_templates/layout.html`` for structure. Edit ``_templates/theme.typ`` for the PDF palette and typography. Edit ``_templates/book.typ`` for book structure, ``_templates/discussion.typ`` for discussion structure, and ``_templates/article.typ`` for standalone article PDFs. All three layouts wrap their content with the shared language-aware theme. Keep reading order, focus indicators, labels, and sufficient contrast intact.

Build both targets after a change. Inspect a page with code, a table,
a diagram, and an equation. An attractive cover is not proof that the rest of the book works.
We build our own design in :doc:`../tutorial/a-custom-theme`.
The full template contract is in :doc:`../templates`.
The Rust API of Guidedog's crates is documented in the separate ``docs/api`` project.
