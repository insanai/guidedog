//! MathML an author wrote, in a real PDF build: the embedded Typst compiler typesets
//! the translation, so every sample here is proven to be valid Typst math.

use guidedog_tests::{has, lacks};

use crate::support::{both, build, project};

/// A name and the inside of a `math` element.
const SAMPLES: [(&str, &str); 12] = [
    (
        "fraction",
        "<mfrac><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow><mrow><mn>2</mn><mi>c</mi>\
         </mrow></mfrac>",
    ),
    (
        "roots",
        "<msqrt><mi>x</mi><mo>+</mo><mn>1</mn></msqrt><mo>+</mo><mroot><mi>y</mi><mn>3</mn>\
         </mroot>",
    ),
    (
        "scripts",
        "<msup><mi>x</mi><mn>2</mn></msup><mo>+</mo><msubsup><mi>T</mi><mi>k</mi><mrow><mi>n</mi>\
         <mo>+</mo><mn>1</mn></mrow></msubsup><mo>+</mo><mmultiscripts><mi>C</mi><mn>6</mn>\
         <mn>14</mn><mprescripts/><mn>1</mn><mn>2</mn></mmultiscripts>",
    ),
    (
        "sum",
        "<munderover><mo>&#x2211;</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mi>n</mi>\
         </munderover><msup><mi>i</mi><mn>2</mn></msup>",
    ),
    (
        "integral",
        "<msubsup><mo>&int;</mo><mn>0</mn><mi>&infin;</mi></msubsup><msup><mi>e</mi><mrow>\
         <mo>-</mo><msup><mi>x</mi><mn>2</mn></msup></mrow></msup><mspace width=\"0.1667em\"/>\
         <mi mathvariant=\"normal\">d</mi><mi>x</mi>",
    ),
    (
        "matrix",
        "<mi>A</mi><mo>=</mo><mrow><mo>(</mo><mtable><mtr><mtd><mi>a</mi></mtd><mtd><mi>b</mi>\
         </mtd></mtr><mtr><mtd><mi>c</mi></mtd><mtd><mi>d</mi></mtd></mtr></mtable><mo>)\
         </mo></mrow>",
    ),
    (
        "cases",
        "<mo>|</mo><mi>x</mi><mo>|</mo><mo>=</mo><mrow><mo>{</mo><mtable><mtr><mtd><mi>x</mi>\
         </mtd><mtd><mtext>if&nbsp;</mtext><mi>x</mi><mo>&ge;</mo><mn>0</mn></mtd></mtr><mtr>\
         <mtd><mo>-</mo><mi>x</mi></mtd><mtd><mtext>otherwise</mtext></mtd></mtr></mtable></mrow>",
    ),
    (
        "accents",
        "<mover accent=\"true\"><mi>x</mi><mo>^</mo></mover><mo>+</mo><mover><mi>v</mi>\
         <mo>&rarr;</mo></mover><mo>+</mo><munder accentunder=\"true\"><mrow><mi>a</mi><mo>+</mo>\
         <mi>b</mi></mrow><mo>&#x23DF;</mo></munder>",
    ),
    (
        "binomial",
        "<mrow><mo>(</mo><mfrac linethickness=\"0\"><mi>n</mi><mi>k</mi></mfrac><mo>)</mo></mrow>",
    ),
    (
        "aligned",
        "<mtable><mtr><mtd><msup><mrow><mo>(</mo><mi>a</mi><mo>+</mo><mi>b</mi><mo>)</mo></mrow>\
         <mn>2</mn></msup></mtd><mtd><mo>=</mo><msup><mi>a</mi><mn>2</mn></msup><mo>+</mo>\
         <mn>2</mn><mi>a</mi><mi>b</mi></mtd></mtr><mtr><mtd></mtd><mtd><mo>&ge;</mo><mn>0</mn>\
         </mtd></mtr></mtable>",
    ),
    ("text", "<mfrac><mtext>favourable cases</mtext><mtext>all cases, 100%</mtext></mfrac>"),
    (
        "greek",
        "<mi>&alpha;</mi><mo>&le;</mo><mi>&Omega;</mi><mo>&rarr;</mo><mi \
         mathvariant=\"double-struck\">\
         R</mi><mo>,</mo><mo>&forall;</mo><mi>&epsilon;</mi><mo>:</mo><mi>sin</mi><mo>&af;</mo>\
         <mi>&theta;</mi><mo>&ne;</mo><mo>&part;</mo><mi>f</mi>",
    ),
];

