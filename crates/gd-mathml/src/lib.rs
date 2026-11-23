//! gd-mathml writes formulas as MathML Core, so a page needs no script to show them.
//!
//! A formula's LaTeX goes through the bounded adapter of guidedoc-typst
//! (`translate_math`) and then through the embedded Typst compiler, whose HTML export
//! writes MathML Core. All new formulas of a document compile in one Typst run, and a
//! formula that was typeset before is taken from the cache. Every result passes the
//! MathML filter of guidedoc-html, so a page holds only the elements and attributes
//! that filter names, and no `style` attribute.
//!
//! A formula is never lost. When the adapter cannot translate it, when MathML Core
//! cannot show part of it, or when a limit is reached, the renderer writes its LaTeX in
//! the usual markup, and the pass reports a finding at the formula.
//!
//! ```no_run
//! use guidedoc::{Pass, RenderOptions, Request};
//! let math = gd_mathml::MathMl::new();
//! let prepare = |doc: guidedoc::Document<'_>, b: &mut guidedoc::Builder<'_>| {
//!     math.prepare(doc, b)
//! };
//! let request = Request {
//!     passes: &[Pass { name: "mathml", run: &prepare }],
//!     render: RenderOptions { math_writer: Some(&math), ..RenderOptions::default() },
//!     ..Request::default()
//! };
//! ```
//!
//! One [`MathMl`] serves one thread, like a workspace; it is not shared.

#![forbid(unsafe_code)]

mod messages;
mod typeset;

use std::cell::RefCell;
use std::collections::HashMap;

use guidedoc::{
    Arg, Builder, Document, Emitter, MathWriter, Message, NodeId, NodeKind, PassOutcome, arg_text,
};

pub use messages::{MATH_FAILED, MATH_LIMIT, MATH_LOSSY, MATH_UNAVAILABLE, MATH_UNSUPPORTED};
pub use typeset::MATHML_CSS;

/// The bounds of a [`MathMl`]. `Default` gives the documented values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The longest formula typeset, in bytes of LaTeX (default 16 KiB).
    pub max_formula_bytes: usize,
    /// How many distinct new formulas one document may add (default 8,192).
    pub max_formulas: usize,
    /// The bytes the cache may hold; it is emptied before a document that finds it
    /// fuller (default 32 MiB).
    pub max_cache_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_formula_bytes: 16 << 10, max_formulas: 8192, max_cache_bytes: 32 << 20 }
    }
}

/// Why a formula stays LaTeX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The adapter met a construct it does not translate.
    Unsupported(String),
    /// MathML Core cannot show a part; Typst said which.
    Lossy(String),
    /// The compiler or the filter refused the formula.
    Failed(String),
    /// A limit was reached.
    Limit(&'static str),
    /// No embedded compiler.
    Unavailable,
}

impl Refusal {
    fn finding(&self) -> (Message, Arg) {
        match self {
            Refusal::Unsupported(what) => (MATH_UNSUPPORTED, arg_text(what)),
            Refusal::Lossy(what) => (MATH_LOSSY, arg_text(what)),
            Refusal::Failed(what) => (MATH_FAILED, arg_text(what)),
            Refusal::Limit(what) => (MATH_LIMIT, arg_text(what)),
            Refusal::Unavailable => (MATH_UNAVAILABLE, Arg::None),
        }
    }
}

/// A formula as the cache knows it: its MathML, or why it has none.
type Typeset = Result<Box<str>, Refusal>;

#[derive(Debug, Default)]
struct Cache {
    /// Displayed formulas and inline formulas, by their LaTeX.
    formulas: [HashMap<Box<str>, Typeset>; 2],
    bytes: usize,
}

/// The MathML math writer of one thread: a cache of typeset formulas, filled by
/// [`MathMl::prepare`] and read by the renderer through [`MathWriter`].
#[derive(Debug, Default)]
pub struct MathMl {
    limits: Limits,
    cache: RefCell<Cache>,
}

