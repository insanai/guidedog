//! The reader's dialect settings.

use guidedoc_rst_inline::Options;

pub use guidedoc_rst_inline::SmartQuotes;

/// The reader's settings. They reach `read` through `ReadOptions::extension`, made with
/// `guidedoc::read_extension(&settings)`; settings of another type are refused. Start
/// from `Settings::default()` (Docutils' defaults): the bibliographic fields are on.
///
/// Directive and role handlers (`Extension`) and a host markup (`Host`) are not
/// settings: a dialect passes them to `read_with` and `embed_begin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Implicit "PEP nnn" references in running text become links.
    pub pep_references: bool,
    /// Implicit "RFC nnn" references in running text become links.
    pub rfc_references: bool,
    /// "" means `https://peps.python.org/`.
    pub pep_base_url: String,
    /// "" means `https://datatracker.ietf.org/doc/html/`.
    pub rfc_base_url: String,
    /// White space before a footnote reference is removed.
    pub trim_footnote_reference_space: bool,
    /// Typographic quotation marks, apostrophes, dashes, and ellipses in text.
    pub smart_quotes: SmartQuotes,
    /// Quotation marks per language, overriding the built-in ones, as
    /// `"tag:“”‘’,tag2:«»‹›"`: primary open and close, then secondary open and close.
    pub smart_quotes_locales: String,
    /// BCP 47 tag of the document language: bibliographic field names, generated titles,
    /// and quotation marks. "" means "en".
    pub language: String,
    /// A field list first in the document becomes bibliographic data (docinfo).
    pub docinfo: bool,
    /// A lone subsection title becomes the section's subtitle.
    pub sectsubtitle: bool,
    /// Tab stops; zero means `ReadOptions::tab_width`, which zero makes 8.
    pub tab_width: usize,
    /// Inline markup is recognized inside words (Docutils'
    /// `character_level_inline_markup`).
    pub character_level_inline_markup: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            pep_references: false,
            rfc_references: false,
            pep_base_url: String::new(),
            rfc_base_url: String::new(),
            trim_footnote_reference_space: false,
            smart_quotes: SmartQuotes::Off,
            smart_quotes_locales: String::new(),
            language: String::new(),
            docinfo: true,
            sectsubtitle: false,
            tab_width: 0,
            character_level_inline_markup: false,
        }
    }
}

/// Returns Docutils' defaults: bibliographic fields on, everything else off or empty
/// (`Settings::default()`).
pub fn default_settings() -> Settings {
    Settings::default()
}

/// The settings inline parsing and resolution need, with tab stops `tab`.
pub(crate) fn inline_options(s: &Settings, tab: usize) -> Options {
    Options {
        pep_references: s.pep_references,
        rfc_references: s.rfc_references,
        pep_base_url: s.pep_base_url.clone(),
        rfc_base_url: s.rfc_base_url.clone(),
        trim_footnote_reference_space: s.trim_footnote_reference_space,
        smart_quotes: s.smart_quotes,
        smart_quotes_locales: s.smart_quotes_locales.clone(),
        language: s.language.clone(),
        docinfo: s.docinfo,
        sectsubtitle: s.sectsubtitle,
        character_level: s.character_level_inline_markup,
        tab_width: tab,
    }
}
