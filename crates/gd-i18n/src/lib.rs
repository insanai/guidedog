//! Translating Guidedoc documents the way Sphinx does with gettext: the translatable
//! messages of a document are extracted into a POT template (Sphinx's gettext builder),
//! and a pass replaces each message with its translation from a catalog (Sphinx's
//! Locale transform), re-parsing the translation as inline markup of the document's own
//! language. Guidedog's own words (labels, page furniture) come from the `guidedog`
//! text domain, with catalogs built in; dates are formatted with CLDR names.
//!
//! The crate depends on the engine and on `gd-gettext` only; the reader that parses
//! translations is handed in (`guidedoc::Reader::read_inline`).
//!
//! ```
//! use gd_i18n::{Template, ExtractOptions, PotHeader, domain_for, Compact};
//! let mut ws = guidedoc::Workspace::new();
//! # let reader = guidedoc::Reader {
//! #     id: "x", name: "x", version: "1", conformance: guidedoc::Conformance::Partial,
//! #     read: |_| Ok(()), read_inline: None };
//! # let request = guidedoc::default_request("index.rst", "", "x", "html");
//! let doc = guidedoc::read_document(&mut ws, &reader, &request).expect("read");
//! let mut pot = Template::new(domain_for("index", &Compact::default()));
//! pot.extract(doc, &ExtractOptions { location: true, ..Default::default() });
//! pot.set_header(&PotHeader { project: "Demo", ..Default::default() });
//! assert!(pot.write().contains("Project-Id-Version: Demo"));
//! ```

mod apply;
mod date_names;
mod dates;
mod extract;
mod figures;
mod messages;
mod refs;
mod translate;
mod ui;
mod units;

pub use dates::{Moment, format_date};
pub use extract::{Compact, ExtractOptions, PotHeader, Template, domain_for};
pub use figures::{
    DEFAULT_FIGURE_FILENAME, FigureFormatError, ImageExists, ImageLocalization, figure_filename,
    figure_for_language, write_figure_filename,
};
pub use messages::{I18N_MARKUP, I18N_REFERENCES, I18N_UNRESOLVED};
pub use translate::{
    Progress, TRANSLATION_PASS, Translation, TranslationCounts, TranslationPass,
    translate_document,
};
pub use ui::{
    UI_DOMAIN, UI_TEMPLATE, UiProblem, embedded_catalog, fill_labels, ui_text, ui_translator,
};
pub use units::{
    IndexPart, Target, Targets, Unit, index_parts, parse_target, unit_of, write_message,
};
