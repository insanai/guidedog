//! Markdown for Guidedoc, as two readers with distinct ids: "commonmark" is pure
//! CommonMark 0.31.2 and takes no settings; "myst" is the MyST Markdown that Sphinx
//! projects write (myst-parser 0.16.1): directives, roles, targets, comments, front
//! matter, footnotes, GFM tables, and the optional syntax `Settings::enable_extensions`
//! turns on.
//!
//! Block structure is parsed line by line into a skeleton; link reference definitions
//! are collected over the whole document; then the skeleton is emitted in preorder, with
//! inline content parsed per paragraph and heading. Directive content is parsed the same
//! way when a handler asks for it (`Extension`, `Host::parse_body`), so a parse works on
//! line views of any source.
//!
//! ```
//! use guidedoc::{Workspace, default_request, dump_to_string, read_document};
//! let mut ws = Workspace::new();
//! let request = default_request("a.md", "# Hi *there*\n", "commonmark", "html");
//! let doc = read_document(&mut ws, &guidedoc_commonmark::READER, &request).unwrap();
//! assert!(dump_to_string(doc).contains("emphasis"));
//! ```

mod blocks;
mod emit;
mod extension;
mod host;
mod inline;
mod messages;
mod myst;
mod refs;
mod settings;
mod starts;

use guidedoc::{Conformance, Fail, ReadContext, Reader, SourceSpan, Status};

pub use extension::{
    Directive, DirectiveOption, DirectiveResult, DirectiveShape, Extension, Line, ListItems, Role,
};
pub use host::Host;
pub use settings::{
    DEFAULT_URL_SCHEMES, Meta, Rule, Settings, Substitution, SyntaxExtension, SyntaxExtensions,
};

use blocks::{parse_lines, split_source};

/// The CommonMark version the "commonmark" reader implements.
pub const VERSION: &str = "0.31.2";
/// The myst-parser version the "myst" reader follows, with the differences
/// docs/manual/internals/commonmark-conformance.rst documents.
pub const MYST_VERSION: &str = "0.16.1";

/// The "commonmark" reader: pure CommonMark 0.31.2, headings without sections, no
/// settings (any are refused with `InvalidInput`).
pub const READER: Reader = Reader {
    id: "commonmark",
    name: "CommonMark",
    version: VERSION,
    conformance: Conformance::Conformant,
    read,
    read_inline: Some(read_inline),
};

/// The "myst" reader: MyST Markdown with the `Settings` the read options carry
/// (`guidedoc::read_extension(&settings)`), or `Settings::default()` without any.
pub const MYST_READER: Reader = Reader {
    id: "myst",
    name: "MyST Markdown",
    version: MYST_VERSION,
    conformance: Conformance::Partial,
    read: read_myst,
    read_inline: Some(read_myst_inline),
};

/// Reads `ctx.source` as pure CommonMark 0.31.2. Settings in the read options are
/// refused with `InvalidInput`; problems go to the reports; a builder failure (limits,
/// capacity) is returned.
pub fn read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    ctx.no_reader_settings()?;
    let mut h = Host::new(ctx, host::default_settings(), false);
    let text = h.ctx.source_of(h.ctx.source);
    let lines = split_source(&text, 0);
    let source = h.ctx.source;
    parse_lines(&mut h, &lines, source, false, false)
}

/// Reads `ctx.source` as MyST Markdown with the `Settings` of the read options. Settings
/// of another type are refused with `InvalidInput`.
pub fn read_myst(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let settings = ctx.reader_settings::<Settings>()?.unwrap_or(host::default_settings());
    let mut h = Host::new(ctx, settings, true);
    let text = h.ctx.source_of(h.ctx.source);
    let lines = split_source(&text, 0);
    myst::myst_document(&mut h, &lines)
}

/// Parses `span` of `ctx.source` as CommonMark inline content of the builder's open
/// node, for a translated message. Link reference definitions and footnotes of the
/// surrounding document are not known to it.
pub fn read_inline(ctx: &mut ReadContext<'_>, span: SourceSpan) -> Result<(), Fail> {
    ctx.no_reader_settings()?;
    let mut h = Host::new(ctx, host::default_settings(), false);
    inline_source(&mut h, span)
}

/// `read_inline` for MyST, with the settings of the read options (roles included).
pub fn read_myst_inline(ctx: &mut ReadContext<'_>, span: SourceSpan) -> Result<(), Fail> {
    let settings = ctx.reader_settings::<Settings>()?.unwrap_or(host::default_settings());
    let mut h = Host::new(ctx, settings, true);
    myst::prepare(&mut h);
    if let Some(ext) = h.ext() {
        ext.begin(&mut h)?;
    }
    inline_source(&mut h, span)
}

fn inline_source(h: &mut Host<'_, '_>, span: SourceSpan) -> Result<(), Fail> {
    let text = h.ctx.source_of(span.source);
    let range = span.start as usize..span.end as usize;
    let Some(body) = text.get(range).filter(|_| span.source == h.ctx.source) else {
        return Err(Fail::new(Status::InvalidInput));
    };
    let lines: Vec<Line<'_>> = split_source(body, span.start)
        .into_iter()
        .map(|l| {
            let trimmed = l.text.trim_start_matches([' ', '\t']);
            Line { text: trimmed, offset: l.offset + (l.text.len() - trimmed.len()) as u32 }
        })
        .collect();
    myst::inline_lines(h, &lines, span.source)
}
