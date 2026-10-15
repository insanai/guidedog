//! The HTML renderer of Guidedoc. The default output is semantic HTML with anchors,
//! notes, and tables, in the element structure Docutils' and Sphinx's HTML writers use;
//! the reference profile (`RenderOptions::reference_profile`) reproduces the CommonMark
//! reference renderer byte for byte, for conformance tests. Raw content and unsafe URLs
//! are governed by the conversion policy.
//!
//! ```
//! use guidedoc::Registry;
//! let registry = Registry { renderers: vec![guidedoc_html::RENDERER], ..Registry::default() };
//! assert!(registry.find_renderer("html").is_some());
//! ```

mod blocks;
mod code;
mod document;
mod escape;
mod fields;
mod inlines;
mod notes;
mod sections;
mod sphinx;
mod state;
mod tables;
mod walk;

use guidedoc::{
    Conformance, Fail, Message, ROOT, RenderContext, Renderer, Severity, Status, build_anchors,
};

use state::Html;

/// The renderer's version: the project version every crate inherits.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How completely the renderer implements HTML output.
pub const CONFORMANCE: Conformance = Conformance::Partial;

/// The "html" renderer descriptor, for a `guidedoc::Registry`. It is a value with no
/// state: registering a copy is harmless.
pub const RENDERER: Renderer = Renderer {
    id: "html",
    name: "HTML",
    version: VERSION,
    media_type: "text/html",
    conformance: CONFORMANCE,
    render,
};

/// Returns `RENDERER` (Odin `html.renderer()`).
pub fn renderer() -> Renderer {
    RENDERER
}

/// Writes `ctx.doc` as HTML through `ctx.out`: a whole page when
/// `ctx.options.standalone`, else a fragment, and the CommonMark reference renderer's
/// bytes when `ctx.options.reference_profile`. It borrows the context for the call and
/// keeps nothing. Problems go to the context's reports; the render fails with the
/// status of the first error: raw content or a URL the policy refuses (`Policy`), an
/// unresolved reference (`InvalidInput`), or nesting past the limits (`Limit`).
pub fn render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    if ctx.doc.node_count() == 0 {
        return Ok(());
    }
    let reference = ctx.options.reference_profile;
    let anchors = build_anchors(ctx.doc, ctx.options, !reference);
    let mut h = Html::new(ctx, anchors);
    h.build_notes();
    h.scan_document();
    let standalone = h.options.standalone;
    if standalone {
        h.document_start();
    }
    let end = h.doc.subtree_end(ROOT);
    h.walk(ROOT, end, None);
    if standalone {
        h.document_end();
    }
    match h.w.status {
        Status::Ok => Ok(()),
        status => Err(Fail::new(status)),
    }
}

/// Raw content the policy refuses; `{0}` names its format.
pub const RAW_REFUSED: Message = Message {
    code: "html.raw.refused",
    severity: Severity::Error,
    status: Status::Policy,
    title: "RAW CONTENT REFUSED",
    message: "Raw {0} content is not allowed by the conversion policy.",
    hint: "Remove the raw content, or convert with --raw=allow if you trust this source.",
};

/// Raw HTML the policy leaves out.
pub const RAW_OMITTED: Message = Message {
    code: "html.raw.omitted",
    severity: Severity::Warning,
    status: Status::Policy,
    title: "RAW CONTENT OMITTED",
    message: "Raw {0} content was left out of the HTML output.",
    hint: "Use --raw=allow to keep trusted raw HTML, or remove it from the source.",
};

/// Raw content for another format, which HTML cannot show.
pub const RAW_FORMAT: Message = Message {
    code: "html.raw.format",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "RAW CONTENT FOR ANOTHER FORMAT",
    message: "Raw {0} content cannot be shown in HTML and was left out.",
    hint: "Add an equivalent raw html block next to it, or use a portable construct.",
};

/// A link target whose scheme can run code, refused by the policy.
pub const URL_UNSAFE: Message = Message {
    code: "html.url.unsafe",
    severity: Severity::Error,
    status: Status::Policy,
    title: "UNSAFE URL REFUSED",
    message: "The link target {0} uses a scheme that can run code.",
    hint: "Use an http, https, or relative URL, or allow unsafe URLs for trusted input.",
};

/// A link target whose scheme can run code, left out by the policy.
pub const URL_OMITTED: Message = Message {
    code: "html.url.omitted",
    severity: Severity::Warning,
    status: Status::Policy,
    title: "UNSAFE URL OMITTED",
    message: "The link target {0} uses a scheme that can run code; it was left out.",
    hint: "Use an http, https, or relative URL instead.",
};

/// A reference that leads nowhere.
pub const UNRESOLVED: Message = Message {
    code: "html.reference.unresolved",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNRESOLVED REFERENCE",
    message: "The reference {0} does not point to any target.",
    hint: "Define the target, or correct the reference name.",
};

/// Substitutions expanding through more than `Limits::max_expansion` levels.
pub const EXPANSION: Message = Message {
    code: "html.substitution.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "SUBSTITUTIONS NESTED TOO DEEPLY",
    message: "The substitution {0} expands through more than {1} levels.",
    hint: "Remove the cycle between substitution definitions, or flatten them.",
};
#[cfg(test)]
mod tests;
