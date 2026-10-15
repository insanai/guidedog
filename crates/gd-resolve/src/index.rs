//! The data of the general index and the module index (the models of genindex.odin
//! and modindex.odin), and objects.inv (inventory.odin). The pages are written by
//! gd-site; the PDF book's index by gd-book.

use std::collections::HashMap;

use crate::view::{BuildView, python_format};

/// A link of an index term to a page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexLink {
    /// A main entry (`!` in the index directive), shown in bold.
    pub main: bool,
    /// The URL from the output root (or the book's anchor key).
    pub uri: String,
}

/// A subterm of an index term.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexSub {
    /// Its text.
    pub name: String,
    /// Its links.
    pub links: Vec<IndexLink>,
}

/// A term of the general index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexTerm {
    /// Its text.
    pub name: String,
    /// The category key a glossary term gives; empty for none.
    pub key: String,
    /// Its links.
    pub links: Vec<IndexLink>,
    /// Its subterms, sorted.
    pub subs: Vec<IndexSub>,
}

/// A group of the general index: a letter, `_`, "Symbols" (translated), or a category
/// key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexGroup {
    /// The group's heading.
    pub name: String,
    /// Its terms, sorted as Sphinx sorts them.
    pub terms: Vec<IndexTerm>,
}

/// Collects the project's index entries into groups of sorted terms, as Sphinx's
/// `IndexEntries.create_index` does: entry types `single`, `pair`, `triple`, `see`,
/// `seealso` and the pair shorthands (`module:`, `keyword:`, ...); terms folded as
/// `f() (in module a)` under `f()`.
pub fn create_index(view: &BuildView<'_>) -> Vec<IndexGroup> {
    let mut ib = IndexBuilder { view, terms: Vec::new(), by_name: HashMap::new() };
    for (id, d) in view.env.iter() {
        let page = view.page_name(&d.docname);
        for ix in &d.index {
            let targets: Vec<&str> = ix.targets.lines().collect();
            for (n, line) in ix.entries.lines().enumerate() {
                let target =
                    targets.get(n).copied().filter(|t| !t.is_empty()).unwrap_or(&ix.anchor);
                let uri = format!("{page}#{}", view.page_anchor(id, target));
                for e in process_index_entry(view, line) {
                    ib.add_entry(&e, &uri, &ix.key);
                }
            }
        }
    }
    let mut terms = ib.terms;
    for t in &mut terms {
        sort_links(&mut t.links);
        t.subs.iter_mut().for_each(|s| sort_links(&mut s.links));
    }
    sort_terms(&mut terms);
    let mut folded = fold_terms(terms);
    folded.iter_mut().for_each(|t| sort_subs(&mut t.subs));
    group_terms(view, folded)
}

/// The entry types Sphinx reads as a pair with a label.
const PAIR_INDEX_TYPES: [(&str, &str); 7] = [
    ("module", "module"),
    ("keyword", "keyword"),
    ("operator", "operator"),
    ("object", "object"),
    ("exception", "exception"),
    ("statement", "statement"),
    ("builtin", "built-in function"),
];

const INDEX_TYPES: [&str; 6] = ["single", "pair", "double", "triple", "see", "seealso"];

/// One entry of an index node, as Sphinx's `process_index_entry` gives it.
#[derive(Debug)]
struct IndexLine {
    kind: &'static str,
    value: String,
    main: bool,
}