/// What [`MathMl::prepare`] did with a document's formulas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Prepared {
    /// The formulas in the document.
    pub formulas: usize,
    /// How many of them were typeset now, not taken from the cache.
    pub typeset: usize,
    /// How many stay LaTeX; each has a finding.
    pub refused: usize,
}

impl MathMl {
    /// Makes a math writer with an empty cache and the default limits.
    pub fn new() -> MathMl {
        MathMl::default()
    }

    /// Makes a math writer with an empty cache and `limits`.
    pub fn with_limits(limits: Limits) -> MathMl {
        MathMl { limits, cache: RefCell::default() }
    }

    /// The body of a pass: typesets the formulas of `doc` the cache does not hold, all
    /// in one compiler run, and reports a finding through `b` for each formula that
    /// stays LaTeX. It leaves the document as it is.
    pub fn prepare(&self, doc: Document<'_>, b: &mut Builder<'_>) -> PassOutcome {
        self.prepare_counted(doc, b);
        PassOutcome::Unchanged
    }

    /// [`MathMl::prepare`], returning what it did.
    pub fn prepare_counted(&self, doc: Document<'_>, b: &mut Builder<'_>) -> Prepared {
        let mut cache = self.cache.borrow_mut();
        if cache.bytes > self.limits.max_cache_bytes {
            *cache = Cache::default();
        }
        let nodes: Vec<(NodeId, bool)> = (0..doc.node_count())
            .map(NodeId::from_index)
            .filter_map(|id| match doc.kind(id) {
                NodeKind::Math => Some((id, false)),
                NodeKind::MathBlock => Some((id, true)),
                _ => None,
            })
            // A formula written as MathML is the renderer's to filter, not ours to
            // translate.
            .filter(|&(id, _)| !guidedoc::is_mathml(doc, id))
            .collect();
        let typeset = self.fill(&mut cache, doc, &nodes);
        let mut refused = 0;
        for &(id, display) in &nodes {
            let entry = cache.formulas[usize::from(display)].get(doc.node_text(id));
            if let Some(Err(refusal)) = entry {
                let (message, arg) = refusal.finding();
                b.report(message, doc.node(id).span, arg, Arg::None);
                refused += 1;
            }
        }
        Prepared { formulas: nodes.len(), typeset, refused }
    }

    /// Typesets the formulas of `nodes` the cache lacks; returns how many were new.
    fn fill(&self, cache: &mut Cache, doc: Document<'_>, nodes: &[(NodeId, bool)]) -> usize {
        let mut new: Vec<typeset::Formula<'_>> = Vec::new();
        for &(id, display) in nodes {
            let latex = doc.node_text(id);
            let known = &mut cache.formulas[usize::from(display)];
            if known.contains_key(latex) {
                continue;
            }
            let limit = if latex.len() > self.limits.max_formula_bytes {
                Some("it is longer than the formula limit")
            } else if new.len() >= self.limits.max_formulas {
                Some("the document has more formulas than the limit")
            } else {
                None
            };
            // The entry also marks the formula as seen; `finish` replaces it below.
            known.insert(latex.into(), Err(Refusal::Limit(limit.unwrap_or(""))));
            if limit.is_none() {
                new.push(typeset::Formula::new(latex, display));
            }
        }
        typeset::typeset(&mut new);
        let count = new.len();
        for formula in new {
            let (latex, display, result) = formula.finish();
            cache.bytes += latex.len() + result.as_ref().map_or(64, |m| m.len());
            cache.formulas[usize::from(display)].insert(latex.into(), result);
        }
        count
    }

    /// The MathML of a prepared formula; `None` when it was refused or never prepared.
    pub fn mathml(&self, latex: &str, display: bool) -> Option<String> {
        let cache = self.cache.borrow();
        match cache.formulas[usize::from(display)].get(latex) {
            Some(Ok(mathml)) => Some(mathml.to_string()),
            _ => None,
        }
    }
}

impl MathWriter for MathMl {
    fn write_math(&self, latex: &str, display: bool, out: &mut Emitter<'_>) -> bool {
        let cache = self.cache.borrow();
        match cache.formulas[usize::from(display)].get(latex) {
            Some(Ok(mathml)) => {
                out.emit(mathml);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
