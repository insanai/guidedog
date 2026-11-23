//! The math check end to end: a formula no output can typeset is reported once, at the
//! formula, with the correction, whatever the output is; valid LaTeX is left alone.

use guidedog_tests::{Scratch, guidedog, has, lacks};

use crate::{build, flat, project};

const FAULTS: &str = "\
Inline :math:`\\fraction{a}{b}` and :math:`a & b`.

.. math::

   \\begin{pmatrix} a & b \\\\
   c & d \\end{bmatrix}

.. math::

   x = \\left( \\frac{a} + {y

.. math::
   :nowrap:

   \\begin{algin} a \\end{algin}
";

#[test]
fn convert_reports_each_faulty_formula_with_a_correction() {
    let dir = Scratch::new("math-faults");
    dir.write("page.rst", FAULTS);
    let args = ["convert", "page.rst", "--from", "sphinx", "-o", "page.html", "--color=never"];
    let r = guidedog(&dir.path, 0, &args);
    let err = flat(&r.err);
    has(&r.err, "-- WARNING: UNKNOWN MATH COMMAND [math.command.unknown]");
    has(&r.err, "page.rst:1:15");
    has(
        &r.err,
        "1| Inline :math:`\\fraction{a}{b}` and :math:`a & b`.\n                 ^^^^^^^^^",
    );
    has(&err, "I do not know the LaTeX math command `\\fraction`.");
    has(&err, "Hint: Did you mean `\\frac`?");
    has(&r.err, "[math.alignment.outside]");
    has(&r.err, "page.rst:1:45");
    has(&r.err, "[math.environment.mismatch]");
    has(&err, "`\\end{bmatrix}` closes `\\begin{pmatrix}`.");
    has(&err, "Hint: Write `\\end{pmatrix}`, or begin the environment as `bmatrix`.");
    has(&r.err, "[math.brace.unbalanced]");
    has(&r.err, "page.rst:10:26");
    has(&r.err, "[math.environment.unknown]");
    has(&err, "Hint: Did you mean `align`?");
    assert_eq!(r.err.matches("-- WARNING").count(), 5, "{}", r.err);
    // The page is written all the same: the LaTeX is there for MathJax.
    has(&dir.read("page.html"), "\\(\\fraction{a}{b}\\)");
    // One report per formula for every output, and the same report as JSON.
    let mathml = ["convert", "page.rst", "--from", "sphinx", "-o", "m.html", "--math=mathml"];
    let r = guidedog(&dir.path, 0, &mathml);
    assert_eq!(r.err.matches("[math.").count(), 5, "{}", r.err);
    let json = ["convert", "page.rst", "--from", "sphinx", "-o", "-", "--diagnostics=json"];
    let r = guidedog(&dir.path, 0, &[&json[..], &["--to", "html", "--fragment"]].concat());
    has(&r.err, "\"code\":\"math.command.unknown\"");
    has(&r.err, "\"hint\":\"Did you mean `\\\\frac`?\"");
    has(&r.err, "\"line\":1,\"column\":15,\"end_line\":1,\"end_column\":24");
    let strict = ["convert", "page.rst", "--from", "sphinx", "-o", "s.html", "--strict"];
    guidedog(&dir.path, 1, &strict);
}

#[test]
fn markdown_formulas_are_checked_too() {
    let dir = Scratch::new("math-markdown");
    dir.write("page.md", "Euler: $e^{i\\pi} + 1 = 0$ and $\\aplha$.\n\n$$\n\\frac{a}\n$$\n");
    let args = ["convert", "page.md", "--from", "myst", "-o", "page.html", "--color=never"];
    let r = guidedog(&dir.path, 0, &args);
    has(&r.err, "page.md:1:32");
    has(&flat(&r.err), "Hint: Did you mean `\\alpha`?");
    has(&r.err, "[math.argument.missing]");
    has(&flat(&r.err), "Hint: Write both arguments, as in `\\frac{a}{b}`.");
    assert_eq!(r.err.matches("-- WARNING").count(), 2, "{}", r.err);
}

#[test]
fn valid_latex_is_not_reported() {
    let dir = Scratch::new("math-valid");
    dir.write(
        "page.rst",
        ":math:`\\operatorname{foo}(x)`, :math:`\\color{red} x`, :math:`\\ce{H2O}`, \
         :math:`\\newcommand{\\RR}{\\mathbb{R}} \\RR` and later :math:`x \\in \\RR`.\n\n\
         .. math::\n\n   a &= b \\\\\n   c &= d\n\n.. math::\n   :nowrap:\n\n   \
         \\begin{align}\n   a &= \\left\\{ b \\right. \\\\\n   c &= \\sqrt[3]{d}\n   \
         \\end{align}\n\n.. math::\n\n   \\int_0^\\infty \\frac{\\sin x}{x}\\,dx = \\frac\\pi2\n",
    );
    let args = ["convert", "page.rst", "--from", "sphinx", "-o", "page.html", "--strict"];
    let r = guidedog(&dir.path, 0, &args);
    assert!(r.err.is_empty(), "{}", r.err);
    // The Typst adapter does not translate \\color: that is a fault of the PDF only,
    // told by the renderer, and it is not "unknown".
    dir.write("pdf.rst", ":math:`\\color{red} x`\n");
    let args = ["convert", "pdf.rst", "--from", "sphinx", "-o", "p.pdf", "--color=never"];
    let r = guidedog(&dir.path, 1, &args);
    has(&r.err, "typst.math.unsupported");
    lacks(&r.err, "math.command.unknown");
}

#[test]
fn a_build_reports_a_formula_once_and_knows_the_mathjax_macros() {
    let conf = "project = \"Demo\"\n\n[mathjax3_config.tex.macros]\nRR = \"{\\\\mathbb{R}}\"\n";
    let dir = project("math-build", conf);
    dir.write(
        "index.rst",
        "Demo\n====\n\n:math:`x \\in \\RR` and :math:`\\fraction{a}{b}`.\n\n.. math::\n   \
         :label: e\n\n   \\left( a\n",
    );
    for args in [&["html", "-E"][..], &["html", "-E", "--math=mathml"][..]] {
        let r = build(&dir.path, 0, args);
        assert_eq!(r.err.matches("[math.command.unknown]").count(), 1, "{}", r.err);
        assert_eq!(r.err.matches("[math.delimiter.unpaired]").count(), 1, "{}", r.err);
        has(&r.err, "index.rst:4:30");
        has(&flat(&r.err), "Hint: Did you mean `\\frac`?");
        lacks(&r.err, "command `\\RR`");
    }
    let r = build(&dir.path, 1, &["html", "-E", "-W"]);
    has(&r.err, "math.command.unknown");
}
