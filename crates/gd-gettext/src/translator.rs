//! Text domains: the explicit stand-in for the C library's global domain table.

use std::path::{Path, PathBuf};

use crate::catalog::Catalog;
use crate::error::Error;
use crate::lookup::{Forms, plural_form};

/// A catalog a translator serves for a domain.
#[derive(Debug, Clone)]
pub struct BoundCatalog {
    /// The text domain, e.g. `guidedog` or `sphinx`.
    pub domain: String,
    /// The catalog, owned by the translator.
    pub catalog: Catalog,
}

/// Serves several text domains, each from a chain of catalogs: the first bound catalog
/// that translates a message wins, as Sphinx searches `locale_dirs` in order before its
/// own catalogs. It stands in for the C library's global state (bindtextdomain,
/// textdomain, setlocale).
#[derive(Debug, Clone)]
pub struct Translator {
    /// The language catalogs are looked for, e.g. `pt_BR`.
    pub language: String,
    /// The current domain (textdomain); `messages` by default.
    pub domain: String,
    /// The bound catalogs, in search order.
    pub bound: Vec<BoundCatalog>,
}

impl Translator {
    /// Returns a translator for `language` with no catalogs bound, in the `messages`
    /// domain.
    pub fn new(language: &str) -> Translator {
        Translator { language: language.to_string(), domain: "messages".into(), bound: Vec::new() }
    }

    /// Binds a catalog to a domain after those already bound; the translator owns it.
    pub fn add_catalog(&mut self, domain: &str, catalog: Catalog) {
        self.bound.push(BoundCatalog { domain: domain.to_string(), catalog });
    }

    /// Sets the current domain, as textdomain does.
    pub fn text_domain(&mut self, domain: &str) {
        domain.clone_into(&mut self.domain);
    }

    /// Looks for `dir/<language>/LC_MESSAGES/<domain>.po`, then `.mo`, trying the
    /// language, then its base language (pt_BR, then pt), and binds the first catalog
    /// found. Returns `Ok(false)` when there is none; fails when the file found cannot
    /// be read or parsed.
    pub fn bind_text_domain(
        &mut self,
        domain: &str,
        dir: &Path,
        allow_fuzzy: bool,
    ) -> Result<bool, Error> {
        let Some(path) = find_catalog_file(dir, &self.language, domain) else {
            return Ok(false);
        };
        let mut cat = if path.extension().is_some_and(|e| e == "mo") {
            crate::load_mo(&path)?
        } else {
            crate::load_po(&path, allow_fuzzy)?
        };
        cat.allow_fuzzy = allow_fuzzy;
        self.add_catalog(domain, cat);
        Ok(true)
    }

    /// Returns the first bound catalog of a domain that translates `id`, and its forms.
    fn search(&self, domain: &str, id: &str, ctx: Option<&str>) -> Option<(&Catalog, Forms<'_>)> {
        self.bound
            .iter()
            .filter(|b| b.domain == domain)
            .find_map(|b| b.catalog.find(id, ctx).map(|forms| (&b.catalog, forms)))
    }

    /// dgettext: the translation of `id` in a domain, or `id`.
    pub fn domain_text<'a>(&'a self, domain: &str, id: &'a str) -> &'a str {
        self.domain_with_context_opt(domain, None, id)
    }

    /// dpgettext: [`Translator::domain_text`] in context `ctx`.
    pub fn domain_with_context<'a>(&'a self, domain: &str, ctx: &str, id: &'a str) -> &'a str {
        self.domain_with_context_opt(domain, Some(ctx), id)
    }

    fn domain_with_context_opt<'a>(
        &'a self,
        domain: &str,
        ctx: Option<&str>,
        id: &'a str,
    ) -> &'a str {
        let found = self.search(domain, id, ctx).and_then(|(_, forms)| forms.get(0));
        found.filter(|s| !s.is_empty()).unwrap_or(id)
    }

    /// dngettext: the form for `n` in a domain, or the English singular or plural.
    pub fn domain_plural<'a>(
        &'a self,
        domain: &str,
        id: &'a str,
        id_plural: &'a str,
        n: u64,
    ) -> &'a str {
        self.domain_plural_with_context(domain, None, id, id_plural, n)
    }

    /// dnpgettext: [`Translator::domain_plural`] with an optional context.
    pub fn domain_plural_with_context<'a>(
        &'a self,
        domain: &str,
        ctx: Option<&str>,
        id: &'a str,
        id_plural: &'a str,
        n: u64,
    ) -> &'a str {
        let found = self.search(domain, id, ctx);
        let form = found.and_then(|(cat, forms)| plural_form(cat, Some(forms), n));
        form.unwrap_or(if n == 1 { id } else { id_plural })
    }

    /// gettext in the current domain.
    pub fn text<'a>(&'a self, id: &'a str) -> &'a str {
        self.domain_text(&self.domain, id)
    }

    /// ngettext in the current domain.
    pub fn plural<'a>(&'a self, id: &'a str, id_plural: &'a str, n: u64) -> &'a str {
        self.domain_plural(&self.domain, id, id_plural, n)
    }

    /// pgettext in the current domain.
    pub fn with_context<'a>(&'a self, ctx: &str, id: &'a str) -> &'a str {
        self.domain_with_context(&self.domain, ctx, id)
    }

    /// npgettext in the current domain.
    pub fn plural_with_context<'a>(
        &'a self,
        ctx: &str,
        id: &'a str,
        id_plural: &'a str,
        n: u64,
    ) -> &'a str {
        self.domain_plural_with_context(&self.domain, Some(ctx), id, id_plural, n)
    }
}

/// Returns the path of a domain's catalog for a language,
/// `dir/<lang>/LC_MESSAGES/<domain>.po` (or `.mo`), trying each of
/// [`language_candidates`]; `None` when there is none.
pub fn find_catalog_file(dir: &Path, language: &str, domain: &str) -> Option<PathBuf> {
    language_candidates(language).into_iter().find_map(|lang| {
        [".po", ".mo"].into_iter().find_map(|ext| {
            let path = dir.join(&lang).join("LC_MESSAGES").join(format!("{domain}{ext}"));
            path.is_file().then_some(path)
        })
    })
}

/// Lists the catalog directory names to try for a language, most specific first:
/// `pt-BR` gives `pt-BR`, `pt_BR`, and `pt`; `sr@latin` gives it, then `sr`. Empty for
/// an empty language.
pub fn language_candidates(language: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(3);
    if language.is_empty() {
        return out;
    }
    out.push(language.to_string());
    let underscored = language.replace('-', "_");
    if underscored != language {
        out.push(underscored.clone());
    }
    let cut = underscored.find(['_', '@', '.']).unwrap_or(underscored.len());
    if cut > 0 && cut < underscored.len() {
        out.push(underscored[..cut].to_string());
    }
    out
}
