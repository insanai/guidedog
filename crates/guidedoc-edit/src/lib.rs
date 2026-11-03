//! Editable source maps and source-preserving edits for reStructuredText and Markdown.
//!
//! [`load_source_map`] reads one document with Guidedog's own readers (reStructuredText
//! with the Sphinx layer, Markdown as MyST) and returns a tree of blocks: each block has
//! a stable [`BlockKind`], the byte span of its whole markup, the spans of the text an
//! author edits (without markup), the parts of a directive, and its children.
//! [`apply_operation`] turns an [`Operation`], such as replacing a paragraph's text or
//! moving a block, into minimal [`TextEdit`]s that leave every other byte untouched; it
//! reads the result again and refuses an edit that would change the document's
//! structure in any way the operation does not intend.
//!
//! ```
//! use guidedoc_edit::{
//!     BlockKind, Format, MapOptions, Operation, apply_operation, load_source_map,
//! };
//!
//! let text = "Title\n=====\n\nSome *old* text.\n";
//! let options = MapOptions::new(Format::Rst);
//! let map = load_source_map(text, &options);
//! let para = map.find(BlockKind::Paragraph).unwrap();
//! assert_eq!(map.content_text(para, text), "Some *old* text.");
//! let op = Operation::ReplaceText { block: para, text: "New text." };
//! let edit = apply_operation(text, &options, &op).unwrap();
//! assert_eq!(edit.text, "Title\n=====\n\nNew text.\n");
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod edit;
mod error;
mod gaps;
mod items;
mod json;
mod lines;
mod map;
mod ops;
mod options;
mod parts;
mod shape;
mod walk;

#[cfg(test)]
mod tests;

pub use edit::{
    EditResult, Operation, OperationKind, Position, TextEdit, apply_edits, apply_operation,
    block_operations, operations,
};
pub use error::EditError;
pub use json::write_source_map_json;
pub use map::{
    Block, BlockId, BlockKind, DirectiveParts, OptionParts, SourceMap, load_source_map,
};

/// The version of the source map's structure and of its JSON form
/// ([`write_source_map_json`]); it changes when a field or a kind changes meaning.
pub const SOURCE_MAP_VERSION: u32 = 1;

/// The most blocks a map lists by default; a longer document maps its first blocks and
/// the map says it is truncated.
pub const DEFAULT_MAX_BLOCKS: usize = 20_000;

/// The markup of a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Format {
    /// reStructuredText, read with the Sphinx layer (reader `sphinx`).
    #[default]
    Rst,
    /// Markdown, read as MyST with the Sphinx layer (reader `myst`), as projects read it.
    Markdown,
}

impl Format {
    /// Both formats.
    pub const ALL: [Format; 2] = [Format::Rst, Format::Markdown];

    /// The format's name: `rst` or `markdown`.
    pub fn name(self) -> &'static str {
        match self {
            Format::Rst => "rst",
            Format::Markdown => "markdown",
        }
    }

    /// The id of the Guidedog reader the map reads the format with.
    pub fn reader(self) -> &'static str {
        match self {
            Format::Rst => guidedoc_sphinx::READER.id,
            Format::Markdown => guidedoc_commonmark::MYST_READER.id,
        }
    }

    /// The file suffixes of the format, with their dots, as projects recognize them.
    pub fn suffixes(self) -> &'static [&'static str] {
        match self {
            Format::Rst => &[".rst", ".rest"],
            Format::Markdown => &[".md", ".markdown"],
        }
    }

    /// The format of a file path by its suffix, compared case-insensitively; `None` for
    /// another suffix.
    pub fn of_path(path: &str) -> Option<Format> {
        let lower = path.to_ascii_lowercase();
        Format::ALL.into_iter().find(|f| f.suffixes().iter().any(|s| lower.ends_with(s)))
    }
}

/// How to read a document for its map. Fill it with struct-update syntax:
/// `MapOptions { sphinx: Some(&config), ..MapOptions::new(Format::Rst) }`.
#[derive(Debug, Clone, Copy)]
pub struct MapOptions<'a> {
    /// The document's markup.
    pub format: Format,
    /// The Sphinx layer's configuration (the project's conf.toml, as
    /// `gd_read::sphinx_config` makes it); `None` reads with `Config::default()`. It
    /// decides which directives exist and which options they take.
    pub sphinx: Option<&'a guidedoc_sphinx::Config>,
    /// The MyST settings for Markdown (enabled syntax extensions); `None` reads with
    /// `guidedoc_commonmark::Settings::default()`.
    pub myst: Option<&'a guidedoc_commonmark::Settings>,
    /// The most blocks to list.
    pub max_blocks: usize,
}

impl MapOptions<'static> {
    /// The options for `format` with default configuration.
    pub fn new(format: Format) -> MapOptions<'static> {
        MapOptions { format, sphinx: None, myst: None, max_blocks: DEFAULT_MAX_BLOCKS }
    }
}

impl Default for MapOptions<'static> {
    fn default() -> Self {
        MapOptions::new(Format::Rst)
    }
}

/// A half-open byte range `start..end` of the document's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Span {
    /// The first byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl Span {
    /// The span `start..end`.
    pub const fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    /// The number of bytes.
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Tells whether the span holds no bytes.
    pub const fn is_empty(self) -> bool {
        self.end <= self.start
    }

    /// Tells whether `other` lies inside this span.
    pub const fn contains(self, other: Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    /// The text of the span in `text`; empty when the span is not inside it.
    pub fn text(self, text: &str) -> &str {
        text.get(self.start..self.end).unwrap_or("")
    }
}
