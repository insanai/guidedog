//! The reStructuredText writer of Guidedoc: it writes a document tree back out as
//! clean, idiomatic reStructuredText that a reader reads back into the same tree. It
//! serves the `rst` renderer id, so Guidedog converts Markdown (MyST with the Sphinx
//! layer) and reStructuredText to reStructuredText itself.
//!
//! Paragraphs keep the author's own lines: each soft line break of the source is
//! written as a line break, and the writer adds none of its own (unless
//! [`Settings::width`] asks it to wrap, which never breaks inside inline markup).
//! Sections take one adornment per level (`=`, `-`, `~`, `^`, ...), and every Sphinx
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

use state::{Writer, WriterWork};

/// The writer's version: the project version every crate inherits.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How completely the writer covers the document model.
pub const CONFORMANCE: Conformance = Conformance::Partial;

/// The line width grid tables are laid out in when [`Settings::width`] is `None`.
pub const TABLE_WIDTH: usize = 92;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    /// `None` (the default) keeps the author's lines: a soft line break of the source
    /// is written as a line break and no other breaks are added. `Some(n)` rewraps
    /// paragraphs at column `n` (indentation included, at least 20) and lays out grid
    /// tables in that width; inline markup, inline code, roles and URLs stay whole on
    /// one line even when they are longer.
    pub width: Option<usize>,
}

/// Writes `ctx.doc` as reStructuredText with the default settings. See
/// [`render_with`].
pub fn render(ctx: &mut RenderContext<'_>) -> Result<(), Fail> {
    render_with(ctx, Settings::default())
}

/// Writes `ctx.doc` as reStructuredText through `ctx.out`, laid out by `settings`. It
/// borrows the context for the call and keeps only its working storage, in the
/// context's scratch store. What cannot be written exactly
/// is reported as a Note ([`LOSSY`]) and written as closely as reStructuredText allows;
/// the render fails only when content nests past the limits (`Limit`).
pub fn render_with(ctx: &mut RenderContext<'_>, settings: Settings) -> Result<(), Fail> {
    if ctx.doc.node_count() == 0 {
        return Ok(());
    }
    let settings = Settings { width: settings.width.map(|w| w.max(20)) };
    let mut work: WriterWork = ctx.take_scratch();
    let mut text = std::mem::take(&mut work.text);
    let mut w = Writer::new(ctx, settings, work);
    w.scan_titles();
    w.blocks(ROOT);
    w.flush_substitutions();
    text.clear();
    text.reserve(w.out.len() + 256);
    w.roles.write(&mut text);
    text.push_str(&w.out);
    let status = w.status;
    let end = text.trim_end_matches('\n').len();
    text.truncate(end);
    if !text.is_empty() {
        text.push('\n');
    }
    let work = w.finish(text);
    ctx.out.emit(&work.text);
    ctx.put_scratch(work);
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
