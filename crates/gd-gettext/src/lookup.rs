//! The gettext lookups every kind of catalog offers, and the forms they return.

use std::fmt;

use crate::plural::{PluralRule, plural_index};

/// The translated forms of one message, borrowed from the catalog that found them.
///
/// An editable [`crate::Catalog`] keeps each form apart in its pool; a zero-copy
/// [`crate::MoCatalog`] keeps them as the MO file does, joined by NUL. Both look the same
/// from here.
#[derive(Debug, Clone, Copy)]
pub struct Forms<'a> {
    repr: Repr<'a>,
}

#[derive(Debug, Clone, Copy)]
enum Repr<'a> {
    /// Forms as (start, length) ranges of a pool.
    Pool { pool: &'a str, ranges: &'a [(usize, usize)] },
    /// Forms joined by NUL, as an MO file stores a plural translation.
    Joined(&'a str),
}

impl<'a> Forms<'a> {
    pub(crate) fn from_pool(pool: &'a str, ranges: &'a [(usize, usize)]) -> Forms<'a> {
        Forms { repr: Repr::Pool { pool, ranges } }
    }

    pub(crate) fn from_joined(text: &'a str) -> Forms<'a> {
        Forms { repr: Repr::Joined(text) }
    }

    /// Returns the number of forms.
    pub fn len(&self) -> usize {
        match self.repr {
            Repr::Pool { ranges, .. } => ranges.len(),
            Repr::Joined(text) => text.split('\0').count(),
        }
    }

    /// Tells whether there are no forms.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns form `k` (0 is the singular), borrowed from the catalog, if it exists.
    pub fn get(&self, k: usize) -> Option<&'a str> {
        match self.repr {
            Repr::Pool { pool, ranges } => {
                ranges.get(k).map(|&(start, len)| &pool[start..start + len])
            }
            Repr::Joined(text) => text.split('\0').nth(k),
        }
    }

    /// Iterates the forms in order.
    pub fn iter(&self) -> impl Iterator<Item = &'a str> + 'a {
        let (pool, ranges, joined) = match self.repr {
            Repr::Pool { pool, ranges } => (pool, ranges, None),
            Repr::Joined(text) => ("", &[][..], Some(text)),
        };
        let from_pool = ranges.iter().map(move |&(start, len)| &pool[start..start + len]);
        from_pool.chain(joined.into_iter().flat_map(|text| text.split('\0')))
    }
}

/// The gettext lookups, for any catalog that can find a message's forms.
///
/// Implementors provide [`Lookup::find`] and [`Lookup::plural_rule`]; the C library's
/// gettext, ngettext, pgettext and npgettext follow from them. Every string returned is
/// borrowed from the catalog, or is the `id` passed in.
pub trait Lookup: fmt::Debug {
    /// Returns the translated forms of a message; `ctx` `None` means no context. A
    /// message without a translation is not found.
    fn find(&self, id: &str, ctx: Option<&str>) -> Option<Forms<'_>>;

    /// Returns the catalog's compiled Plural-Forms rule.
    fn plural_rule(&self) -> &PluralRule;

    /// Returns a message's singular translation when it has a non-empty one.
    fn translation(&self, id: &str, ctx: Option<&str>) -> Option<&str> {
        self.find(id, ctx)?.get(0).filter(|s| !s.is_empty())
    }

    /// gettext: returns the translation of `id`, or `id` itself.
    fn text<'a>(&'a self, id: &'a str) -> &'a str {
        self.translation(id, None).unwrap_or(id)
    }

    /// pgettext: returns the translation of `id` in context `ctx`, or `id` itself.
    fn with_context<'a>(&'a self, ctx: &str, id: &'a str) -> &'a str {
        self.translation(id, Some(ctx)).unwrap_or(id)
    }

    /// ngettext: returns the form for `n`, or the English singular (n == 1) or plural
    /// when there is no translation or the form is empty.
    fn plural<'a>(&'a self, id: &'a str, id_plural: &'a str, n: u64) -> &'a str {
        self.plural_with_context(None, id, id_plural, n)
    }

    /// npgettext: [`Lookup::plural`] with an optional context.
    fn plural_with_context<'a>(
        &'a self,
        ctx: Option<&str>,
        id: &'a str,
        id_plural: &'a str,
        n: u64,
    ) -> &'a str {
        let english = if n == 1 { id } else { id_plural };
        plural_form(self.plural_rule(), self.find(id, ctx), n).unwrap_or(english)
    }
}

/// Picks the form for `n` from found forms, when it is there and not empty.
pub(crate) fn plural_form<'a>(
    rule: &PluralRule,
    forms: Option<Forms<'a>>,
    n: u64,
) -> Option<&'a str> {
    forms?.get(plural_index(rule, n)).filter(|s| !s.is_empty())
}
