//! The search index (the index side of search.odin). Reading keeps each document's
//! words by section in a file beside its object (`search/<docname>.<digest16>.txt`:
//! one line per section, `anchor\ttitle\ttext`). The HTML builders join them into an
//! inverted index, `_static/searchindex.js`, which the theme's script queries without
//! reading any page's text:
//!
//! ```text
//! window.GUIDEDOG_INDEX = {
//!   "pages": [[title, url], ...],
//!   "sections": [[page, title, anchor, excerpt], ...],  // sections, then objects
//!   "terms": {"word": [section, count, section, count, ...], ...},
//!   "titles": {"word": [section, ...], ...},
//!   "stopwords": ["the", ...]};
//! ```
//!
//! Words are split as [`tokenize`] splits them, which the script repeats for the query.

use std::collections::{BTreeMap, HashMap};

use gd_env::{Digest, Digester, DocId, DocInfo};
use gd_host::Problem;
use gd_outputs::{Generation, UnitRecord};
use gd_report::Report;
use gd_resolve::BuildView;

use crate::escape::push_js_string;

/// Characters of a section shown under its search result.
const SEARCH_EXCERPT: usize = 180;

/// The unit key of the search index.
const SEARCH_UNIT: &str = "search";

/// Where the index is written, below the output folder.
pub(crate) const SEARCH_INDEX: &str = "_static/searchindex.js";

/// English stop words, left out of the index as Sphinx leaves them out.
pub const STOPWORDS_EN: [&str; 33] = [
    "a", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
    "near", "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there",
    "these", "they", "this", "to", "was", "will", "with",
];

/// Calls `word` with each word of `text`, lowercased: runs of letters (Unicode category
/// L), decimal digits and underscores, with each Chinese or Japanese character a word
/// of its own. The theme's script splits a query the same way.
pub fn tokenize(text: &str, mut word: impl FnMut(&str)) {
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if ideographic(c) {
            if let Some(s) = start.take() {
                word(&text[s..i].to_lowercase());
            }
            word(&text[i..i + c.len_utf8()]);
        } else if guidedoc::is_letter(c) || guidedoc::is_decimal_digit(c) || c == '_' {
            start.get_or_insert(i);
        } else if let Some(s) = start.take() {
            word(&text[s..i].to_lowercase());
        }
    }
    if let Some(s) = start {
        word(&text[s..].to_lowercase());
    }
}

/// The scripts written without spaces between words, whose every character is a
/// word: Han, Hiragana and Katakana. The script tests the same ranges.
fn ideographic(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

/// Writes the search index as a unit of its own, kept while no document's words,
/// title, address or objects changed. A document whose words cannot be read is left out
/// of the index with a warning.
pub(crate) fn write_search(
    view: &BuildView<'_>,
    generation: &mut Generation,
    reports: &mut Vec<Report>,
) -> Result<(), Problem> {
    let inputs = search_inputs(view);
    if let Some(old) = generation.reuse(SEARCH_UNIT, inputs, |_| Digest::default()) {
        reports.extend(old.reports.iter().cloned());
        return Ok(());
    }
    let mut own = Vec::new();
    let text = search_index(view, &mut own);
    let mut unit = generation.unit(SEARCH_UNIT);
    unit.emit_text(SEARCH_INDEX, &text)?;
    reports.extend(own.iter().cloned());
    unit.finish(UnitRecord { inputs, reports: own, ..UnitRecord::default() });
    Ok(())
}

/// What the index is made of: the output settings and the language; per document read,
/// its name, title, URL and words; per object its full name, type and anchor.
fn search_inputs(view: &BuildView<'_>) -> Digest {
    let mut d = Digester::new();
    d.digest(&view.settings).text(&view.config.language);
    for (_, doc) in view.env.iter().filter(|(_, doc)| !doc.failed) {
        d.text(&doc.docname).text(&doc.title).text(&view.page_name(&doc.docname));
        d.digest(&doc.search.as_ref().map_or(Digest::default(), |s| s.digest));
        for o in &doc.objects {
            d.text(&o.fullname).text(&o.objtype).text(&o.anchor);
        }
    }
    d.finish()
}

/// The inverted index being built.
#[derive(Default)]
struct SearchIndex {
    stopwords: &'static [&'static str],
    /// The sections, written as JSON arrays.
    sections: String,
    /// Sections written.
    count: u32,
    /// Per word: section, occurrences, section, occurrences, ...
    terms: BTreeMap<String, Vec<u32>>,
    /// Per word: the sections whose titles hold it.
    titles: BTreeMap<String, Vec<u32>>,
    /// The section being indexed: word -> occurrences.
    counts: HashMap<String, u32>,
}

/// The text of `_static/searchindex.js`.
fn search_index(view: &BuildView<'_>, reports: &mut Vec<Report>) -> String {
    let language = &view.config.language;
    let mut ix = SearchIndex::default();
    if language.is_empty() || language.starts_with("en") {
        ix.stopwords = &STOPWORDS_EN;
    }
    let mut pages = String::new();
    let read = view.env.iter().filter(|(_, d)| !d.failed);
    for (page, (id, d)) in (0u32..).zip(read) {
        if page > 0 {
            pages.push(',');
        }
        pages.push('[');
        push_js_string(&mut pages, &d.title);
        pages.push(',');
        push_js_string(&mut pages, &view.page_name(&d.docname));
        pages.push(']');
        let words = document_words(view, d, reports);
        ix.add_document(view, id, d, page, &words);
    }
    let mut out = String::with_capacity(pages.len() + ix.sections.len() * 2);
    out.push_str("window.GUIDEDOG_INDEX = {\"pages\":[");
    out.push_str(&pages);
    out.push_str("],\"sections\":[");
    out.push_str(&ix.sections);
    out.push_str("],\"terms\":");
    write_postings(&mut out, &ix.terms);
    out.push_str(",\"titles\":");
    write_postings(&mut out, &ix.titles);
    out.push_str(",\"stopwords\":[");
    for (i, w) in ix.stopwords.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_js_string(&mut out, w);
    }
    out.push_str("]};\n");
    out
}