/// The Typst math each sample becomes, in the order of `SAMPLES`.
const TRANSLATIONS: [&str; 12] = [
    "$ frac(a + b, 2 c) $",
    "$ sqrt(x + 1) + root(3, y) $",
    "$ attach(x, t: 2) + attach(T, b: k, t: n + 1) + attach(C, b: 6, t: 14, bl: 1, tl: 2) $",
    "$ attach(limits(sum), b: i = 1, t: n) attach(i, t: 2) $",
    "$ attach(integral, b: 0, t: infinity) attach(e, t: - attach(x, t: 2)) #h(0.1667em) \
     upright(d) x $",
    "$ A = mat(delim: \"(\", a, b; c, d) $",
    "$ lr(| x |) = cases(x & #box(text(font: gd-math-font, \"if\u{a0}\")) x gt.eq 0, - x & \
     #box(text(font: gd-math-font, \"otherwise\"))) $",
    "$ hat(x) + arrow(v) + underbrace(a + b) $",
    "$ binom(n, k) $",
    "$ attach(lr(\\( a + b \\)), t: 2) & = attach(a, t: 2) + 2 a b \\ & gt.eq 0 $",
    "$ frac(#box(text(font: gd-math-font, \"favourable cases\")), #box(text(font: \
     gd-math-font, \"all cases, 100%\"))) $",
    "$ alpha lt.eq Omega arrow.r bb(R) \\, forall epsilon \\: sin theta eq.not partial f $",
];

fn formula(inner: &str, label: &str) -> String {
    let label = if label.is_empty() { String::new() } else { format!("   :label: {label}\n") };
    format!(
        ".. math::\n   :notation: mathml\n{label}\n   <math xmlns=\"http://www.w3.org/1998/Math/\
         MathML\">{inner}</math>\n\n"
    )
}

/// The project's page: every sample, the first one labelled and referenced, text that
/// would be Typst code if it were not escaped, and one element outside MathML Core.
fn page() -> String {
    let mut text = String::from("Formulas\n========\n\n");
    for (i, (name, inner)) in SAMPLES.iter().enumerate() {
        text.push_str(&formula(inner, if i == 0 { "first" } else { "" }));
        text.push_str(&format!("That was the {name}.\n\n"));
    }
    text.push_str("See :eq:`first`.\n\n");
    text.push_str(&formula("<mtext>\") #panic() //</mtext><mi>#panic()</mi><mo>$</mo>", ""));
    text.push_str(&formula("<menclose notation=\"box\"><mi>x</mi></menclose>", ""));
    text
}

/// A PDF build typesets authored MathML: the numbered formula keeps its number and its
/// `:eq:` link, hostile text stays text, and a formula outside MathML Core shows its
/// source with a warning that names the element. `--untrusted` changes nothing, because
/// the translation is generated markup, not raw Typst.
#[test]
fn authored_mathml_is_typeset_in_a_pdf() {
    let dir = project("mathml-pdf");
    dir.write("index.rst", &page());
    for confinement in [&[][..], &["--untrusted"][..]] {
        let r = build(&dir.path, 0, &[&["pdf"], confinement].concat());
        let source = dir.read("_build/pdf/book-en.typ");
        for translation in TRANSLATIONS {
            has(&source, translation);
        }
        has(&source, "<index-equation-first>#gd-equation(\"1\")[\n");
        has(&source, "#gd-link(\"index-equation-first\")[\\(1\\)]");
        has(&source, "\"\\\") #panic() //\")) \"#panic()\" \\$ $");
        has(&source, "#raw(block: true, lang: \"xml\", \"<math");
        let warnings = both(&r);
        has(&warnings, "typst.math.mathml");
        has(&warnings, "I cannot translate <menclose> in this MathML formula");
        assert_eq!(warnings.matches("typst.math.mathml").count(), 1, "{warnings}");
        // The embedded compiler accepted every translation.
        assert!(dir.exists("_build/pdf/demo-en.pdf"));
    }
}

/// The reStructuredText writer keeps the notation, so a document converted to
/// reStructuredText and read again gives the same Typst math.
#[test]
fn authored_mathml_survives_the_restructuredtext_round_trip() {
    let dir = guidedog_tests::Scratch::new("mathml-round-trip");
    dir.write("page.rst", &formula(SAMPLES[0].1, "first"));
    let to_rst = ["convert", "page.rst", "--from", "sphinx", "--to", "rst", "-o", "again.rst"];
    guidedog_tests::guidedog(&dir.path, 0, &to_rst);
    has(&dir.read("again.rst"), "   :notation: mathml\n");
    for page in ["page.rst", "again.rst"] {
        let to_pdf = ["convert", page, "--from", "sphinx", "-o", "p.pdf", "--emit-typst", "p.typ"];
        let r = guidedog_tests::guidedog(&dir.path, 0, &[&to_pdf[..], &["--force"]].concat());
        lacks(&r.err, "typst.math.mathml");
        has(&dir.read("p.typ"), TRANSLATIONS[0]);
    }
}
