//! The MathML translator: translations, failures, bounds, and Typst validity.

use guidedoc::{Fail, NodeKind as K, Policy, Severity, Status};

use super::{Tb, expect_compiles, render_policy};
use crate::mathml::{ENTITIES, OPERATOR_NAMES, SYMBOLS};
use crate::{MAX_MATHML_ELEMENTS, MAX_MATHML_NESTING, PREAMBLE, translate_mathml};

/// The inside of a `math` element and the Typst math it becomes.
const MATHML_CASES: [(&str, &str); 44] = [
    // Tokens.
    ("<mi>x</mi><mi>y</mi>", "x y"),
    (
        "<mi>&alpha;</mi><mo>&le;</mo><mi>&#x3A9;</mi><mo>&#8594;</mo><mi>ℝ</mi>",
        "alpha lt.eq Omega arrow.r RR",
    ),
    ("<mi>sin</mi><mo>&af;</mo><mi>x</mi>", "sin x"),
    ("<mi>speed</mi>", "\"speed\""),
    ("<mi mathvariant=\"normal\">d</mi><mi mathvariant=\"bold\">v</mi>", "upright(d) bold(v)"),
    (
        "<mi mathvariant=\"double-struck\">R</mi><mi mathvariant=\"script\">L</mi>\
         <mi mathvariant=\"fraktur\">g</mi><mi mathvariant=\"sans-serif\">s</mi>\
         <mi mathvariant=\"monospace\">m</mi><mi mathvariant=\"italic\">ab</mi>\
         <mi mathvariant=\"bold-italic\">b</mi>",
        "bb(R) cal(L) frak(g) sans(s) mono(m) italic(\"ab\") bold(italic(b))",
    ),
    ("<mn>3.14</mn><mn>1,000</mn><mn>0x1F</mn>", "3.14 \"1,000\" \"0x1F\""),
    (
        "<mo>lim</mo><mo>:=</mo><mo>arg&nbsp;max</mo><mo>argmax</mo>",
        "lim colon.eq a r g m a x op(\"argmax\")",
    ),
    ("<mi>a</mi><mo>&it;</mo><mi>b</mi><mo>&minus;</mo><mn>1</mn>", "a b - 1"),
    ("<mtext> for all  x </mtext>", "#box(text(font: gd-math-font, \"for all x\"))"),
    (
        "<mtext mathvariant=\"bold\">b</mtext>",
        "#box(text(font: gd-math-font, weight: \"bold\", \"b\"))",
    ),
    ("<ms>a\"b</ms>", "#box(text(font: gd-math-font, \"\\\"a\\\"b\\\"\"))"),
    (
        "<mi>a</mi><mspace width=\"1em\"/><mspace width=\"2ex\"/><mspace width=\"x\"/><mi>b</mi>",
        "a #h(1.0000em) #h(1.0000em) b",
    ),
    // Rows and fences.
    ("<mrow><mo>(</mo><mi>a</mi><mo>,</mo><mi>b</mi><mo>]</mo></mrow>", "lr(\\( a \\, b \\])"),
    ("<mo>(</mo><mi>a</mi><mo>)</mo><mo>(</mo><mi>b</mi><mo>)</mo>", "lr(\\( a \\) \\( b \\))"),
    (
        "<mrow><mo stretchy=\"false\">(</mo><mi>a</mi><mo stretchy=\"false\">)</mo></mrow>",
        "\\( a \\)",
    ),
    ("<mo>|</mo><mi>x</mi><mo>|</mo><mo>=</mo><mn>1</mn>", "lr(| x |) = 1"),
    ("<mrow><mo>&#x2016;</mo><mi>v</mi><mo>&#x2016;</mo></mrow>", "lr(\\‖ v \\‖)"),
    ("<mi>a</mi><mo>|</mo><mi>b</mi><mo>|</mo><mi>c</mi><mo>|</mo><mi>d</mi>", "a | b | c | d"),
    ("<mrow><mo>&lang;</mo><mi>x</mi><mo>&rang;</mo></mrow>", "lr(chevron.l x chevron.r)"),
    // Fractions and roots.
    ("<mfrac><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow><mn>2</mn></mfrac>", "frac(a + b, 2)"),
    ("<mfrac><mrow/><mn>2</mn></mfrac>", "frac(\"\", 2)"),
    (
        "<mrow><mo>(</mo><mfrac linethickness=\"0\"><mi>n</mi><mi>k</mi></mfrac><mo>)</mo></mrow>",
        "binom(n, k)",
    ),
    ("<mfrac linethickness=\"0px\"><mi>n</mi><mi>k</mi></mfrac>", "mat(delim: #none, n; k)"),
    (
        "<msqrt><mi>x</mi><mo>+</mo><mn>1</mn></msqrt><mroot><mi>y</mi><mn>3</mn></mroot>",
        "sqrt(x + 1) root(3, y)",
    ),
    // Scripts, limits and accents.
    (
        "<msup><mi>x</mi><mn>2</mn></msup><msub><mi>a</mi><mrow><mi>i</mi><mi>j</mi></mrow>\
         </msub>",
        "attach(x, t: 2) attach(a, b: i j)",
    ),
    (
        "<msubsup><mo>&int;</mo><mn>0</mn><mi>&infin;</mi></msubsup>",
        "attach(integral, b: 0, t: infinity)",
    ),
    (
        "<munderover><mo>&sum;</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mi>n</mi>\
         </munderover>",
        "attach(limits(sum), b: i = 1, t: n)",
    ),
    (
        "<munder><mo>lim</mo><mrow><mi>x</mi><mo>&rarr;</mo><mn>0</mn></mrow></munder>",
        "attach(limits(lim), b: x arrow.r 0)",
    ),
    ("<mover><mi>x</mi><mo>=</mo></mover>", "attach(limits(x), t: =)"),
    (
        "<mover accent=\"true\"><mi>x</mi><mo>^</mo></mover><mover><mi>v</mi><mo>&rarr;\
         </mo></mover>\
         <mover accent=\"false\"><mi>y</mi><mo>~</mo></mover>",
        "hat(x) arrow(v) attach(limits(y), t: \\~)",
    ),
    ("<mover><mrow><mi>a</mi><mi>b</mi></mrow><mo>&OverBar;</mo></mover>", "overline(a b)"),
    ("<munder accentunder=\"true\"><mi>x</mi><mo>&#x23DF;</mo></munder>", "underbrace(x)"),
    (
        "<mmultiscripts><mi>C</mi><mn>6</mn><mn>14</mn><mprescripts/><mrow/><mn>2</mn>\
         </mmultiscripts>",
        "attach(C, b: 6, t: 14, tl: 2)",
    ),
    // Tables.
    (
        "<mrow><mo>[</mo><mtable><mtr><mtd><mi>a</mi></mtd><mtd/></mtr>\
         <mtr><mtd><mi>c</mi></mtd><mtd><mi>d</mi></mtd></mtr></mtable><mo>]</mo></mrow>",
        "mat(delim: \"[\", a, \"\"; c, d)",
    ),
    (
        "<mi>M</mi><mo>=</mo><mtable><mtr><mtd><mn>1</mn></mtd></mtr></mtable>",
        "M = mat(delim: #none, 1)",
    ),
    (
        "<mi>f</mi><mo>=</mo><mrow><mo>{</mo><mtable><mtr><mtd><mn>1</mn></mtd><mtd><mi>x\
         </mi></mtd></mtr>\
         <mtr><mtd><mn>0</mn></mtd></mtr></mtable></mrow>",
        "f = cases(1 & x, 0)",
    ),
    (
        "<mtable><mtr><mtd><mi>a</mi></mtd><mtd><mo>=</mo><mi>b</mi></mtd></mtr>\
         <mtr><mtd/><mtd><mo>=</mo><mi>c</mi></mtd></mtr></mtable>",
        "a & = b \\ & = c",
    ),
    // Wrappers.
    ("<mstyle displaystyle=\"true\" mathvariant=\"bold\"><mi>x</mi></mstyle>", "display(bold(x))"),
    (
        "<mpadded width=\"+1em\"><mi>x</mi></mpadded><merror><mtext>e</mtext></merror>",
        "x #box(text(font: gd-math-font, \"e\"))",
    ),
    ("<mphantom><mi>x</mi></mphantom><mi>y</mi>", "#hide($x$) y"),
    (
        "<semantics><mrow><mi>x</mi></mrow><annotation encoding=\"application/x-tex\">x\
         </annotation>\
         <annotation-xml encoding=\"MathML-Content\"><ci>x</ci></annotation-xml></semantics>",
        "x",
    ),
    ("<mi style=\"color:red\" id=\"a\" href=\"x\">x</mi><!-- c -->", "x"),
    ("", ""),
];

