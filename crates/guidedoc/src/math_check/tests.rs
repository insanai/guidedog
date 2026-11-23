use super::*;

fn inline(latex: &str) -> Option<MathFault> {
    check_math(latex, &MathCheck::default())
}

/// The code, the offending text and the hint of the fault of inline `latex`.
#[track_caller]
fn found(latex: &str) -> (&'static str, String, String) {
    let fault = inline(latex).unwrap_or_else(|| panic!("no fault in {latex:?}"));
    (fault.message.code, latex[fault.start..fault.end].to_string(), fault.hint)
}

#[test]
fn the_name_tables_are_sorted_for_binary_search() {
    assert!(COMMANDS.is_sorted() && ENVIRONMENTS.is_sorted());
    assert!(is_math_command("frac") && is_math_command("Longleftrightarrow"));
    assert!(is_math_environment("align*") && !is_math_environment("algin"));
}

#[test]
fn an_unknown_command_gets_the_closest_known_one() {
    let (code, text, hint) = found("x = \\fraction{a}{b}");
    assert_eq!((code, text.as_str()), ("math.command.unknown", "\\fraction"));
    assert_eq!(hint, "Did you mean `\\frac`?");
    assert_eq!(found("\\aplha + 1").2, "Did you mean `\\alpha`?");
    assert_eq!(found("\\sqr{2}").2, "Did you mean `\\sqrt`?");
    assert_eq!(found("\\mathbff{x}").2, "Did you mean `\\mathbf`?");
    let (code, _, hint) = found("\\zzzqqq");
    assert_eq!(code, "math.command.unknown");
    assert!(hint.starts_with("Check the spelling."), "{hint}");
    let fault = inline("a \\foo b").expect("a fault");
    assert_eq!((fault.start, fault.end, fault.subject.as_str()), (2, 6, "\\foo"));
}

#[test]
fn an_unknown_environment_gets_the_closest_known_one() {
    let (code, text, hint) = found("\\begin{algin} a \\end{algin}");
    assert_eq!((code, text.as_str()), ("math.environment.unknown", "\\begin{algin}"));
    assert_eq!(hint, "Did you mean `align`?");
    assert_eq!(found("\\begin{pmatrx} a \\end{pmatrx}").2, "Did you mean `pmatrix`?");
}

#[test]
fn braces_must_balance() {
    let (code, text, hint) = found("\\frac{a}{b + {c}");
    assert_eq!((code, text.as_str()), ("math.brace.unbalanced", "{"));
    assert!(hint.starts_with("Add the missing `}`"), "{hint}");
    assert_eq!(inline("\\frac{a}{b + {c}").expect("a fault").start, 8);
    let fault = inline("a + b}").expect("a fault");
    assert_eq!((fault.message.code, fault.start), ("math.brace.unbalanced", 5));
    assert_eq!(fault.subject, "a `}` that closes nothing");
    assert_eq!(inline("\\{ a \\}"), None);
}

#[test]
fn environments_must_close_by_their_own_name() {
    let (code, text, hint) = found("\\begin{pmatrix} a & b");
    assert_eq!((code, text.as_str()), ("math.environment.unclosed", "\\begin"));
    assert_eq!(hint, "Add `\\end{pmatrix}` where the environment ends.");
    let fault = inline("\\begin{pmatrix} a \\end{bmatrix}").expect("a fault");
    assert_eq!(fault.message.code, "math.environment.mismatch");
    assert_eq!(fault.subject, "`\\end{bmatrix}` closes `\\begin{pmatrix}`.");
    assert_eq!(fault.hint, "Write `\\end{pmatrix}`, or begin the environment as `bmatrix`.");
    let fault = inline("a \\end{cases}").expect("a fault");
    assert_eq!(fault.message.code, "math.environment.unclosed");
    assert_eq!(fault.subject, "`\\end{cases}` without `\\begin{cases}`");
}

#[test]
fn left_and_right_come_in_pairs() {
    let (code, text, hint) = found("\\left( \\frac{a}{b}");
    assert_eq!((code, text.as_str()), ("math.delimiter.unpaired", "\\left"));
    assert_eq!(hint, "Add `\\right` with a delimiter, or `\\right.` for none.");
    let (code, text, _) = found("\\frac{a}{b} \\right)");
    assert_eq!((code, text.as_str()), ("math.delimiter.unpaired", "\\right"));
    assert_eq!(inline("\\left\\{ a \\middle| b \\right."), None);
}

#[test]
fn a_command_needs_its_arguments() {
    let (code, text, hint) = found("\\frac{a}");
    assert_eq!((code, text.as_str()), ("math.argument.missing", "\\frac"));
    assert_eq!(hint, "Write both arguments, as in `\\frac{a}{b}`.");
    assert_eq!(found("x + \\sqrt").2, "Write its argument after it, as in `\\sqrt{x}`.");
    assert_eq!(found("{\\frac{a}} + b").0, "math.argument.missing");
    for fine in ["\\frac ab", "\\frac12", "\\frac\\alpha\\beta", "\\sqrt[3]{x}", "\\hat x"] {
        assert_eq!(inline(fine), None, "{fine}");
    }
}

