Mathematics and proofs
======================

An equation is useful when it makes a relation easier to inspect.
Define every symbol before relying on it. State the units where they matter.
The equation should support the explanation, not replace it.

Inline and display mathematics
------------------------------

Use the ``math`` role for a short expression. Use the ``math`` directive for a display.
Write the common LaTeX mathematics subset that both targets understand.

.. code-block:: rst

   A side of length :math:`s` encloses area :math:`s^2`.

   .. math::
      :label: area-square

      A = s^2

   The relation is :eq:`area-square`.

A side of length :math:`s` encloses area :math:`s^2`.

.. math::
   :label: area-square

   A = s^2

The relation is :eq:`area-square`.
Guidedog gives a labeled equation a stable target.
The HTML builder supplies MathJax markup. The PDF renderer translates the
supported LaTeX subset to Typst mathematics.
An unsupported PDF command is reported. Arbitrary LaTeX packages are not evaluated.

Faults in a formula
-------------------

Guidedog checks every LaTeX formula once, when it reads the document.
The check does not depend on the output: an HTML build with MathJax, a MathML build, ``guidedog convert`` and a PDF all give the same report.
A fault is a warning. The page is still written, with the formula as we wrote it.

.. code-block:: text

   -- WARNING: UNKNOWN MATH COMMAND [math.command.unknown] ---------- page.rst:1:15

   I do not know the LaTeX math command `\fraction`. A page shows it as an error or
   as text.

   1| Inline :math:`\fraction{a}{b}` and more.
                    ^^^^^^^^^

   Hint: Did you mean `\frac`?

The marker stands under the offending text, and the hint proposes the correction where one is close.
Only the first fault of a formula is reported.

.. list-table:: Math faults
   :header-rows: 1
   :widths: 34 66

   * - Code
     - Fault
   * - ``math.command.unknown``
     - A command that neither MathJax nor Guidedog knows, such as ``\fraction`` or ``\aplha``. The hint names the closest known command.
   * - ``math.environment.unknown``
     - ``\begin{algin}``: an environment nobody knows. The hint names the closest one.
   * - ``math.brace.unbalanced``
     - A ``{`` that no ``}`` closes, or a ``}`` that closes nothing.
   * - ``math.environment.unclosed``
     - ``\begin{x}`` without ``\end{x}``, or ``\end{x}`` without ``\begin{x}``.
   * - ``math.environment.mismatch``
     - ``\end{y}`` where ``\begin{x}`` is open.
   * - ``math.delimiter.unpaired``
     - ``\left`` without ``\right``, or the reverse.
   * - ``math.argument.missing``
     - A command with fewer arguments than it takes, such as ``\frac{a}``.
   * - ``math.alignment.outside``
     - ``&`` outside an environment, or ``\\`` in inline math.

A command is unknown only when no output can typeset it.
The check knows the commands of MathJax's base, AMS and autoloaded packages, such as ``\color``, ``\cancel`` and ``\ce``.
Some of them cannot become Typst mathematics. That is a narrower fault, and only the output that needs Typst reports it: ``typst.math.unsupported`` for a PDF, ``mathml.math.unsupported`` for MathML.
A MathJax page gets no report for them.

A command we define is known.
``\newcommand``, ``\renewcommand``, ``\providecommand``, ``\def``, ``\let``, ``\DeclareMathOperator`` and ``\newenvironment`` in a formula define a name for the rest of the document.
A project also names its MathJax macros in ``conf.toml``, and every document knows them:

.. code-block:: toml

   [mathjax3_config.tex.macros]
   RR = "{\\mathbb{R}}"

A displayed formula with ``\\`` is wrapped in an alignment, as Sphinx does, so ``&`` and ``\\`` are fine there.
With ``:nowrap:`` nothing is wrapped, and ``&`` needs its own environment.
The work is bounded: a formula longer than 64 KiB is not checked, and a document gets at most 64 math reports.
``-W`` and ``--strict`` treat these warnings as any other warning.
A formula written as MathML is not checked as LaTeX.

A small proof
-------------

Let :math:`U` be the bytes already charged to a budget.
Let :math:`B` be its limit. Let :math:`n` be a requested allocation.
Assume all three values are nonnegative and :math:`U \le B`.

**Admission rule.** Admit the request exactly when

.. math::
   :label: budget-admission

   n \le B - U.

**Claim.** An admitted request preserves :math:`U + n \le B`.

**Proof.** Add :math:`U` to both sides of :eq:`budget-admission`.
The result is :math:`U+n \le B`. The implementation tests the subtraction form.
It does not first compute :math:`U+n`, which could overflow a machine integer.
The proof and the arithmetic order agree.

**Boundary.** This accounts for charged host storage.
It does not bound the Typst compiler's internal allocations.
A proof is only as strong as the boundary it states.

