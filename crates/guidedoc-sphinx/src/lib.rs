//! reStructuredText with the Sphinx layer: Sphinx's directives, roles, and domains,
//! implemented independently on the base reader's extension interface
//! (`guidedoc_rst::Extension`). It produces the shapes of
//! docs/manual/internals/ast.rst, "Sphinx layer"; cross-document work (toctrees,
//! cross-references, numbering) is left to the project host. The same handlers serve
//! MyST Markdown read by the "myst" reader: see [`myst_settings`].
//!
//! ```
//! use guidedoc::{Workspace, default_request, dump_to_string, read_document, read_extension};
//! use guidedoc_sphinx::{Config, reader};
//!
//! let config = Config { primary_domain: "py".into(), ..Config::default() };
//! let mut request = default_request("api.rst", ".. py:function:: f(x)\n", "sphinx", "html");
//! request.read.extension = Some(read_extension(&config));
//! let mut ws = Workspace::new();
//! let doc = read_document(&mut ws, &reader(), &request).unwrap();
//! assert!(dump_to_string(doc).contains("desc-signature"));
//! ```

#![forbid(unsafe_code)]

mod autodoc;
mod code;
mod config;
mod diff;
mod directives;
mod docfield_write;
mod docfields;
mod doctest;
mod domains;
mod extension;
mod glossary;
mod graphviz;
mod ids;
mod literalinclude;
mod markdown;
mod math;
mod messages;
mod object_types;
mod pyobject;
mod roles;
mod state;
mod text;

#[cfg(test)]
mod tests;

use guidedoc::{Conformance, Fail, ReadContext, Reader, SourceSpan};

pub use autodoc::{AutodocMessage, AutodocOption, AutodocProvider, AutodocRequest, AutodocResult};
pub use config::{Autodoc, Config, DirectiveAlias, Extlink, ObjectType, TocParents};
pub use graphviz::{EXTENSIONS, extension_supported};
pub use markdown::{MystExtension, myst_settings};

use extension::Sphinx;

/// The layer's version: the project version.
pub const VERSION: &str = guidedoc::VERSION;
/// How completely the layer implements Sphinx.
pub const CONFORMANCE: Conformance = Conformance::Partial;

/// The "sphinx" reader: reStructuredText with the Sphinx layer. A value with no state:
/// registering it twice or copying it is harmless.
pub const READER: Reader = Reader {
    id: "sphinx",
    name: "reStructuredText (Sphinx)",
    version: VERSION,
    conformance: CONFORMANCE,
    read,
    read_inline: Some(read_inline),
};

/// Returns the "sphinx" reader descriptor, for a `guidedoc::Registry`.
pub fn reader() -> Reader {
    READER
}

/// Reads reStructuredText with the Sphinx layer. `ctx.options.extension` is a
/// `Config` (`guidedoc::read_extension(&config)`), or empty for `Config::default()`;
/// another type is refused with `InvalidInput`. The read keeps nothing of the
/// configuration afterwards. Otherwise it fails as `guidedoc_rst::read`.
pub fn read(ctx: &mut ReadContext<'_>) -> Result<(), Fail> {
    let default;
    let config = match ctx.reader_settings::<Config>()? {
        Some(c) => c,
        None => {
            default = Config::default();
            &default
        }
    };
    let sx = Sphinx::new(config);
    guidedoc_rst::read_with(ctx, &config.settings, Some(&sx))
}

/// Parses a span of `ctx.source` as reStructuredText inline content with Sphinx's
/// roles, for a translated message (`guidedoc::ReadInlineFn`). rst_prolog and
/// rst_epilog do not apply to a message. Settings and failures are `read`'s.
pub fn read_inline(ctx: &mut ReadContext<'_>, span: SourceSpan) -> Result<(), Fail> {
    let given = ctx.reader_settings::<Config>()?.cloned().unwrap_or_default();
    let config = Config { rst_prolog: String::new(), rst_epilog: String::new(), ..given };
    let sx = Sphinx::new(&config);
    guidedoc_rst::read_inline_with(ctx, span, &config.settings, Some(&sx))
}