/// Reads one written entry: `type: value`, an old pair type such as `module: name`,
/// or the shorthand of comma-separated single entries.
fn process_index_entry(view: &BuildView<'_>, raw: &str) -> Vec<IndexLine> {
    let entry = raw.trim();
    if entry.is_empty() {
        return Vec::new();
    }
    let main = entry.starts_with('!');
    let typed = if main { entry[1..].trim_start() } else { entry };
    for (name, label) in PAIR_INDEX_TYPES {
        if let Some(value) = typed.strip_prefix(name).and_then(|r| r.strip_prefix(':')) {
            let label = view.ui.gettext(label);
            return vec![IndexLine {
                kind: "pair",
                value: format!("{label}; {}", value.trim()),
                main,
            }];
        }
    }
    for kind in INDEX_TYPES {
        if let Some(value) = typed.strip_prefix(kind).and_then(|r| r.strip_prefix(':')) {
            let kind = if kind == "double" { "pair" } else { kind };
            return vec![IndexLine { kind, value: value.trim().to_string(), main }];
        }
    }
    // Shorthand: each comma-separated part of the entry as written is a single entry.
    entry
        .split(',')
        .filter_map(|part| {
            let value = part.trim();
            let main = value.starts_with('!');
            let value = if main { value[1..].trim_start() } else { value };
            (!value.is_empty()).then(|| IndexLine { kind: "single", value: value.into(), main })
        })
        .collect()
}

/// Splits a value at its first `n - 1` semicolons, as Sphinx's `_split_into`.
fn split_into(value: &str, n: usize) -> [String; 3] {
    let mut out: [String; 3] = Default::default();
    let mut rest = value;
    for (i, slot) in out.iter_mut().enumerate().take(n) {
        if i == n - 1 {
            *slot = rest.trim().to_string();
            break;
        }
        match rest.split_once(';') {
            Some((head, tail)) => {
                *slot = head.trim().to_string();
                rest = tail;
            }
            None => {
                *slot = rest.trim().to_string();
                break;
            }
        }
    }
    out
}

struct IndexBuilder<'v, 'b> {
    view: &'v BuildView<'b>,
    terms: Vec<IndexTerm>,
    by_name: HashMap<String, usize>,
}

impl IndexBuilder<'_, '_> {
    /// Adds the terms of one entry, as Sphinx's `create_index` does per entry type.
    fn add_entry(&mut self, e: &IndexLine, uri: &str, key: &str) {
        let [first, second, third] = split_into(&e.value, if e.kind == "triple" { 3 } else { 2 });
        match e.kind {
            "single" => self.add_term(&first, &second, e.main, uri, key),
            "pair" => {
                self.add_term(&first, &second, e.main, uri, key);
                self.add_term(&second, &first, e.main, uri, key);
            }
            "triple" => {
                self.add_term(&first, &format!("{second} {third}"), e.main, uri, key);
                self.add_term(&second, &format!("{third}, {first}"), e.main, uri, key);
                self.add_term(&third, &format!("{first} {second}"), e.main, uri, key);
            }
            _ => {
                let label = if e.kind == "see" { "see %s" } else { "see also %s" };
                let see = python_format(&self.view.ui.gettext(label), &[&second]);
                self.add_term(&first, &see, false, "", key);
            }
        }
    }

    /// Sphinx's `_add_entry`: the term, then its subterm, gets the link.
    fn add_term(&mut self, word: &str, sub: &str, main: bool, uri: &str, key: &str) {
        if word.is_empty() {
            return;
        }
        let i = *self.by_name.entry(word.to_string()).or_insert_with(|| {
            self.terms.push(IndexTerm {
                name: word.into(),
                key: key.into(),
                ..IndexTerm::default()
            });
            self.terms.len() - 1
        });
        let term = &mut self.terms[i];
        let links = if sub.is_empty() {
            &mut term.links
        } else {
            let at = match term.subs.iter().position(|s| s.name == sub) {
                Some(at) => at,
                None => {
                    term.subs.push(IndexSub { name: sub.into(), links: Vec::new() });
                    term.subs.len() - 1
                }
            };
            &mut term.subs[at].links
        };
        if !uri.is_empty() {
            links.push(IndexLink { main, uri: uri.into() });
        }
    }
}

/// Main links first, then by URL.
fn sort_links(links: &mut [IndexLink]) {
    links.sort_by(|a, b| b.main.cmp(&a.main).then_with(|| a.uri.cmp(&b.uri)));
}

/// Sphinx's comparison form: NFD of the lowercase text, without a leading
/// right-to-left mark.
fn lower_nfd(s: &str) -> String {
    let lower = s.to_lowercase();
    let folded = guidedoc::normalize_nfd(&lower);
    folded.strip_prefix('\u{200f}').unwrap_or(&folded).to_string()
}

