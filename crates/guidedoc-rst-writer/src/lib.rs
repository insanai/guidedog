//! The reStructuredText writer of Guidedoc: it writes a document tree back out as
//! clean, idiomatic reStructuredText that a reader reads back into the same tree. It
//! serves the `rst` renderer id, so Guidedog converts Markdown (MyST with the Sphinx
//! layer) and reStructuredText to reStructuredText itself.
//!
//! Paragraphs are wrapped at [`Settings::width`] columns (never inside inline markup),
//! sections take one adornment per level (`=`, `-`, `~`, `^`, ...), and every Sphinx
//! construct the tree keeps (cross-references, toctrees, domain objects, info fields)
//! is written as its directive or role. Where the tree has lost what reStructuredText
//! would need, the writer chooses the closest faithful markup and reports a Note
//! ([`LOSSY`]); the crate README lists those cases.
//!
//! ```
//! use guidedoc::Registry;
//! let renderers = vec![guidedoc_rst_writer::RENDERER];
//! let registry = Registry { renderers, ..Registry::default() };
//! assert!(registry.find_renderer("rst").is_some());
//! ```

#![forbid(unsafe_code)]

mod blocks;
mod desc;
mod directives;
mod escape;
mod explicit;
mod inline;
mod inlines;
mod lists;
mod literal;
mod roles;
mod sections;
mod sphinx;
mod state;
mod tables;
mod walk;

#[cfg(test)]
mod tests;

use guidedoc::{Conformance, Fail, Message, ROOT, RenderContext, Renderer, Severity, Status};

use state::Writer;

/// The writer's version: the project version every crate inherits.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How completely the writer covers the document model.
pub const CONFORMANCE: Conformance = Conformance::Partial;

/// The line width paragraphs are wrapped at by default.
pub const DEFAULT_WIDTH: usize = 92;

/// The "rst" renderer descriptor, for a `guidedoc::Registry`. It writes with the
/// default [`Settings`]; call [`render_with`] for others.
pub const RENDERER: Renderer = Renderer {
    id: "rst",
    name: "reStructuredText",
    version: VERSION,
    media_type: "text/x-rst",
    conformance: CONFORMANCE,
    render,
};

/// Returns `RENDERER`.
pub fn renderer() -> Renderer {
    RENDERER
}

/// How the writer lays out its output. Fill it with struct-update syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// The column paragraphs wrap at (indentation included); markup and URLs that are
    /// longer stay whole.
    pub width: usize,
}

impl Default for Settings {
    /// Wraps at [`DEFAULT_WIDTH`].
    fn default() -> Self {
        Settings { width: DEFAULT_WIDTH }
    }
}

/// Writes `ctx.doc` as reStructuredText with the default settings. See
/// [`render_with`].
pub fn render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    render_with(ctx, Settings::default())
}

/// Writes `ctx.doc` as reStructuredText through `ctx.out`, laid out by `settings`. It
/// borrows the context for the call and keeps nothing. What cannot be written exactly
/// is reported as a Note ([`LOSSY`]) and written as closely as reStructuredText allows;
/// the render fails only when content nests past the limits (`Limit`).
pub fn render_with(ctx: &mut RenderContext<'_>, settings: Settings) -> Result<(), Fail> {
    if ctx.doc.node_count() == 0 {
        return Ok(());
    }
    let settings = Settings { width: settings.width.max(20) };
    let mut w = Writer::new(ctx, settings);
    w.scan_titles();
    w.blocks(ROOT);
    w.flush_substitutions();
    let mut text = String::with_capacity(w.out.len() + 256);
    w.roles.write(&mut text);
    text.push_str(&w.out);
    let status = w.status;
    drop(w);
    let end = text.trim_end_matches('\n').len();
    text.truncate(end);
    if !text.is_empty() {
        text.push('\n');
    }
    ctx.out.emit(&text);
    match status {
        Status::Ok => Ok(()),
        status => Err(Fail::new(status)),
    }
}

/// Something reStructuredText cannot express exactly: `{0}` says what, `{1}` what
/// was written instead.
pub const LOSSY: Message = Message {
    code: "rst.write.lossy",
    severity: Severity::Note,
    status: Status::Ok,
    title: "WRITTEN APPROXIMATELY",
    message: "reStructuredText cannot express {0} exactly; the writer wrote {1}.",
    hint: "Check this place in the output; the guidedoc-rst-writer README lists every \
           such case and why.",
};

/// Content nested deeper than the writer's recursion allows.
pub const DEPTH_LIMIT: Message = Message {
    code: "rst.limit.depth",
    severity: Severity::Error,
    status: Status::Limit,
    title: "CONTENT NESTED TOO DEEPLY FOR RESTRUCTUREDTEXT",
    message: "Writing this content needs more than {0} nested levels.",
    hint: "Flatten the nesting, or raise limits.max_depth.",
};
