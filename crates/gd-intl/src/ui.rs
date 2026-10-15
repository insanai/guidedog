//! Guidedog's own words in the build's language: gd-env's `UiText` over the
//! `guidedog` text domain (gd-i18n's chain of project, `sphinx.po` and built-in catalogs).

use std::borrow::Cow;
use std::path::Path;

use gd_env::UiText;
use gd_gettext::Translator;
use gd_i18n::{UI_DOMAIN, UiProblem, fill_labels, ui_translator};
use guidedoc::Labels;

/// The translator of Guidedog's interface words ("Index", "Next", "Fig. %s", ...).
/// Renderers and templates take it as `&dyn UiText`; it is `Sync`, so rendering
/// threads share one.
#[derive(Debug)]
pub struct UiTranslator {
    translator: Translator,
}

impl UiTranslator {
    /// The words in English: no catalog at all.
    pub fn english() -> UiTranslator {
        UiTranslator { translator: Translator::new("en") }
    }

    /// Loads the words of `language` from the project's locale folders `dirs`
    /// (`guidedog.po`, then `sphinx.po`, each `.po` or `.mo`), then from the catalog
    /// built into Guidedog. A project catalog that cannot be read is left out and
    /// returned among the problems; the others still apply. Fuzzy translations count
    /// when `allow_fuzzy` is set.
    pub fn load(
        language: &str,
        dirs: &[&Path],
        allow_fuzzy: bool,
    ) -> (UiTranslator, Vec<UiProblem>) {
        let (translator, problems) = ui_translator(language, dirs, allow_fuzzy);
        (UiTranslator { translator }, problems)
    }

    /// The underlying gettext translator, borrowed.
    pub fn translator(&self) -> &Translator {
        &self.translator
    }

    /// Every renderer label translated, for `RenderOptions::labels`.
    pub fn labels(&self) -> Labels {
        fill_labels(&self.translator)
    }
}

impl Default for UiTranslator {
    fn default() -> UiTranslator {
        UiTranslator::english()
    }
}

impl UiText for UiTranslator {
    fn gettext<'a>(&'a self, message: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(self.translator.domain_text(UI_DOMAIN, message))
    }

    fn ngettext<'a>(&'a self, singular: &'a str, plural: &'a str, n: u64) -> Cow<'a, str> {
        Cow::Borrowed(self.translator.domain_plural(UI_DOMAIN, singular, plural, n))
    }
}