/// The inside of a `math` element and the construct the translator names.
const MATHML_FAILURES: [(&str, &str); 8] = [
    ("<menclose notation=\"box\"><mi>x</mi></menclose>", "<menclose>"),
    ("<mi id=\"a\">x</mi><mfenced><mi>x</mi></mfenced>", "<mfenced>"),
    ("<mi>&bogus;</mi>", "the character reference &bogus;"),
    ("<mfrac><mi>a</mi></mfrac>", "an <mfrac> that does not have two parts"),
    ("<mroot><mi>a</mi></mroot>", "an <mroot> that does not have two parts"),
    ("<msub><mi>a</mi></msub>", "an <msub> with the wrong number of parts"),
    (
        "<munderover><mi>a</mi><mi>b</mi></munderover>",
        "an <munderover> with the wrong number of parts",
    ),
    ("<mi>x", "a tag that is not closed or is closed out of order"),
];

fn translate(inner: &str) -> Result<String, String> {
    let mut out = String::from("kept ");
    let mathml = format!("<math xmlns=\"http://www.w3.org/1998/Math/MathML\">{inner}</math>");
    match translate_mathml(&mathml, &mut out) {
        Ok(_) => Ok(out.split_off(5)),
        Err(e) => {
            assert_eq!(out, "kept ", "a failure leaves the output as it was");
            Err(e.construct)
        }
    }
}

