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
Guidedog gives a labelled equation a stable target.
The HTML builder supplies MathJax markup. The PDF renderer translates the
supported LaTeX subset to Typst mathematics.
An unsupported PDF command is reported. Arbitrary LaTeX packages are not evaluated.

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
