//! Typesetting a batch of formulas: LaTeX to Typst math with the adapter, one compiler
//! run for the batch, and one filtered `math` element per formula.

use std::path::Path;

use guidedoc_html::{MathMlOptions, filter_mathml};
use guidedoc_typst::translate_math;

use crate::{Refusal, Typeset};

/// The style rules Typst's MathML needs, as Typst 0.15.1 writes them in a page's head.
/// They align multi-line equations and cases and tune spacing; without them a formula
/// is still readable. A fragment's host adds them to its style sheet.
pub const MATHML_CSS: &str = include_str!("mathml.css");

/// What stands before the formulas in the generated Typst text. The adapter writes text
/// and boxes for a page; here they become plain math, which MathML can hold.
const PREAMBLE: &str = include_str!("preamble.typ");

/// The most formulas one compiler run takes.
const MAX_BATCH: usize = 1024;
/// The most Typst text one compiler run takes, in bytes.
const MAX_BATCH_BYTES: usize = 1 << 20;
/// How often a batch is compiled again without the formulas the compiler refused.
const MAX_ROUNDS: usize = 12;

/// One formula on its way through the compiler.
pub(crate) struct Formula<'a> {
    latex: &'a str,
    display: bool,
    typst: String,
    result: Option<Typeset>,
}

