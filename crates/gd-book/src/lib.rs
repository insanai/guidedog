//! gd-book: Guidedog's `pdf` and `text` builders (and `dummy`, which only links).
//!
//! The pdf builder composes one Typst book per `pdf_documents` entry: the renderer's
//! preamble, the book template (the project's `_templates/book.typ` or
//! `gd_theme::pdf_template`), cover settings, then every document of the root's
//! toctree as a chapter, and the general index. Typst (embedded, or the `typst`
//! program conf.toml names) typesets it; a problem Typst finds in a chapter is reported
//! at the line of reStructuredText or Markdown it came from. The text builder writes one
//! `.txt` file per document.
//! Standalone discussion records select `_templates/discussion.typ` or the built-in
//! RFC-style layout, with structured front matter instead of book covers.
//!
//! ```no_run
//! # fn build(view: &gd_resolve::BuildView<'_>, generation: &mut gd_outputs::Generation)
//! # -> Result<(), gd_host::Problem> {
//! use gd_resolve::OutputBuilder;
//! let mut reports = Vec::new();
//! gd_book::PdfBuilder::default().write(view, generation, &mut reports)?;
//! assert_eq!(gd_book::language_file("manual.pdf", "de"), "manual-de.pdf");
//! # Ok(()) }
//! ```
//!
//! Docs: docs/manual/internals/project.rst. Nothing here prints; problems are
//! `gd_report::Report`s and `gd_host::Problem`s.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod book_index;
mod compose;
mod failure;
mod fonts;
mod inputs;
mod layout;
mod pages;
mod pdf;
mod text;
mod typst_root;

pub use compose::{language_file, typst_markup, typst_string};
pub use fonts::bundled_font_dir;
pub use pdf::{PdfBuilder, TypesetBook};
pub use text::{DummyBuilder, TextBuilder};

#[cfg(test)]
mod tests;