#[test]
fn mathml_translates_to_typst_math() {
    for (mathml, typst) in MATHML_CASES {
        assert_eq!(translate(mathml).as_deref(), Ok(typst), "{mathml}");
    }
    let mut out = String::new();
    let block = translate_mathml("<math display=\"block\"><mi>x</mi></math>", &mut out);
    assert_eq!(block.map(|m| m.block), Ok(true));
    assert_eq!(translate_mathml("<math/>", &mut out).map(|m| m.block), Ok(false));
}

#[test]
fn mathml_failures_name_the_construct() {
    for (mathml, construct) in MATHML_FAILURES {
        assert_eq!(translate(mathml), Err(construct.to_string()), "{mathml}");
    }
    let mut out = String::new();
    let error = translate_mathml("<mi>x</mi>", &mut out).expect_err("no root");
    assert_eq!(error.construct, "text that is not one <math> element");
    let message = "cannot translate text that is not one <math> element in this MathML formula";
    assert_eq!(error.to_string(), format!("{message} to Typst math"));
}

#[test]
fn mathml_size_depth_and_element_count_are_bounded() {
    let deep = format!("{}{}", "<mrow>".repeat(60), "</mrow>".repeat(60));
    let expected = format!("nesting deeper than {MAX_MATHML_NESTING} elements");
    assert_eq!(translate(&deep), Err(expected));
    let level = format!("{}<mi>x</mi>{}", "<mrow>".repeat(46), "</mrow>".repeat(46));
    assert_eq!(translate(&level).as_deref(), Ok("x"));
    let many = "<mi>x</mi>".repeat(MAX_MATHML_ELEMENTS);
    let expected = format!("more than {MAX_MATHML_ELEMENTS} elements");
    assert_eq!(translate(&many), Err(expected));
    let long = format!("<mtext>{}</mtext>", "x".repeat(guidedoc::MAX_MATHML_BYTES));
    assert_eq!(translate(&long), Err("a formula longer than 256 KiB".to_string()));
}

/// Text that would be Typst code if it were written as it is.
const HOSTILE: [&str; 10] = [
    "#read(\"/etc/passwd\")",
    "$ #panic() $",
    "\") #panic() //",
    "x) #panic(",
    "a, b; c: d",
    "\\u{41}",
    "sum_(i)^n",
    "..args",
    "/* */ // _ ^ & \\ ` ~ @ < > [ ] { }",
    "√x ′ '' \u{7}\u{0}",
];

// No text of a formula becomes Typst code: outside string literals the output holds
// only single letters, digits, escaped characters, and the names and calls the
// translator writes itself.
#[test]
fn mathml_text_never_becomes_typst_code() {
    for text in HOSTILE {
        let text = text.replace('&', "&amp;").replace('<', "&lt;");
        for element in ["mi", "mn", "mo", "mtext", "ms"] {
            let typst = translate(&format!("<{element}>{text}</{element}>"))
                .unwrap_or_else(|e| panic!("{element} {text}: {e}"));
            assert_generated_only(&typst, element);
        }
    }
    let attributes = "<mspace width=\"1em)#panic(\"/><mi mathvariant=\"bold)#panic(\">x</mi>";
    assert_eq!(translate(attributes).as_deref(), Ok("x"));
}