/// Whether text starts with a letter or `_`, as Sphinx tests with `str.isalpha`.
fn starts_alpha(s: &str) -> bool {
    s.chars().next().is_some_and(|r| guidedoc::is_letter(r) || r == '_')
}

/// Sphinx's `_key_func_1`: symbols before letters, then the lowercase NFD form of the
/// category key or the term, then the term itself.
fn sort_terms(terms: &mut [IndexTerm]) {
    terms.sort_by_cached_key(|t| {
        let folded = lower_nfd(if t.key.is_empty() { &t.name } else { &t.key });
        (starts_alpha(&folded), folded, t.name.clone())
    });
}

/// Sphinx's `_key_func_2`: letters after symbols, by lowercase NFD form.
fn sort_subs(subs: &mut [IndexSub]) {
    subs.sort_by_cached_key(|s| {
        let k = lower_nfd(&s.name);
        if starts_alpha(&k) { format!("\u{7f}{k}") } else { k }
    });
}

/// Folds `f() (in module b)` into the `f() (in module a)` before it, as a subterm
/// `(in module b)`, exactly as Sphinx's fixup loop does: only terms without subterms
/// move, and they join the subterms of the last term kept.
fn fold_terms(sorted: Vec<IndexTerm>) -> Vec<IndexTerm> {
    let mut out: Vec<IndexTerm> = Vec::with_capacity(sorted.len());
    let mut old_key = String::new();
    for t in sorted {
        if t.subs.is_empty() {
            if let Some((prefix, paren)) = parenthesized(&t.name) {
                if old_key == prefix {
                    if let Some(last) = out.last_mut() {
                        fold_into(last, paren, &t.links);
                    }
                    continue;
                }
                old_key = prefix.to_string();
            } else {
                old_key.clone_from(&t.name);
            }
        }
        out.push(t);
    }
    out
}

fn fold_into(t: &mut IndexTerm, paren: &str, links: &[IndexLink]) {
    match t.subs.iter_mut().find(|s| s.name == paren) {
        Some(s) => s.links.extend_from_slice(links),
        None => t.subs.push(IndexSub { name: paren.into(), links: links.to_vec() }),
    }
}

/// Sphinx's `(.*) ([(][^()]*[)])` at the start of `name`: the last ` (` whose group
/// closes before any other parenthesis.
fn parenthesized(name: &str) -> Option<(&str, &str)> {
    if name.contains('\n') {
        return None;
    }
    let bytes = name.as_bytes();
    for i in (0..bytes.len().saturating_sub(1)).rev() {
        if bytes[i] != b' ' || bytes[i + 1] != b'(' {
            continue;
        }
        for j in i + 2..bytes.len() {
            match bytes[j] {
                b'(' => break,
                b')' => return Some((&name[..i], &name[i + 1..=j])),
                _ => {}
            }
        }
    }
    None
}

/// Sphinx's `_key_func_3` and `groupby`: consecutive terms with the same category key,
/// first letter, or "Symbols" form a group.
fn group_terms(view: &BuildView<'_>, terms: Vec<IndexTerm>) -> Vec<IndexGroup> {
    let symbols = view.ui.gettext("Symbols");
    let mut groups: Vec<IndexGroup> = Vec::new();
    for t in terms {
        let name = group_name(&t, &symbols);
        match groups.last_mut() {
            Some(g) if g.name == name => g.terms.push(t),
            _ => groups.push(IndexGroup { name, terms: vec![t] }),
        }
    }
    groups
}

fn group_name(t: &IndexTerm, symbols: &str) -> String {
    if !t.key.is_empty() {
        return t.key.clone();
    }
    let first = guidedoc::nfd_first(t.name.strip_prefix('\u{200f}').unwrap_or(&t.name));
    let upper = first.to_uppercase().next().unwrap_or(first);
    if guidedoc::is_letter(upper) || upper == '_' {
        return upper.to_string();
    }
    symbols.to_string()
}
