use guidedoc::{Pass, RenderOptions, Request, Severity, Source, Workspace};

use super::*;

/// What converting `text` (reStructuredText with the Sphinx layer) gave.
struct Converted {
    html: String,
    /// The code and the rendered message of each finding.
    findings: Vec<(&'static str, String)>,
}

fn convert_with(math: &MathMl, text: &str) -> Converted {
    let registry = guidedoc_defaults::registry();
    let settings = guidedoc_defaults::ReaderSettings::default();
    let prepare = |doc: Document<'_>, b: &mut Builder<'_>| math.prepare(doc, b);
    let request = Request {
        source: Source { name: "page.rst", text },
        reader: "sphinx",
        renderer: "html",
        read: settings.options("sphinx"),
        passes: &[Pass { name: "mathml", run: &prepare }],
        render: RenderOptions {
            standalone: false,
            math_writer: Some(math),
            ..RenderOptions::default()
        },
        ..Request::default()
    };
    let mut ws = Workspace::new();
    let html = guidedoc::convert(&mut ws, &registry, &request).expect("converts").to_string();
    let findings = ws
        .reports()
        .items()
        .iter()
        .map(|d| {
            assert_eq!(d.severity, Severity::Warning, "{}", d.code);
            (d.code, ws.describe(d))
        })
        .collect();
    Converted { html, findings }
}

fn convert(text: &str) -> Converted {
    convert_with(&MathMl::new(), text)
}

#[track_caller]
fn has(c: &Converted, fragment: &str) {
    assert!(c.html.contains(fragment), "missing {fragment:?} in:\n{}", c.html);
}

#[test]
fn inline_math_is_a_math_element() {
    let c = convert("Pythagoras: :math:`a^2 + b^2 = c^2`.\n");
    has(
        &c,
        "<span class=\"math\"><math><msup><mi>𝑎</mi><mn>2</mn></msup><mo>+</mo>\
         <msup><mi>𝑏</mi><mn>2</mn></msup><mo>=</mo><msup><mi>𝑐</mi><mn>2</mn></msup>\
         </math></span>",
    );
    assert!(!c.html.contains("\\("), "{}", c.html);
    assert!(c.findings.is_empty(), "{:?}", c.findings);
}

/// A build numbers equations and resolves `:eq:`; the number and the link are tested
/// with the project (tests/integration), and the number's markup with the renderer.
#[test]
fn display_math_is_a_block_in_the_usual_wrapper() {
    let c = convert(
        ".. math::\n   :label: euler\n\n   e^{i\\pi} + 1 = 0\n\n.. math::\n\n   \\frac{a}{b}\n",
    );
    has(&c, "<div id=\"equation-euler\" class=\"math\"><math display=\"block\"><msup>");
    has(&c, "<math display=\"block\"><mfrac><mi>𝑎</mi><mi>𝑏</mi></mfrac></math></div>");
    assert!(!c.html.contains("\\["), "{}", c.html);
    assert!(c.findings.is_empty(), "{:?}", c.findings);
}

#[test]
fn a_formula_the_adapter_refuses_stays_latex_with_a_finding() {
    let c = convert("Good :math:`x_1` and bad :math:`\\color{red} x`.\n");
    has(&c, "<math><msub><mi>𝑥</mi><mn>1</mn></msub></math>");
    has(&c, "<span class=\"math\">\\(\\color{red} x\\)</span>");
    assert_eq!(c.findings.len(), 1, "{:?}", c.findings);
    assert_eq!(c.findings[0].0, "mathml.math.unsupported");
    assert!(c.findings[0].1.contains("\\color"), "{}", c.findings[0].1);
}

#[test]
fn a_formula_mathml_cannot_show_stays_latex() {
    let c = convert("Boxed :math:`\\boxed{x}`, hidden :math:`a \\phantom{b} c`.\n");
    has(&c, "\\(\\boxed{x}\\)");
    has(&c, "\\(a \\phantom{b} c\\)");
    let codes: Vec<_> = c.findings.iter().map(|f| f.0).collect();
    assert_eq!(codes, ["mathml.math.lossy", "mathml.math.lossy"]);
}

#[test]
fn text_matrices_and_aligned_lines_are_mathml_core() {
    let c = convert(
        ".. math::\n\n   a &= b + \\text{if } x < 1 \\\\\n   c &= \\begin{pmatrix} 1 & 2 \\\\ 3 \
         & 4 \\end{pmatrix}\n\n.. math::\n\n   \\mathbf{F} = \\sum_{i=1}^{n} \\sqrt{x_i} \
         \\cdot \\hat{y}\n",
    );
    assert!(c.findings.is_empty(), "{:?}", c.findings);
    has(&c, "<mtable class=\"multiline-equation aligned\">");
    has(&c, "<mtext>if </mtext>");
    has(&c, "<mo>&lt;</mo>");
    has(&c, "<munderover>");
    assert!(!c.html.contains("style="), "{}", c.html);
    let mut names = Vec::new();
    for part in c.html.split('<').skip(1) {
        let name: String = part.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if name.starts_with('m') {
            names.push(name);
        }
    }
    for name in names {
        assert!(guidedoc_html::MATHML_ELEMENTS.contains(&name.as_str()), "{name}");
    }
}

#[test]
fn equal_formulas_are_typeset_once_and_kept_between_documents() {
    let math = MathMl::new();
    let registry = guidedoc_defaults::registry();
    let settings = guidedoc_defaults::ReaderSettings::default();
    let count = |text: &str| {
        let mut ws = Workspace::new();
        let seen = std::cell::Cell::new(Prepared::default());
        let prepare = |doc: Document<'_>, b: &mut Builder<'_>| {
            seen.set(math.prepare_counted(doc, b));
            PassOutcome::Unchanged
        };
        let request = Request {
            source: Source { name: "page.rst", text },
            reader: "sphinx",
            renderer: "html",
            read: settings.options("sphinx"),
            passes: &[Pass { name: "mathml", run: &prepare }],
            ..Request::default()
        };
        guidedoc::convert(&mut ws, &registry, &request).expect("converts");
        seen.get()
    };
    let first = count(":math:`x` and :math:`x` and :math:`y`.\n");
    assert_eq!(first, Prepared { formulas: 3, typeset: 2, refused: 0 });
    let second = count(":math:`y` and :math:`z`.\n");
    assert_eq!(second, Prepared { formulas: 2, typeset: 1, refused: 0 });
    assert!(math.mathml("x", false).is_some());
    assert!(math.mathml("x", true).is_none());
}

#[test]
fn limits_give_findings_and_keep_the_latex() {
    let limits = Limits { max_formula_bytes: 8, max_formulas: 1, ..Limits::default() };
    let math = MathMl::with_limits(limits);
    let c = convert_with(&math, ":math:`a`, :math:`b`, :math:`c + d + e + f`.\n");
    has(&c, "<math><mi>𝑎</mi></math>");
    has(&c, "\\(b\\)");
    has(&c, "\\(c + d + e + f\\)");
    let codes: Vec<_> = c.findings.iter().map(|f| f.0).collect();
    assert_eq!(codes, ["mathml.math.limit", "mathml.math.limit"]);
    assert!(c.findings[0].1.contains("more formulas"), "{}", c.findings[0].1);
    assert!(c.findings[1].1.contains("longer"), "{}", c.findings[1].1);
}

/// The style rules are a copy of what the pinned Typst writes; a Typst upgrade that
/// changes them fails here, so the copy and its documentation are brought up to date.
#[test]
fn the_style_rules_are_those_typst_writes() {
    let options = gd_typst::Options {
        target: gd_typst::Target::Html,
        offline: true,
        embedded_fonts_only: true,
        ..gd_typst::Options::default()
    };
    let (bytes, _) =
        gd_typst::compile_bytes("$x$\n", std::path::Path::new("."), &options).expect("compiles");
    let page = String::from_utf8(bytes).expect("UTF-8");
    let start = page.find("<style>").expect("a style element") + "<style>".len();
    let end = page.find("</style>").expect("its end");
    assert_eq!(page[start..end].trim(), MATHML_CSS.trim());
}
