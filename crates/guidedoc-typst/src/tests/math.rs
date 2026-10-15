//! The LaTeX math adapter: translations, failures, and Typst validity.

use super::expect_compiles;
use crate::PREAMBLE;
use crate::math::{COMMANDS, Kind};
use crate::translate_math;

/// LaTeX and the Typst math it becomes.
const MATH_CASES: [(&str, &str); 79] = [
    ("xy", "x y"),
    ("x^2 + y^2 = z^2", "x^(2) + y^(2) = z^(2)"),
    ("a_{ij}^{n+1}", "a_(i j)^(n + 1)"),
    ("x^23", "x^(2) 3"),
    ("3.14 r", "3.14 r"),
    ("\\frac{a}{b}", "frac(a, b)"),
    ("\\frac12", "frac(1, 2)"),
    ("\\dfrac{1}{x}", "display(frac(1, x))"),
    ("\\tfrac{1}{x}", "inline(frac(1, x))"),
    ("\\sqrt{x}", "sqrt(x)"),
    ("\\sqrt[3]{x+1}", "root(3, x + 1)"),
    ("\\left( \\frac{a}{b} \\right)", "lr(\\( frac(a, b) \\))"),
    ("\\left\\{ x \\middle| x > 0 \\right\\}", "lr(\\{ x mid(|) x > 0 \\})"),
    ("\\left. f \\right|_0^1", "lr(f |)_(0)^(1)"),
    ("\\langle u, v \\rangle", "chevron.l u \\, v chevron.r"),
    ("a \\cdot b \\times c \\pm d \\mp e", "a dot.op b times c plus.minus d minus.plus e"),
    ("a \\leq b \\geq c \\neq d", "a lt.eq b gt.eq c eq.not d"),
    ("a \\approx b \\equiv c \\sim d \\propto e", "a approx b equiv c tilde.op d prop e"),
    ("\\infty \\partial \\nabla \\forall \\exists", "infinity partial nabla forall exists"),
    ("x \\in A \\notin B \\subset C \\subseteq D", "x in A in.not B subset C subset.eq D"),
    ("A \\cup B \\cap C", "A union B inter C"),
    ("f: A \\to B \\rightarrow C \\Rightarrow D", "f : A arrow.r B arrow.r C arrow.r.double D"),
    (
        "a \\leftarrow b \\Leftrightarrow c \\mapsto d",
        "a arrow.l b arrow.l.r.double c arrow.r.bar d",
    ),
    ("1, \\ldots, n \\cdots \\dots", "1 \\, dots.h \\, n dots.h.c dots.h"),
    ("\\alpha\\beta\\gamma", "alpha beta gamma"),
    ("\\epsilon \\varepsilon \\phi \\varphi", "epsilon.alt epsilon phi.alt phi"),
    ("\\Gamma \\Delta \\Omega \\vartheta", "Gamma Delta Omega theta.alt"),
    ("\\sum_{i=1}^{n} i", "sum_(i = 1)^(n) i"),
    (
        "\\prod_k \\int_0^1 \\iint \\oint",
        "product_(k) integral_(0)^(1) integral.double integral.cont",
    ),
    ("\\lim_{x \\to 0} \\frac{\\sin x}{x}", "lim_(x arrow.r 0) frac(sin x, x)"),
    ("\\max \\min \\sup \\inf \\log \\ln \\exp", "max min sup inf log ln exp"),
    ("\\cos \\tan", "cos tan"),
    ("\\mathbf{v}", "bold(upright(v))"),
    ("\\mathbf{AB}", "bold(upright(\"AB\"))"),
    ("\\mathit{x}", "italic(x)"),
    ("\\mathrm{d}x", "upright(d) x"),
    ("\\mathrm{max}", "upright(\"max\")"),
    ("\\operatorname{rank} A", "op(\"rank\") A"),
    ("\\operatorname*{arg\\,max}_x", "op(\"arg max\", limits: #true)_(x)"),
    ("\\mathbb{R} \\mathcal{L}", "bb(R) cal(L)"),
    ("\\text{if } x", "#box(text(font: gd-math-font, \"if \")) x"),
    ("\\textbf{bold}", "#box(text(font: gd-math-font, weight: \"bold\", \"bold\"))"),
    ("\\text{क्षेत्रफल}", "#box(text(font: gd-math-font, \"क्षेत्रफल\"))"),
    ("\\textit{한국어}", "#box(text(font: gd-math-font, style: \"italic\", \"한국어\"))"),
    ("\\texttt{保存}", "#box(text(font: gd-mono + gd-math-font, \"保存\"))"),
    ("\\textsf{マニュアル}", "#gd-math-sans-text(gd-math-font, \"マニュアル\")"),
    ("\\hat{x} \\bar{y} \\vec{v}", "hat(x) macron(y) arrow(v)"),
    ("\\dot{x} \\ddot{x} \\tilde{n}", "dot(x) dot.double(x) tilde(n)"),
    ("\\overline{AB} \\underline{c}", "overline(A B) underline(c)"),
    ("\\binom{n}{k}", "binom(n, k)"),
    ("\\begin{matrix} a & b \\\\ c & d \\end{matrix}", "mat(delim: #none, a, b; c, d)"),
    ("\\begin{pmatrix} 1 & 0 \\\\ 0 & 1 \\end{pmatrix}", "mat(delim: \"(\", 1, 0; 0, 1)"),
    ("\\begin{bmatrix} a \\\\ b \\\\ \\end{bmatrix}", "mat(delim: \"[\", a; b)"),
    ("\\begin{vmatrix} a & \\\\ c & d \\end{vmatrix}", "mat(delim: \"|\", a, \"\"; c, d)"),
    ("\\begin{Bmatrix} x \\end{Bmatrix}", "mat(delim: \"{\", x)"),
    (
        "f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & \\text{otherwise} \\end{cases}",
        "f \\( x \\) = cases(1 & x > 0, 0 & #box(text(font: gd-math-font, \"otherwise\")))",
    ),
    ("\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}", "a & = b \\ c & = d"),
    ("\\begin{align*} x &= 1 \\end{align*}", "x & = 1"),
    ("a \\quad b \\qquad c \\, d \\; e \\! f", "a quad b wide c thin d thick e #h(-0.1667em) f"),
    ("\\{ x \\} \\| y \\|", "\\{ x \\} ‖ y ‖"),
    ("a % comment\n + b", "a + b"),
    ("a/b * c", "a \\/ b \\* c"),
    ("f'(x) + g''", "f' \\( x \\) + g''"),
    ("{}^{14}C", "\"\"^(14) C"),
    ("^2", "\"\"^(2)"),
    ("\\not= \\not\\in \\not x", "eq.not in.not cancel(x)"),
    ("a \\pmod{n}", "a \\( mod n \\)"),
    ("\\overset{!}{=}", "limits(=)^(!)"),
    ("\\underset{x}{\\arg\\min}", "limits(arg min)_(x)"),
    ("\\big( x \\Big]", "\\( x \\]"),
    ("\\boxed{x}", "#box(stroke: 0.5pt, inset: 2pt, $x$)"),
    ("\\displaystyle \\sum x \\label{eq}", "sum x"),
    ("x_{\\text{max}}", "x_(#box(text(font: gd-math-font, \"max\")))"),
    ("α β", "α β"),
    ("\\\\", ""),
    ("a \\\\ b", "a \\ b"),
    ("x^{a^{b}}", "x^(a^(b))"),
    ("\"#$@`~", "\\\" \\# \\$ \\@ \\` space"),
    ("[0, 1)", "\\[ 0 \\, 1 \\)"),
];

