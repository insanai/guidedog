//! Settings of the block reader that inline parsing and resolution need.

/// Typographic quotation marks, apostrophes, dashes, and ellipses (Docutils'
/// `smart_quotes` setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SmartQuotes {
    /// Text keeps its straight quotes.
    #[default]
    Off,
    /// The language's primary quotation marks outside, secondary inside.
    On,
    /// The language's alternative marks, where it has them.
    Alt,
}

/// The settings of the block reader (its `Settings`) that inline parsing and resolution
/// need. `Default` gives Docutils' defaults: bibliographic fields on, everything else
/// off or empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Recognize "PEP n" in text as a link to the PEP.
    pub pep_references: bool,
    /// Recognize "RFC n" in text as a link to the RFC.
    pub rfc_references: bool,
    /// Base URL of PEPs; "" means `https://peps.python.org/`.
    pub pep_base_url: String,
    /// Base URL of RFCs; "" means `https://datatracker.ietf.org/doc/html/`.
    pub rfc_base_url: String,
    /// Remove the white space before footnote and citation references.
    pub trim_footnote_reference_space: bool,
    /// Typographic quotes.
    pub smart_quotes: SmartQuotes,
    /// Quote overrides, "tag:“”‘’,tag:...".
    pub smart_quotes_locales: String,
    /// The document language tag ("" means English).
    pub language: String,
    /// Turn a leading field list into bibliographic fields.
    pub docinfo: bool,
    /// Promote a lone subsection title to the section's subtitle.
    pub sectsubtitle: bool,
    /// Character-level inline markup: no context is needed around start- and
    /// end-strings.
    pub character_level: bool,
    /// Tab stops of the main source; 0 means 8.
    pub tab_width: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
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
            character_level: false,
            tab_width: 0,
        }
    }
}
