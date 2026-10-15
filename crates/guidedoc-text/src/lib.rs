//! The plain-text renderer of Guidedoc, following the conventions of Sphinx's text
//! builder: paragraphs wrapped at 70 columns, titles underlined with `* = - ~` and the
//! other section characters, `* ` and `1. ` list markers with hanging indents,
//! definitions and literal blocks indented by three spaces, grid tables drawn with
//! `+---+` borders, and `Note:` admonitions.
//!
//! ```
//! use guidedoc::Registry;
//! let registry = Registry { renderers: vec![guidedoc_text::RENDERER], ..Registry::default() };
//! assert!(registry.find_renderer("text").is_some());
//! ```

#![forbid(unsafe_code)]

mod blocks;
mod frames;
mod inlines;
mod state;
mod tables;

#[cfg(test)]
mod tests;

use guidedoc::{Conformance, Fail, Message, ROOT, RenderContext, Renderer, Severity, Status};

use state::State;

/// The renderer's version: the project version every crate inherits.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How completely the renderer implements plain-text output.
pub const CONFORMANCE: Conformance = Conformance::Partial;

/// The "text" renderer descriptor, for a `guidedoc::Registry`. It is a value with no
/// state: registering a copy is harmless.
pub const RENDERER: Renderer = Renderer {
    id: "text",
    name: "Plain text",
    version: VERSION,
    media_type: "text/plain",
    conformance: CONFORMANCE,
    render,
};

/// Returns `RENDERER` (Odin `text_render.renderer()`).
pub fn renderer() -> Renderer {
    RENDERER
}

/// Writes `ctx.doc` as plain text through `ctx.out`. It borrows the context for the
/// call and keeps nothing. Problems go to the context's reports; the render fails with
/// the status of the first error: raw content the policy refuses (`Policy`), an
/// unresolved reference (`InvalidInput`), or nesting past the limits (`Limit`).
pub fn render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    if ctx.doc.node_count() == 0 {
        return Ok(());
    }
    let mut s = State::new(ctx);
    s.scan_document();
    s.render_blocks(ROOT);
    let status = s.status;
    let text = std::mem::take(&mut s.out);
    ctx.out.emit(&text);
    match status {
        Status::Ok => Ok(()),
        status => Err(Fail::new(status)),
    }
}

/// Raw content the policy refuses; `{0}` names its format.
pub const RAW_REFUSED: Message = Message {
    code: "text.raw.refused",
    severity: Severity::Error,
    status: Status::Policy,
    title: "RAW CONTENT REFUSED",
    message: "Raw {0} content is not allowed by the conversion policy.",
    hint: "Remove the raw content, or convert with --raw=allow if you trust this source.",
};

/// Raw text the policy leaves out.
pub const RAW_OMITTED: Message = Message {
    code: "text.raw.omitted",
    severity: Severity::Warning,
    status: Status::Policy,
    title: "RAW CONTENT OMITTED",
    message: "Raw {0} content was left out of the text output.",
    hint: "Use --raw=allow to keep trusted raw text, or remove it from the source.",
};

/// Raw content for another format, which plain text cannot show.
pub const RAW_FORMAT: Message = Message {
    code: "text.raw.format",
    severity: Severity::Warning,
    status: Status::Unsupported,
    title: "RAW CONTENT FOR ANOTHER FORMAT",
    message: "Raw {0} content cannot be shown as plain text and was left out.",
    hint: "Add an equivalent raw text block next to it, or use a portable construct.",
};

/// A reference that leads nowhere.
pub const UNRESOLVED: Message = Message {
    code: "text.reference.unresolved",
    severity: Severity::Error,
    status: Status::InvalidInput,
    title: "UNRESOLVED REFERENCE",
    message: "The reference {0} does not point to any target.",
    hint: "Define the target, or correct the reference name.",
};

/// Substitutions expanding through more than `Limits::max_expansion` levels.
pub const EXPANSION: Message = Message {
    code: "text.substitution.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "SUBSTITUTIONS NESTED TOO DEEPLY",
    message: "The substitution {0} expands through more than {1} levels.",
    hint: "Remove the cycle between substitution definitions, or flatten them.",
};

/// Content nested deeper than the renderer's recursion allows.
pub const DEPTH_LIMIT: Message = Message {
    code: "text.limit.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "CONTENT NESTED TOO DEEPLY FOR TEXT",
    message: "Rendering this content needs more than {0} nested levels.",
    hint: "Flatten the nesting, or raise limits.max_depth.",
};