/// A document's search words, read back from the store; empty, with a warning, when
/// they cannot be read.
fn document_words(view: &BuildView<'_>, d: &DocInfo, reports: &mut Vec<Report>) -> String {
    let Some(file) = &d.search else { return String::new() };
    match view.objects.load_bytes(file) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => {
            reports.push(Report {
                path: d.relative.clone(),
                ..Report::warning(
                    "build.search",
                    "SEARCH WORDS MISSING",
                    &format!(
                        "The words of {} could not be read, so the search index leaves the \
                         page out: {}",
                        d.docname, e.problem.message
                    ),
                )
                .hint("Rebuild with guidedog build -E.")
            });
            String::new()
        }
    }
}

impl SearchIndex {
    /// Indexes a document's sections, from its search words, and its objects, whose
    /// names are searched as titles.
    fn add_document(
        &mut self,
        view: &BuildView<'_>,
        id: DocId,
        d: &DocInfo,
        page: u32,
        words: &str,
    ) {
        for line in words.lines() {
            let mut parts = line.splitn(3, '\t');
            let anchor = parts.next().unwrap_or("");
            let title = parts.next().unwrap_or("");
            let text = parts.next().unwrap_or("");
            let shown = if title.is_empty() { &d.title } else { title };
            self.add_section(page, shown, &view.page_anchor(id, anchor), &excerpt(text));
            self.add_words(text, false);
            self.add_words(title, true);
        }
        for o in d.objects.iter().filter(|o| !o.aliased) {
            // As in Sphinx, aliases are not searched.
            self.add_section(page, &o.fullname, &view.page_anchor(id, &o.anchor), &o.objtype);
            self.add_words(&o.fullname, true);
        }
    }

    fn add_section(&mut self, page: u32, title: &str, anchor: &str, excerpt: &str) {
        if self.count > 0 {
            self.sections.push(',');
        }
        self.sections.push_str(&format!("[{page},"));
        push_js_string(&mut self.sections, title);
        self.sections.push(',');
        push_js_string(&mut self.sections, anchor);
        self.sections.push(',');
        push_js_string(&mut self.sections, excerpt);
        self.sections.push(']');
        self.count += 1;
    }

    /// Adds the words of the latest section's text, or of its title, to the index.
    fn add_words(&mut self, text: &str, title: bool) {
        self.counts.clear();
        let (stopwords, counts) = (self.stopwords, &mut self.counts);
        tokenize(text, |w| {
            let single = w.chars().nth(1).is_none();
            if single && !w.chars().next().is_some_and(ideographic) {
                return;
            }
            if stopwords.contains(&w) {
                return;
            }
            *counts.entry(w.to_string()).or_insert(0) += 1;
        });
        let section = self.count - 1;
        for (w, n) in self.counts.drain() {
            if title {
                self.titles.entry(w).or_default().push(section);
            } else {
                self.terms.entry(w).or_default().extend([section, n]);
            }
        }
    }
}

/// Writes a word map in word order, so the file is the same every build.
fn write_postings(out: &mut String, map: &BTreeMap<String, Vec<u32>>) {
    out.push('{');
    for (i, (w, postings)) in map.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_js_string(out, w);
        out.push_str(":[");
        for (j, n) in postings.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&n.to_string());
        }
        out.push(']');
    }
    out.push('}');
}

/// The first `SEARCH_EXCERPT` characters of a section's text, with an ellipsis when
/// it is longer.
fn excerpt(text: &str) -> String {
    match text.char_indices().nth(SEARCH_EXCERPT) {
        None => text.to_string(),
        Some((end, _)) => format!("{}...", text[..end].trim()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        tokenize(text, |w| out.push(w.to_string()));
        out
    }

    #[test]
    fn words_are_lowercased_runs_and_single_ideographs() {
        assert_eq!(words("Hello, World_2 x"), ["hello", "world_2", "x"]);
        assert_eq!(words("Über-Größe"), ["über", "größe"]);
        assert_eq!(words("abc日本def"), ["abc", "日", "本", "def"]);
        assert!(words(" -- ").is_empty());
    }

    #[test]
    fn short_excerpts_stay_and_long_ones_end_in_an_ellipsis() {
        assert_eq!(excerpt("short"), "short");
        let long = "word ".repeat(100);
        let e = excerpt(&long);
        assert!(e.ends_with("..."));
        assert_eq!(e.chars().count(), 182);
    }

    #[test]
    fn postings_count_words_and_skip_stopwords_and_single_letters() {
        let mut ix = SearchIndex { stopwords: &STOPWORDS_EN, ..SearchIndex::default() };
        ix.add_section(0, "Intro", "intro", "");
        ix.add_words("the build builds a build", false);
        ix.add_words("Intro", true);
        assert_eq!(ix.terms.get("build"), Some(&vec![0, 2]));
        assert_eq!(ix.terms.get("builds"), Some(&vec![0, 1]));
        assert!(!ix.terms.contains_key("the") && !ix.terms.contains_key("a"));
        assert_eq!(ix.titles.get("intro"), Some(&vec![0]));
        let mut out = String::new();
        write_postings(&mut out, &ix.terms);
        assert_eq!(out, "{\"build\":[0,2],\"builds\":[0,1]}");
    }
}