impl<'a> Formula<'a> {
    pub(crate) fn new(latex: &'a str, display: bool) -> Formula<'a> {
        Formula { latex, display, typst: String::new(), result: None }
    }

    /// The formula's LaTeX, whether it is displayed, and its MathML or the refusal.
    pub(crate) fn finish(self) -> (&'a str, bool, Typeset) {
        let failed = || Refusal::Failed("the compiler wrote no formula".to_string());
        (self.latex, self.display, self.result.unwrap_or_else(|| Err(failed())))
    }
}

/// Typesets every formula: afterwards each has a result.
pub(crate) fn typeset(formulas: &mut [Formula<'_>]) {
    for formula in formulas.iter_mut() {
        translate(formula);
    }
    let mut pending: Vec<usize> =
        (0..formulas.len()).filter(|&i| formulas[i].result.is_none()).collect();
    if !gd_typst::EMBEDDED {
        for &i in &pending {
            formulas[i].result = Some(Err(Refusal::Unavailable));
        }
        return;
    }
    while !pending.is_empty() {
        let mut bytes = 0;
        let mut take = 0;
        while take < pending.len() && take < MAX_BATCH && bytes < MAX_BATCH_BYTES {
            bytes += formulas[pending[take]].typst.len();
            take += 1;
        }
        let rest = pending.split_off(take);
        compile_batch(formulas, pending);
        pending = rest;
    }
}

/// Translates one formula's LaTeX to Typst math, or records why it cannot be.
fn translate(formula: &mut Formula<'_>) {
    let latex = joined_parts(formula.latex);
    let mut typst = String::with_capacity(latex.len() * 2);
    if let Err(error) = translate_math(&latex, &mut typst) {
        formula.result = Some(Err(Refusal::Unsupported(error.construct.to_string())));
        return;
    }
    // The adapter draws `\boxed` as a box with a stroke, which MathML Core lacks.
    if typst.contains("#box(stroke:") {
        formula.result = Some(Err(Refusal::Lossy("\\boxed draws a frame".to_string())));
        return;
    }
    // One formula stands on one line, so the compiler's line names the formula.
    formula.typst = typst.replace(['\n', '\r'], " ");
}

/// Displayed math in parts separated by blank lines is one formula of several lines,
/// as Sphinx reads it.
fn joined_parts(latex: &str) -> std::borrow::Cow<'_, str> {
    if !latex.contains("\n\n") {
        return latex.into();
    }
    let parts: Vec<&str> = latex.split("\n\n").filter(|part| !part.trim().is_empty()).collect();
    parts.join(" \\\\ ").into()
}

/// Compiles the formulas `batch` names. A formula the compiler refuses is taken out
/// and the rest compiled again, a bounded number of times.
fn compile_batch(formulas: &mut [Formula<'_>], mut batch: Vec<usize>) {
    for _ in 0..MAX_ROUNDS {
        if batch.is_empty() {
            return;
        }
        let source = batch_source(formulas, &batch);
        match compile(&source) {
            Ok((html, warnings)) => {
                for (line, message) in warnings {
                    if let Some(&i) = batch.get(line) {
                        formulas[i].result = Some(Err(Refusal::Lossy(message)));
                    }
                }
                extract(formulas, &batch, &html);
                return;
            }
            Err(Failure::Lines(lines)) => {
                for (line, message) in lines {
                    if let Some(&i) = batch.get(line) {
                        formulas[i].result = Some(Err(Refusal::Failed(message)));
                    }
                }
                let before = batch.len();
                batch.retain(|&i| formulas[i].result.is_none());
                if batch.len() == before {
                    fail_all(formulas, &batch, "the compiler failed outside a formula");
                    return;
                }
            }
            Err(Failure::Whole(message)) => {
                fail_all(formulas, &batch, &message);
                return;
            }
        }
    }
    fail_all(formulas, &batch, "too many formulas of the document failed");
}

fn fail_all(formulas: &mut [Formula<'_>], batch: &[usize], message: &str) {
    for &i in batch {
        formulas[i].result = Some(Err(Refusal::Failed(message.to_string())));
    }
}

/// The Typst text of a batch: the preamble, then one line per formula, each formula
/// in a `div` that carries its place in the batch.
fn batch_source(formulas: &[Formula<'_>], batch: &[usize]) -> String {
    let size: usize = batch.iter().map(|&i| formulas[i].typst.len() + 48).sum();
    let mut source = String::with_capacity(PREAMBLE.len() + size);
    source.push_str(PREAMBLE);
    for (place, &i) in batch.iter().enumerate() {
        let formula = &formulas[i];
        let pad = if formula.display { " " } else { "" };
        source.push_str(&format!("#html.elem(\"div\", attrs: (id: \"f{place}\"))[${pad}"));
        source.push_str(&formula.typst);
        source.push_str(pad);
        source.push_str("$]\n");
    }
    source
}

/// Why a compile gave no page.
enum Failure {
    /// Errors in formulas: the place in the batch and the message of each.
    Lines(Vec<(usize, String)>),
    /// A failure no formula explains.
    Whole(String),
}

/// A diagnostic's place in the batch, when it is on a formula's line.
fn place_of(diagnostic: &gd_typst::Diagnostic) -> Option<usize> {
    let first = PREAMBLE.lines().count() + 1;
    (diagnostic.stdin && diagnostic.line >= first).then(|| diagnostic.line - first)
}

/// Compiles the batch to an HTML page. The compile reads no file and no package: the
/// text is generated, and every file lookup is refused. Returns the page and the
/// warnings on formula lines.
fn compile(source: &str) -> Result<(String, Vec<(usize, String)>), Failure> {
    let refuse = |_: &Path| false;
    let options = gd_typst::Options {
        target: gd_typst::Target::Html,
        offline: true,
        embedded_fonts_only: true,
        observe_file: Some(&refuse),
        ..gd_typst::Options::default()
    };
    let on_lines = |diagnostics: &[gd_typst::Diagnostic], severity| {
        diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .filter_map(|d| place_of(d).map(|place| (place, d.message.clone())))
            .collect::<Vec<_>>()
    };
    match gd_typst::compile_bytes(source, Path::new("."), &options) {
        Ok((bytes, compiled)) => {
            let html = String::from_utf8(bytes)
                .map_err(|_| Failure::Whole("the compiler wrote invalid UTF-8".to_string()))?;
            Ok((html, on_lines(&compiled.diagnostics, gd_typst::Severity::Warning)))
        }
        Err(gd_typst::Error::Compile { diagnostics, .. }) => {
            let lines = on_lines(&diagnostics, gd_typst::Severity::Error);
            if lines.is_empty() {
                let first = diagnostics.first().map(|d| d.message.clone());
                Err(Failure::Whole(first.unwrap_or_else(|| "the compile failed".to_string())))
            } else {
                Err(Failure::Lines(lines))
            }
        }
        Err(error) => Err(Failure::Whole(error.message().into_owned())),
    }
}

/// Takes each formula's `math` element out of the page and filters it.
fn extract(formulas: &mut [Formula<'_>], batch: &[usize], html: &str) {
    let mut rest = html;
    for (place, &i) in batch.iter().enumerate() {
        let open = format!("<div id=\"f{place}\">");
        let Some(at) = rest.find(&open) else { break };
        let body = &rest[at + open.len()..];
        let next = format!("</div><div id=\"f{}\">", place + 1);
        let end = body.find(&next).or_else(|| body.rfind("</div>")).unwrap_or(body.len());
        rest = &body[end..];
        if formulas[i].result.is_some() {
            continue; // a warning already refused it.
        }
        formulas[i].result = Some(filtered(&body[..end], formulas[i].display));
    }
}

/// One formula's MathML as the filter passes it. Typst's `style` attributes
/// (`math-shift`) are dropped; anything else outside MathML Core refuses the formula.
fn filtered(mathml: &str, display: bool) -> Typeset {
    let options = MathMlOptions {
        block: Some(display),
        quiet_attributes: &["style"],
        ..MathMlOptions::default()
    };
    let mut out = String::with_capacity(mathml.len());
    match filter_mathml(mathml, &options, &mut out) {
        Ok(dropped) if dropped.count == 0 => Ok(out.into_boxed_str()),
        Ok(dropped) => Err(Refusal::Failed(format!("{} is outside MathML Core", dropped.first))),
        Err(error) => Err(Refusal::Failed(error.to_string())),
    }
}
