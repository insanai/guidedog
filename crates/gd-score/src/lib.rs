//! Exact musical time, spelled pitches and a flat score shared by readers and engravers.
//! No file access, font policy or engraving geometry belongs in this crate.

#![forbid(unsafe_code)]

mod check;
mod duration;
mod error;
mod markup;
mod pitch;
mod ratio;
mod score;

pub use check::check_score;
pub use duration::{Base, Duration};
pub use error::{Error, ErrorKind, FileId, Severity, Span};
pub use markup::{ColumnAlign, Enclosure, Markup, MarkupRole, StyleChange};
pub use pitch::{Alter, Key, Mode, Pitch, Step};
pub use ratio::Ratio;
pub use score::*;

#[cfg(test)]
mod tests;
