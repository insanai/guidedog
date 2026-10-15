//! Shared pieces of the Guidedoc example programs in `examples/`.
//!
//! The engine (`guidedoc`) knows no reader or renderer; a caller puts the adapters it
//! wants into a [`Registry`]. The examples all use the one [`registry`] builds here, from
//! the reader and renderer crates directly.
//!
//! ```
//! let registry = guidedoc_examples::registry();
//! assert!(guidedoc::validate_registry(&registry).is_ok());
//! ```

use guidedoc::Registry;

/// Returns a registry with the reStructuredText, CommonMark, and MyST readers and the
/// HTML, Typst, and plain-text renderers.
///
/// Borrows nothing; the adapters are plain `const` descriptors, so the registry is cheap
/// to build and the caller owns it. It cannot fail: [`guidedoc::validate_registry`]
/// accepts it (unique ids, versions, and functions), which the test below checks.
pub fn registry() -> Registry {
    Registry {
        readers: vec![
            guidedoc_rst::reader(),
            guidedoc_commonmark::READER,
            guidedoc_commonmark::MYST_READER,
        ],
        renderers: vec![
            guidedoc_html::renderer(),
            guidedoc_typst::renderer(),
            guidedoc_text::renderer(),
        ],
    }
}

/// Picks the reader for a file name: `"commonmark"` for `.md`, `"rst"` otherwise.
pub fn reader_for(path: &str) -> &'static str {
    if path.ends_with(".md") { "commonmark" } else { "rst" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_valid() {
        assert!(guidedoc::validate_registry(&registry()).is_ok());
    }

    #[test]
    fn markdown_files_use_the_commonmark_reader() {
        assert_eq!(reader_for("notes.md"), "commonmark");
        assert_eq!(reader_for("guide.rst"), "rst");
    }
}
