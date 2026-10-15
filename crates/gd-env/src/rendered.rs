//! What rendering one document gives a builder, and the interface words it is
//! rendered with.

use std::borrow::Cow;
use std::path::PathBuf;

use gd_report::Report;

use crate::lookup::Trace;

/// An image or download a page needs copied into the output.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Asset {
    /// The file in the source folder (absolute).
    pub source: PathBuf,
    /// Its path below the output folder, with `/` separators.
    pub target: String,
}

/// One document rendered for one output. `failed` means no page could be made (its
/// object could not be loaded, linked, or rendered), which fails the build; problems in
/// the document's content are only reported.
#[derive(Debug, Clone, Default)]
pub struct Rendered {
    /// The rendered body.
    pub body: String,
    /// The images and downloads it needs copied.
    pub assets: Vec<Asset>,
    /// What its references looked up, to verify on the next build.
    pub trace: Trace,
    /// What linking and rendering it reported.
    pub reports: Vec<Report>,
    /// No page could be made.
    pub failed: bool,
}

/// Guidedog's own interface words in the build's language ("Index", "Search", ...),
/// for renderers and templates. Implemented by gd-intl's translator; [`EnglishUi`]
/// gives the words as written. `Sync`, as rendering threads share it.
pub trait UiText: Sync {
    /// The translation of `message`, or `message` itself.
    fn gettext<'a>(&'a self, message: &'a str) -> Cow<'a, str>;

    /// The translation of `singular` or `plural` for the count `n`, as gettext's
    /// ngettext chooses; in English, `singular` for 1 and `plural` otherwise.
    fn ngettext<'a>(&'a self, singular: &'a str, plural: &'a str, n: u64) -> Cow<'a, str>;
}

/// The interface words untranslated.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnglishUi;

impl UiText for EnglishUi {
    fn gettext<'a>(&'a self, message: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(message)
    }

    fn ngettext<'a>(&'a self, singular: &'a str, plural: &'a str, n: u64) -> Cow<'a, str> {
        Cow::Borrowed(if n == 1 { singular } else { plural })
    }
}