/// Asserts that `typst`, outside its string literals, has no `#`, `$`, comma, colon,
/// semicolon or bracket that is not escaped or written by the translator, and no two
/// letters side by side that the translator's tables do not hold.
fn assert_generated_only(typst: &str, element: &str) {
    let own = ["#box(text(font: gd-math-font, ", "op(", "))", ")", "prime", "space", "dots.h"];
    let mut rest = typst;
    let mut word = 0;
    while let Some(c) = rest.chars().next() {
        if let Some(part) = own.iter().find(|part| rest.starts_with(**part)) {
            rest = &rest[part.len()..];
            word = 0;
        } else if c == '\\' {
            let escaped = rest[1..].chars().next().expect("a character after the backslash");
            assert!(!escaped.is_alphanumeric(), "{element}: \\{escaped} in {typst}");
            rest = &rest[1 + escaped.len_utf8()..];
            word = 0;
        } else if c == '"' {
            let mut end = 1;
            let bytes = rest.as_bytes();
            while bytes[end] != b'"' {
                end += if bytes[end] == b'\\' { 2 } else { 1 };
            }
            rest = &rest[end + 1..];
            word = 0;
        } else {
            word = if c.is_alphabetic() { word + 1 } else { 0 };
            assert!(word < 2, "{element}: an identifier in {typst}");
            assert!(
                c.is_alphanumeric() || " +-=<>!?|.".contains(c),
                "{element}: {c:?} in {typst}"
            );
            rest = &rest[c.len_utf8()..];
        }
    }
}

#[test]
fn the_tables_are_sound() {
    assert!(ENTITIES.is_sorted_by(|a, b| a.0 < b.0), "entities are sorted and distinct");
    let mut symbols: Vec<char> = SYMBOLS.iter().map(|s| s.0).collect();
    symbols.sort_unstable();
    symbols.dedup();
    assert_eq!(symbols.len(), SYMBOLS.len(), "every character has one name");
    for (c, name) in SYMBOLS {
        let plain = name.bytes().all(|b| b.is_ascii_alphabetic() || b == b'.') || name == "-";
        assert!(plain, "{c}: {name}");
    }
}

/// Every expected translation, table name and operator name is valid Typst.
#[test]
fn mathml_output_compiles() {
    let mut source = String::from(PREAMBLE);
    source.push_str("\n#context { let gd-math-font = (\"New Computer Modern\",);\n");
    for (mathml, _) in MATHML_CASES {
        let typst = translate(mathml).expect("translates");
        source.push_str(&format!("$ {typst} $\n\n"));
    }
    for (_, name) in SYMBOLS {
        source.push_str(&format!("$ {name} $\n"));
    }
    for name in OPERATOR_NAMES {
        source.push_str(&format!("$ {name} $\n"));
    }
    source.push_str("}\n");
    expect_compiles("mathml", &source, false);
}

fn build_mathml(t: &mut Tb<'_, '_>) -> Result<(), Fail> {
    let good = t.leaf(K::MathBlock, "<math><mfrac><mi>a</mi><mi>b</mi></mfrac></math>", 0)?;
    t.custom(good, guidedoc::MATH_NOTATION_ATTRIBUTE, "mathml")?;
    t.custom(good, "number", "7")?;
    let bad = t.leaf(K::MathBlock, "<math><menclose><mi>a</mi></menclose></math>", 0)?;
    t.custom(bad, guidedoc::MATH_NOTATION_ATTRIBUTE, "mathml")
}

// A MathML block is typeset and numbered as a LaTeX one is. One the translator refuses
// shows its source with a warning that names the element, under every policy: the
// output is generated markup and the source a string literal, so neither is raw Typst.
#[test]
fn mathml_blocks_are_typeset_under_every_policy() {
    for policy in [Policy::default(), guidedoc::trusted_policy()] {
        let r = render_policy(build_mathml, policy);
        assert_eq!(r.status, Status::Ok);
        r.expect("#gd-equation(\"7\")[\n#context { let gd-math-font = ");
        r.expect("$ frac(a, b) $ }\n]");
        r.expect(
            "#raw(block: true, lang: \"xml\", \"<math><menclose><mi>a</mi></menclose></math>\")",
        );
        assert!(r.has_code("typst.math.mathml", Severity::Warning));
        // The construct is the message's argument; the integration tests read it.
        let report = r.reports.iter().find(|d| d.code == "typst.math.mathml").expect("reported");
        assert_ne!(report.args[0], guidedoc::Arg::None);
        assert_eq!(r.reports.len(), 1);
    }
}