HTML setup
----------

These projects bundle MathJax 3.2.2 with its SVG font data.
The bundle is loaded only on pages containing mathematics.
It makes these pages readable offline after a build.
No diagram script or web font service is required.

For our own projects, the default uses MathJax from a CDN.
To serve a local copy, we place it in ``_static`` and set:

.. code-block:: toml

   html_static_path = ["_static"]
   mathjax_path = "_static/tex-svg-full.js"

JavaScript is needed here to typeset web equations.
Without it, their source remains visible. PDF equations do not need browser JavaScript.
See the `MathJax component contract <https://docs.mathjax.org/en/v3.2/web/components/combined.html>`_.

MathML without a script
-----------------------

MathJax is the default. We can ask for MathML instead:

.. code-block:: toml

   html_math_renderer = "mathml"

``guidedog build html --math=mathml`` does the same for one build, and ``guidedog convert --math=mathml`` for one file.
Every formula is then written as a MathML ``math`` element, which current browsers show themselves.
The page loads no MathJax, and it is readable with JavaScript off.
Equation numbers, labels and ``:eq:`` references work as before.

.. code-block:: html

   <span class="math"><math><msup><mi>s</mi><mn>2</mn></msup></math></span>

Guidedog translates the LaTeX as it does for the PDF, and the embedded Typst compiler writes the MathML.
All formulas of a page are typeset in one run, and equal formulas are typeset once.
The elements are those of MathML Core, and no element has a ``style`` or ``id`` attribute.
A build adds ``_static/mathml.css``, a few rules for aligned equations and cases, to the pages that have a formula.

Some formulas cannot be MathML.
A LaTeX command the translation does not know is one case (``mathml.math.unsupported``).
A construct MathML Core cannot draw, such as ``\boxed``, ``\phantom``, ``\overline`` or ``\cancel``, is another (``mathml.math.lossy``).
Such a formula is not lost: the page shows its LaTeX in the usual markup, and a warning points at it.
In a MathML build no script typesets that LaTeX, so we rewrite the formula or keep MathJax for the project.

Formulas written as MathML
--------------------------

We can also write a displayed formula in MathML itself, with the ``notation`` option:

.. code-block:: rst

   .. math::
      :notation: mathml
      :label: half

      <math><mfrac><mi>a</mi><mn>2</mn></mfrac></math>

The content is one ``math`` element.
HTML output passes it through a filter, whatever the math renderer is.
The filter keeps the presentation elements of MathML Core and their plain attributes.
It leaves out everything else, such as ``style``, ``id``, ``href``, event attributes and foreign elements, and reports ``html.mathml.dropped``.
Content that is not one well-formed ``math`` element is shown as code, with ``html.mathml.invalid``.
The equation keeps its label and number.

The PDF typesets the formula too.
Guidedog reads the same filtered MathML Core and translates it to Typst math, so the book shows a real formula with its number, and ``:eq:`` links to it.

.. code-block:: text

   <mfrac><mi>a</mi><mn>2</mn></mfrac>                        frac(a, 2)
   <munderover><mo>&sum;</mo><mi>i</mi><mi>n</mi></munderover>  attach(limits(sum), b: i, t: n)
   <mrow><mo>(</mo><mtable>...</mtable><mo>)</mo></mrow>      mat(delim: "(", ...)

The translation knows every element the filter keeps.
Fractions, roots, scripts, limits, accents and prescripts become the Typst calls of the same meaning.
A table between fences is a matrix, a table after a lone ``{`` is ``cases``, and a table that is the whole formula is an aligned equation of several lines.
A fraction with ``linethickness="0"`` between parentheses is a binomial.
Fences around a row grow with their content.
The ``mathvariant`` attribute selects bold, italic, double-struck, script, fraktur, sans-serif, monospace or upright letters.
Greek letters, operators and arrows are written by their Typst names; any other character is written as itself.
A ``semantics`` element shows its first child, and its annotations are passed over.

No text of the formula can become Typst code.
Every character of it is written as a known name, a single letter or number, an escaped character, or a string.
For this reason the translation is also written in an ``--untrusted`` build, where raw Typst is left out.

The translator reads at most 256 KiB, 4096 elements and 48 levels of nesting in one formula.
A formula beyond these bounds, one with an element outside MathML Core such as ``menclose``, or one with a named character reference Guidedog does not know, is not translated.
The PDF then shows its source as code and reports ``typst.math.mathml``, which names what could not be translated.
This is a warning: the build goes on.
A numeric reference such as ``&#x2211;`` always works.

Text output shows the source, and ``--to rst`` keeps the option, so a converted document builds the same formula.
The ``math`` role has no such option: inline math is always LaTeX.
Raw HTML is a different matter: a ``math`` element inside ``raw`` content is passed or refused as a whole by the raw content rule, not filtered.
