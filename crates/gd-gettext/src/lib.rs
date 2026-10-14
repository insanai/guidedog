//! GNU gettext message catalogs: reading and writing PO/POT text, reading and writing
//! compiled MO files (both byte orders), Plural-Forms expressions, msgmerge-style
//! updates with fuzzy matching, and translation statistics.
//!
//! State is explicit: a [`Catalog`] or a [`Translator`] instead of the C library's
//! process-wide domain table. Nothing panics on bad input; every failure is an
//! [`Error`] whose `Display` is an Elm-style report with a hint.
//!
//! ```
//! let cat = gd_gettext::parse_po("msgid \"Hello\"\nmsgstr \"Hallo\"\n", "de.po", false)?;
//! assert_eq!(cat.text("Hello"), "Hallo");
//! assert_eq!(cat.plural("%d file", "%d files", 3), "%d files");
//! # Ok::<(), gd_gettext::Error>(())
//! ```
//!
//! Reading, merging, and writing cost what the input is long, never its square: a PO
//! field continued over many lines is gathered once; an MO file whose strings share
//! bytes is refused as damaged; the fuzzy search is bounded by
//! [`FUZZY_WORK_PER_BYTE`]; and wrapping a long line looks only at the next piece.

mod catalog;
mod error;
mod fuzzy;
mod lookup;
mod merge;
mod mo;
mod plural;
mod po_read;
mod po_write;
mod translator;

#[cfg(test)]
mod tests;

pub use catalog::{Catalog, Message, Previous};
pub use error::{Error, ErrorKind};
pub use fuzzy::{FUZZY_MIN_WORK, FUZZY_WORK_PER_BYTE};
pub use lookup::Forms;
pub use merge::{
    FUZZY_THRESHOLD, MergeOptions, MergeStats, Stats, format_stats, merge, merge_counted,
    similarity, sort_by_reference, stats,
};
pub use mo::{load_mo, parse_mo, write_mo};
pub use plural::{
    MAX_NPLURALS, MAX_PLURAL_DEPTH, MAX_PLURAL_NODES, PluralProblem, PluralRule, compile_plural,
    parse_plural_forms, plural_forms_for, plural_index,
};
pub use po_read::{load_po, parse_po, parse_po_bytes};
pub use po_write::{
    ReferenceStyle, WriteOptions, escape_string, save_po, write_po, write_po_into,
};
pub use translator::{BoundCatalog, Translator, find_catalog_file, language_candidates};