/// LaTeX the adapter refuses, and the construct it names.
const MATH_FAILURES: [(&str, &str); 11] = [
    ("\\foo x", "\\foo"),
    ("\\color{red} x", "\\color"),
    ("{x", "an unclosed {"),
    ("x}", "an unmatched }"),
    ("\\left( x", "\\left without \\right"),
    ("x \\right)", "\\right without \\left"),
    ("\\begin{tabular} x \\end{tabular}", "an unsupported \\begin environment"),
    ("\\begin{matrix} x \\end{pmatrix}", "an \\end that does not match its \\begin"),
    ("\\frac{a}", "a missing argument"),
    ("x^", "a missing argument"),
    ("\\left\\foo x \\right)", "\\foo"),
];

#[test]
fn math_translates_latex() {
    let mut out = String::new();
    for (latex, typst) in MATH_CASES {
        out.clear();
        let result = translate_math(latex, &mut out);
        assert_eq!(result, Ok(()), "{latex:?}");
        assert_eq!(out, typst, "{latex:?}");
    }
}

#[test]
fn math_reports_unsupported_commands() {
    let mut out = String::new();
    for (latex, construct) in MATH_FAILURES {
        let e = translate_math(latex, &mut out).expect_err(latex);
        assert_eq!(e.construct, construct, "{latex:?}");
    }
    assert!(translate_math(&"{".repeat(200), &mut out).is_err());
    // Multi-byte characters after a backslash or as delimiters are named whole.
    assert_eq!(translate_math("\\é", &mut out).map_err(|e| e.construct), Err("\\é"));
    assert_eq!(translate_math("\\left é x", &mut out).map_err(|e| e.construct), Err("é"));
}

/// Every expected translation and every table command is valid Typst.
#[test]
fn math_output_compiles() {
    let mut source = String::from(PREAMBLE);
    source.push_str("\n#context { let gd-math-font = (\"New Computer Modern\",);\n");
    let mut out = String::new();
    for (latex, _) in MATH_CASES {
        out.clear();
        assert!(translate_math(latex, &mut out).is_ok());
        source.push_str(&format!("$ {out} $\n\n"));
    }
    for c in COMMANDS {
        let args = if c.kind == Kind::Delimiter { "( x" } else { "{x}{y}" };
        let latex = format!("\\{}{args}", c.name);
        out.clear();
        assert!(translate_math(&latex, &mut out).is_ok(), "{} failed", c.name);
        source.push_str(&format!("$ {out} $\n"));
    }
    source.push_str("\ngd-math-sans.update((\"New Computer Modern\",))\n");
    source.push_str("$ #gd-math-sans-text(gd-math-font, \"named text\") $\n}");
    expect_compiles("math", &source, false);
}
