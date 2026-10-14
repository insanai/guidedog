//! msgmerge: updating a translation to a new template; statistics and sorting.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::catalog::{Catalog, Message, Previous, message_key};
use crate::fuzzy::FuzzyIndex;

/// msgmerge's threshold: a changed message inherits the translation of the most similar
/// old message when their similarity is at least this.
pub const FUZZY_THRESHOLD: f64 = 0.6;

/// How [`merge`] works. The default is msgmerge's: fuzzy matching on, obsolete messages
/// kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeOptions {
    /// Look for similar messages (msgmerge without `-N`).
    pub fuzzy_matching: bool,
    /// Keep translated messages that left the template as `#~`.
    pub keep_obsolete: bool,
}

impl Default for MergeOptions {
    fn default() -> MergeOptions {
        MergeOptions { fuzzy_matching: true, keep_obsolete: true }
    }
}

/// What a merge's fuzzy search did; [`merge_counted`] returns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MergeStats {
    /// msgids searched for (each distinct msgid once).
    pub searches: usize,
    /// Candidates whose byte counts were compared (the cheap bound).
    pub bounds: usize,
    /// Candidates compared in full ([`similarity`]).
    pub comparisons: usize,
    /// Bytes those comparisons read.
    pub work: usize,
    /// Searches the work bound stopped before they had looked everywhere.
    pub cut_short: usize,
}

/// Updates a translation (`def`) to a new template (`reference`), as msgmerge and
/// sphinx-intl update do, and returns the result as a new catalog; both inputs are
/// borrowed. The result has the template's messages in the template's order, with its
/// locations, notes, and flags:
///
/// - a message found in `def` keeps its translation, translator comments, and fuzzy flag;
/// - a new message similar to an old translated one gets that translation, marked fuzzy,
///   with the old msgid kept as its previous msgid (`#|`);
/// - old translated messages the template lost become obsolete (`#~`).
///
/// The header is `def`'s, with the template's POT-Creation-Date.
pub fn merge(def: &Catalog, reference: &Catalog, options: &MergeOptions) -> Catalog {
    merge_counted(def, reference, options).0
}

/// [`merge`], also returning what its fuzzy search did.
pub fn merge_counted(
    def: &Catalog,
    reference: &Catalog,
    options: &MergeOptions,
) -> (Catalog, MergeStats) {
    let mut out = Catalog::new(&def.name);
    out.allow_fuzzy = def.allow_fuzzy;
    out.plural = def.plural.clone();
    out.header = def.header.clone().or_else(|| reference.header.clone());
    if let (Some(date), true) = (reference.header_field("POT-Creation-Date"), def.header.is_some())
    {
        out.set_header_field("POT-Creation-Date", date);
    }
    let mut keys: HashMap<String, usize> = HashMap::with_capacity(def.messages.len());
    for (i, m) in def.messages.iter().enumerate() {
        keys.entry(message_key(m)).or_insert(i);
    }
    let mut used = vec![false; def.messages.len()];
    let mut fuzzy = options.fuzzy_matching.then(|| FuzzyIndex::new(def, reference, &keys));
    out.messages.reserve(reference.messages.len());
    for r in reference.messages.iter().filter(|r| !r.obsolete) {
        let mut m = Message { comments: Vec::new(), fuzzy: false, previous: None, ..r.clone() };
        let found = keys.get(&message_key(r)).map(|&i| (i, false));
        let similar = || fuzzy.as_mut().and_then(|ix| ix.search(&r.id)).map(|j| (j, true));
        if let Some((i, is_fuzzy)) = found.or_else(similar) {
            used[i] = true;
            take_translation(&mut m, &def.messages[i], is_fuzzy);
        }
        fit_forms(&mut m, out.plural.nplurals);
        out.messages.push(m);
    }
    if options.keep_obsolete {
        keep_lost_translations(&mut out, def, &used);
    }
    (out, fuzzy.map(|ix| ix.stats).unwrap_or_default())
}

/// Appends the translated messages of `def` that no template message used, as obsolete
/// (`#~`) entries without locations, so their translations stay for reuse.
fn keep_lost_translations(out: &mut Catalog, def: &Catalog, used: &[bool]) {
    for (d, &was_used) in def.messages.iter().zip(used) {
        if !was_used && d.is_translated() {
            out.messages.push(Message { obsolete: true, references: Vec::new(), ..d.clone() });
        }
    }
}