#[test]
fn alignment_needs_an_environment() {
    let (code, text, hint) = found("a & b");
    assert_eq!((code, text.as_str()), ("math.alignment.outside", "&"));
    assert!(hint.contains("`aligned` or `matrix`") && hint.contains("`\\&`"), "{hint}");
    assert_eq!(found("a \\\\ b").1, "\\\\");
    let display = MathCheck { line_breaks: true, ..MathCheck::default() };
    assert_eq!(check_math("a \\\\ b", &display), None);
    assert_eq!(
        check_math("a & b", &display).expect("a fault").message.code,
        "math.alignment.outside"
    );
    let wrapped = MathCheck { alignment: true, line_breaks: true, ..MathCheck::default() };
    assert_eq!(check_math("a &= b \\\\ c &= d", &wrapped), None);
    assert_eq!(inline("\\begin{matrix} a & b \\\\ c & d \\end{matrix}"), None);
    assert_eq!(inline("\\substack{a \\\\ b} + \\text{R\\&D}"), None);
}

#[test]
fn valid_latex_has_no_fault() {
    let fine = [
        "a^2 + b^2 = c^2",
        "\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}",
        "\\operatorname{foo}(x) + \\operatorname*{argmax}_x f",
        "\\sum_{i=1}^{n} \\binom{n}{i} \\mathbb{R} \\mathcal{O} \\vec{v} \\hat{x}",
        "\\left( \\frac{a}{b} \\right) \\Big| \\langle x \\rangle",
        "\\begin{cases} 1 & x > 0 \\\\ 0 & \\text{otherwise} \\end{cases}",
        "\\begin{array}{c|c} a & b \\\\ \\hline c & d \\end{array}",
        "\\color{red} x \\boxed{y} \\cancel{z} \\ce{H2O} \\xrightarrow[a]{b}",
        "\\mathrm{d}x \\; \\% \\$ \\# \\_ \\{ \\} \\| \\! \\, \\: \\ ",
        "x % a comment with } and \\nonsense\n + y",
        "\\tag{1} \\label{eq:a_b} \\eqref{eq:a_b} \\text{if } x\\in\\mathbb{Q}",
        "\\underbrace{a+b}_{n} \\overset{!}{=} \\stackrel{\\text{def}}{=} \\not\\in",
        "\\lim_{x\\to0} \\sin x \\cdot \\nabla \\times \\mathbf{E} \\partial_t \\hbar \\ell",
        "",
    ];
    for latex in fine {
        assert_eq!(inline(latex), None, "{latex}");
    }
}

#[test]
fn definitions_and_host_macros_are_known() {
    let mut checker = MathChecker::new();
    let options = MathCheck::default();
    let define = "\\newcommand{\\RR}{\\mathbb{R}} \\def\\half{\\frac12} \\RR \\half";
    assert_eq!(checker.check(define, &options), None);
    // A definition holds for the later formulas of the page, as in MathJax.
    assert_eq!(checker.check("x \\in \\RR", &options), None);
    assert_eq!(checker.check("\\DeclareMathOperator{\\sgn}{sgn} \\sgn x", &options), None);
    let environment = "\\newenvironment{mine}{\\begin{aligned}}{\\end{aligned}}";
    assert_eq!(checker.check(environment, &options), None);
    assert_eq!(checker.check("\\begin{mine} a \\end{mine}", &options), None);
    assert_eq!(inline("x \\in \\RR").expect("a fault").message.code, "math.command.unknown");
    let host = MathCheck { macros: &["RR", "bra"], ..MathCheck::default() };
    assert_eq!(check_math("\\RR", &host), None);
    // A definition body still has its names checked.
    let bad = checker.check("\\newcommand{\\x}{\\aplha}", &options).expect("a fault");
    assert_eq!(bad.hint, "Did you mean `\\alpha`?");
}

#[test]
fn the_work_is_bounded() {
    let long = "\\nonsense ".repeat(MAX_FORMULA_BYTES / 8);
    assert_eq!(inline(&long), None);
    let deep = "{".repeat(MAX_OPEN + 10);
    assert_eq!(inline(&deep), None);
    assert_eq!(suggest_math_command(&"a".repeat(MAX_NAME + 1)), None);
    assert_eq!(inline("\\é \\").map(|f| f.message.code), None);
}

#[test]
fn a_fault_is_a_warning_with_its_hint_in_the_diagnostic() {
    let fault = inline("\\fraction{a}{b}").expect("a fault");
    let d = fault.diagnostic(SourceSpan::default());
    assert_eq!((d.code, d.severity), ("math.command.unknown", Severity::Warning));
    assert_eq!(d.title, "UNKNOWN MATH COMMAND");
    assert!(d.formatted_message().contains("`\\fraction`"), "{}", d.formatted_message());
    assert_eq!(d.formatted_hint(), "Did you mean `\\frac`?");
}
