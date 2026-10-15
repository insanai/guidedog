//! The Typst-source renderer of Guidedoc: a validated document becomes one Typst source
//! file. It writes text, not PDF; `gd-typst` compiles it.
//!
//! The output is a fixed, trusted preamble of `gd-` helper definitions ([`PREAMBLE`])
//! followed by the body. Source text never becomes executable Typst: markup text escapes
//! every ASCII punctuation character, and code, URLs, labels, and paths are string
//! literals. Raw Typst is written only under `ContentPolicy::Allow`; LaTeX math goes
//! through a bounded adapter ([`translate_math`]) whose failures follow the math policy.
//!
//! ```
//! use guidedoc::Registry;
//! let registry = Registry { readers: vec![], renderers: vec![guidedoc_typst::RENDERER] };
//! assert!(registry.find_renderer("typst").is_some());
//! assert!(guidedoc_typst::PREAMBLE.contains("#let gd-link"));
//! ```

#![forbid(unsafe_code)]

mod blocks;
mod dispatch;
mod escape;
mod inlines;
mod math;
mod messages;
mod notes;
mod settings;
mod sphinx;
mod state;
mod structure;
mod tables;
#[cfg(test)]
mod tests;

use guidedoc::{Conformance, Emitter, Fail, ROOT, RenderContext, Renderer, stack_guard};

pub use escape::{is_label_safe, write_markup_text, write_string_literal};
pub use math::{MathError, translate_math};
pub use messages::{
    CITATION_MISSING, DEPTH_LIMIT, DIMENSION_IGNORED, FOOTNOTE_MISSING, IMAGE_REMOTE,
    LANGUAGE_INVALID, LINK_UNRESOLVED, MATH_OMITTED, MATH_UNSUPPORTED, MATH_VERBATIM, RAW_FOREIGN,
    RAW_FOREIGN_OMITTED, RAW_OMITTED, RAW_REFUSED, SUBSTITUTION_MISSING, URL_OMITTED, URL_UNSAFE,
};

use state::State;

/// The renderer's version: the project version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How completely the renderer covers the document model.
pub const CONFORMANCE: Conformance = Conformance::Partial;

/// The "typst" renderer descriptor, for a `guidedoc::Registry`. It has no state.
pub const RENDERER: Renderer = Renderer {
    id: "typst",
    name: "Typst source",
    version: VERSION,
    media_type: "text/x-typst",
    conformance: CONFORMANCE,
    render,
};

/// Returns [`RENDERER`], the Typst renderer descriptor.
pub fn renderer() -> Renderer {
    RENDERER
}

/// The helper definitions every body relies on: fixed, trusted Typst into which no
/// document data is ever interpolated.
pub const PREAMBLE: &str = include_str!("preamble.typ");

/// Writes [`PREAMBLE`]. A host composing a book from body-only chapters
/// (`RenderOptions::standalone = false`) writes it once, before the chapters.
pub fn emit_preamble(out: &mut Emitter<'_>) {
    out.emit(PREAMBLE);
}

/// Writes `ctx.doc` as Typst source through `ctx.out`: the preamble, the document and
/// page settings, and the body when `ctx.options.standalone`, else the body alone. It
/// borrows the context for the call. Problems go to `ctx.reports()`; it fails with the
/// first error (raw content the policy refuses, an unresolved reference, untranslatable
/// math under the refuse policy, nesting past the limits), or `Capacity` when the
/// per-node marks cannot be allocated.
pub fn render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    let stack = stack_guard(&ctx.limits);
    let work = ctx.take_scratch();
    let mut s = State::new(ctx, stack, work)?;
    let scan = s.scan_document();
    if s.options.standalone {
        emit_preamble(&mut s.ctx.out);
        s.emit_settings(&scan);
    }
    s.render_blocks(ROOT);
    s.flush_footnotes();
    let (result, work) = s.finish();
    ctx.put_scratch(work);
    result
}