/// Copies `d`'s translation into `m`; `fuzzy` marks a similar match.
fn take_translation(m: &mut Message, d: &Message, fuzzy: bool) {
    m.strs.clone_from(&d.strs);
    m.comments.extend(d.comments.iter().cloned());
    m.fuzzy = d.fuzzy || fuzzy || d.obsolete;
    if fuzzy || d.is_plural() != m.is_plural() {
        m.fuzzy = true;
        m.previous = Some(Previous {
            ctx: d.ctx.clone(),
            id: d.id.clone(),
            id_plural: d.id_plural.clone(),
        });
    } else if d.fuzzy && d.previous.is_some() {
        m.previous.clone_from(&d.previous);
    }
}

/// Gives a message as many forms as it needs: one, or `nplurals`.
fn fit_forms(m: &mut Message, nplurals: usize) {
    let want = if m.is_plural() { nplurals } else { 1 };
    if m.is_plural() && !m.strs.is_empty() && m.strs.len() != want && m.is_translated() {
        m.fuzzy = true;
    }
    if m.strs.len() < want {
        m.strs.resize(want, String::new());
    }
    if !m.is_plural() && m.strs.len() > 1 {
        m.strs.truncate(1);
        m.fuzzy = m.fuzzy || !m.strs[0].is_empty();
    }
}

/// Counts of a catalog's messages, as sphinx-intl stat reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// Translated and not fuzzy.
    pub translated: usize,
    /// Translated but fuzzy.
    pub fuzzy: usize,
    /// Missing a form.
    pub untranslated: usize,
    /// Obsolete (`#~`).
    pub obsolete: usize,
}

/// Counts a catalog's messages as sphinx-intl stat does; the header does not count.
pub fn stats(cat: &Catalog) -> Stats {
    let mut s = Stats::default();
    for m in &cat.messages {
        match (m.obsolete, m.is_translated(), m.fuzzy) {
            (true, _, _) => s.obsolete += 1,
            (false, true, true) => s.fuzzy += 1,
            (false, true, false) => s.translated += 1,
            (false, false, _) => s.untranslated += 1,
        }
    }
    s
}

/// Returns sphinx-intl's summary line: `3 translated, 1 fuzzy, 2 untranslated.`
pub fn format_stats(s: &Stats) -> String {
    format!("{} translated, {} fuzzy, {} untranslated.", s.translated, s.fuzzy, s.untranslated)
}

/// Orders messages by their first location: file name, then line number as a number,
/// then msgid; messages without locations go last. Obsolete messages keep their place
/// after the live ones. Each message's own locations are sorted the same way, as Sphinx
/// sorts them.
pub fn sort_by_reference(cat: &mut Catalog) {
    cat.drop_index();
    for m in &mut cat.messages {
        m.references.sort_by(|a, b| reference_cmp(a, b));
    }
    cat.messages.sort_by(|a, b| {
        a.obsolete.cmp(&b.obsolete).then_with(|| {
            match (a.references.first(), b.references.first()) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(x), Some(y)) if x != y => reference_cmp(x, y),
                _ => a.id.cmp(&b.id),
            }
        })
    });
}

fn reference_cmp(a: &str, b: &str) -> Ordering {
    split_reference(a).cmp(&split_reference(b))
}

/// Splits `file:line` into the file and the line number (0 without one).
fn split_reference(r: &str) -> (&str, i64) {
    match r.rsplit_once(':') {
        Some((file, line)) => match line.parse() {
            Ok(n) => (file, n),
            Err(_) => (r, 0),
        },
        None => (r, 0),
    }
}

/// `2·M / (|a| + |b|)`, where M is the length of the longest common subsequence of
/// bytes: 1 for equal strings, 0 for strings with nothing in common. It is the measure
/// msgmerge's fstrcmp approximates. Long strings are compared by their first 2000 bytes.
pub fn similarity(a: &str, b: &str) -> f64 {
    let mut row = Vec::new();
    crate::fuzzy::lcs_ratio(crate::fuzzy::prefix(a), crate::fuzzy::prefix(b), &mut row)
}
