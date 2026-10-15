//! The built-in pieces Guidedog assembles around the engine.
//!
//! Today this holds the highlighters that connect `gd-highlight` to the renderers
//! ([`HtmlHighlighter`], [`TypstHighlighter`]). The built-in reader and renderer
//! registry joins it once the Sphinx layer is ported.

mod highlight;

pub use highlight::{HtmlHighlighter, TypstHighlighter};
