//! The built-in pieces Guidedog assembles around the engine: the registry of readers and
//! renderers ([`registry`]), typed read options for the built-in readers
//! ([`rst_options`], [`sphinx_options`], [`myst_options`]), and the highlighters that
//! connect `gd-highlight` to the renderers ([`HtmlHighlighter`], [`TypstHighlighter`]).
//!
//! ```
//! let registry = guidedoc_defaults::registry();
//! let mut ws = guidedoc::Workspace::new();
//! let html = guidedoc::convert_text(&mut ws, &registry, "myst", "html", "# Hello\n");
//! assert!(html.expect("converts").contains("Hello"));
//! ```

#![forbid(unsafe_code)]

mod highlight;
mod registry;

pub use highlight::{HtmlHighlighter, TypstHighlighter};
pub use registry::{ReaderSettings, myst_options, registry, rst_options, sphinx_options};
