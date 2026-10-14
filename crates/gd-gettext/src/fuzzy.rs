//! The fuzzy search of [`crate::merge`].
//!
//! For a message the template added, it finds the old translated message whose msgid is
//! most similar ([`crate::similarity`]), if any reaches [`FUZZY_THRESHOLD`]. The answer is
//! the one a comparison with every old message gives (the highest score, the earliest
//! message on a tie), found without comparing every pair:
//!
//! - old messages with the same msgid (as compared: its first [`FUZZY_PREFIX`] bytes) are
//!   one candidate, the earliest; a msgid found among them scores 1 and needs no search;
//!   and a msgid is searched once however many messages the template has with it;
//! - candidates are sorted by length, and similarity is at most `2·min / (|a| + |b|)`, so
//!   a search walks out from the msgid's own length and stops where no longer or shorter
//!   candidate could win;
//! - an index of trigrams puts first the candidates that share the most of them with the
//!   msgid (a trigram most candidates share ranks nothing), so the best is usually found
//!   first and the byte-count bound turns most others away cheaply.
//!
//! A search still compares pairs, and a catalog can be made of messages that all score
//! alike. So a merge's fuzzy search reads at most [`FUZZY_WORK_PER_BYTE`] bytes per byte
//! of the two catalogs' msgids ([`FUZZY_MIN_WORK`] at least), shared among its searches,
//! each allowed at least a few comparisons of its own length: the work grows with the
//! catalogs, never with their square. A search the bound stops keeps the best it found,
//! and is counted in [`MergeStats::cut_short`]; catalogs people write never reach it.

use std::collections::HashMap;

use crate::catalog::{Catalog, Message};
use crate::merge::{FUZZY_THRESHOLD, MergeStats};

/// How many bytes of each msgid similarity compares.
pub(crate) const FUZZY_PREFIX: usize = 2000;
/// Bytes a merge's fuzzy search may read per byte of the two catalogs' msgids.
pub const FUZZY_WORK_PER_BYTE: usize = 256;
/// The least work a merge's fuzzy search is allowed.
pub const FUZZY_MIN_WORK: usize = 1 << 26;
/// A trigram more candidates than this share does not rank them.
const FUZZY_GRAM_LIMIT: usize = 64;

/// A candidate number: an index into [`FuzzyIndex::candidates`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CandidateId(u32);

#[derive(Debug, Clone, Copy)]
struct Candidate<'a> {
    id: &'a [u8],   // the msgid's first FUZZY_PREFIX bytes.
    message: usize, // the earliest old message with it.
}

/// The first `FUZZY_PREFIX` bytes of a msgid, as bytes: a cut may split a character.
pub(crate) fn prefix(s: &str) -> &[u8] {
    &s.as_bytes()[..s.len().min(FUZZY_PREFIX)]
}

/// The candidates of one merge, indexed, and what their searches spent.
#[derive(Debug)]
pub(crate) struct FuzzyIndex<'a> {
    candidates: Vec<Candidate<'a>>,          // by length, then message.
    first: HashMap<&'a [u8], usize>,         // msgid -> candidate message: a search that scores 1.
    grams: HashMap<u32, Vec<CandidateId>>,   // up to FUZZY_GRAM_LIMIT + 1 each.
    found: HashMap<&'a [u8], Option<usize>>, // msgid searched -> answer.
    seen: Vec<u32>,                          // per candidate: the search that last looked at it.
    shared: Vec<u32>, // per candidate: trigrams shared with the msgid searched.
    touched: Vec<CandidateId>, // scratch of rank_by_grams, reused.
    stamp: u32,
    row: Vec<u16>, // similarity's working row, reused.
    work_left: usize,
    searches_left: usize,
    pub(crate) stats: MergeStats,
}

struct Search<'q> {
    q: &'q [u8],
    best: Option<usize>,
    best_score: f64,
    allowance: usize,
    spent: usize,
    cut: bool,
}

fn gram(s: &[u8], i: usize) -> u32 {
    u32::from(s[i]) << 16 | u32::from(s[i + 1]) << 8 | u32::from(s[i + 2])
}

fn is_candidate(d: &Message) -> bool {
    !d.obsolete && d.is_translated() && !d.id.is_empty()
}

impl<'a> FuzzyIndex<'a> {
    /// Indexes `def`'s candidates for the messages of `reference` that `keys` (`def`'s
    /// messages by key) does not hold.
    pub(crate) fn new(
        def: &'a Catalog,
        reference: &'a Catalog,
        keys: &HashMap<String, usize>,
    ) -> FuzzyIndex<'a> {
        let mut bytes = 0;
        let mut searches_left = 0;
        for r in &reference.messages {
            if !r.obsolete && !keys.contains_key(&crate::catalog::message_key(r)) {
                searches_left += 1;
                bytes += prefix(&r.id).len();
            }
        }
        let mut first = HashMap::new();
        let mut candidates = Vec::new();
        for (i, d) in def.messages.iter().enumerate().filter(|(_, d)| is_candidate(d)) {
            let id = prefix(&d.id);
            if first.contains_key(id) {
                continue;
            }
            first.insert(id, i);
            candidates.push(Candidate { id, message: i });
            bytes += id.len();
        }
        candidates.sort_by_key(|c| (c.id.len(), c.message));
        let n = candidates.len();
        FuzzyIndex {
            grams: index_grams(&candidates),
            candidates,
            first,
            found: HashMap::new(),
            seen: vec![0; n],
            shared: vec![0; n],
            touched: Vec::new(),
            stamp: 0,
            row: Vec::with_capacity(FUZZY_PREFIX + 1),
            work_left: (FUZZY_WORK_PER_BYTE * bytes).max(FUZZY_MIN_WORK),
            searches_left,
            stats: MergeStats::default(),
        }
    }

    /// Returns the old message most like `id`, if one reaches the threshold.
    pub(crate) fn search(&mut self, id: &'a str) -> Option<usize> {
        let q = prefix(id);
        let share = self.work_left / self.searches_left.max(1);
        self.searches_left = self.searches_left.saturating_sub(1);
        if let Some(&answer) = self.found.get(q) {
            return answer;
        }
        if let Some(&exact) = self.first.get(q) {
            return Some(exact);
        }
        self.stats.searches += 1;
        self.stamp += 1;
        let mut s = Search {
            q,
            best: None,
            best_score: FUZZY_THRESHOLD,
            allowance: share.max(4 * (q.len() + 1) * (q.len() + 1)),
            spent: 0,
            cut: false,
        };
        self.rank_by_grams(&mut s);
        if !s.cut {
            self.sweep_by_length(&mut s);
        }
        self.work_left = self.work_left.saturating_sub(s.spent);
        if s.cut {
            self.stats.cut_short += 1;
        }
        self.found.insert(q, s.best);
        s.best
    }

    /// Looks first at the candidates sharing the most trigrams with the msgid.
    fn rank_by_grams(&mut self, s: &mut Search<'_>) {
        self.touched.clear();
        for i in 0..s.q.len().saturating_sub(2) {
            let Some(posting) = self.grams.get(&gram(s.q, i)) else { continue };
            if posting.len() > FUZZY_GRAM_LIMIT {
                continue;
            }
            for &c in posting {
                if self.shared[c.0 as usize] == 0 {
                    self.touched.push(c);
                }
                self.shared[c.0 as usize] += 1;
            }
        }
        let mut touched = std::mem::take(&mut self.touched);
        let shared = &mut self.shared;
        touched.sort_unstable_by_key(|&c| (std::cmp::Reverse(shared[c.0 as usize]), c));
        for &c in &touched {
            shared[c.0 as usize] = 0; // ready for the next search.
        }
        for &c in &touched {
            if s.cut {
                break;
            }
            self.consider(s, c.0 as usize);
        }
        self.touched = touched;
    }

    /// Looks at the rest from the msgid's length outward, while a candidate of that
    /// length could still win.
    fn sweep_by_length(&mut self, s: &mut Search<'_>) {
        let n = self.candidates.len();
        let mut right = self.candidates.partition_point(|c| c.id.len() < s.q.len());
        let mut left = right; // the next on the left is left - 1.
        while !s.cut && (left > 0 || right < n) {
            let bound = |at: usize| length_bound(s.q.len(), self.candidates[at].id.len());
            let bl = if left > 0 { bound(left - 1) } else { -1.0 };
            let br = if right < n { bound(right) } else { -1.0 };
            if bl.max(br) < s.best_score {
                return;
            }
            if br >= bl {
                self.consider(s, right);
                right += 1;
            } else {
                left -= 1;
                self.consider(s, left);
            }
        }
    }

    fn consider(&mut self, s: &mut Search<'_>, at: usize) {
        if self.seen[at] == self.stamp {
            return;
        }
        self.seen[at] = self.stamp;
        let c = self.candidates[at];
        if !could_win(s, length_bound(s.q.len(), c.id.len()), c.message) {
            return;
        }
        if !self.spend(s, s.q.len() + c.id.len()) {
            return;
        }
        self.stats.bounds += 1;
        if !could_win(s, similarity_bound(s.q, c.id), c.message) {
            return;
        }
        if !self.spend(s, s.q.len() * c.id.len()) {
            return;
        }
        self.stats.comparisons += 1;
        let score = lcs_ratio(s.q, c.id, &mut self.row);
        if could_win(s, score, c.message) {
            s.best = Some(c.message);
            s.best_score = score;
        }
    }

    fn spend(&mut self, s: &mut Search<'_>, bytes: usize) -> bool {
        if s.spent + bytes > s.allowance {
            s.cut = true;
            return false;
        }
        s.spent += bytes;
        self.stats.work += bytes;
        true
    }
}

/// The trigram postings of sorted candidates, each up to `FUZZY_GRAM_LIMIT + 1` long.
fn index_grams(candidates: &[Candidate<'_>]) -> HashMap<u32, Vec<CandidateId>> {
    let mut grams: HashMap<u32, Vec<CandidateId>> = HashMap::new();
    for (at, c) in candidates.iter().enumerate() {
        let at = CandidateId(at as u32);
        for i in 0..c.id.len().saturating_sub(2) {
            let posting = grams.entry(gram(c.id, i)).or_default();
            if posting.len() <= FUZZY_GRAM_LIMIT && posting.last() != Some(&at) {
                posting.push(at);
            }
        }
    }
    grams
}

/// Tells whether a score (or a bound of one) beats the best so far: higher, or as high
/// from an earlier message; the first to win must reach the threshold.
fn could_win(s: &Search<'_>, score: f64, message: usize) -> bool {
    score > s.best_score || (score == s.best_score && s.best.is_none_or(|best| message < best))
}

/// The most similarity can be for msgids of these lengths.
fn length_bound(a: usize, b: usize) -> f64 {
    if a + b == 0 {
        return 1.0;
    }
    2.0 * a.min(b) as f64 / (a + b) as f64
}

/// An upper bound of similarity from byte counts alone, so most pairs are rejected
/// without the quadratic comparison.
pub(crate) fn similarity_bound(a: &[u8], b: &[u8]) -> f64 {
    if a.len() + b.len() == 0 {
        return 1.0;
    }
    let mut counts = [0u32; 256];
    for &c in a {
        counts[usize::from(c)] += 1;
    }
    let mut common = 0;
    for &c in b {
        if counts[usize::from(c)] > 0 {
            counts[usize::from(c)] -= 1;
            common += 1;
        }
    }
    2.0 * f64::from(common) / (a.len() + b.len()) as f64
}

/// [`crate::similarity`] of two prefixes, with a reusable working row.
pub(crate) fn lcs_ratio(a: &[u8], b: &[u8], row: &mut Vec<u16>) -> f64 {
    if a.len() + b.len() == 0 {
        return 1.0;
    }
    let (x, y) = if a.len() < b.len() { (b, a) } else { (a, b) };
    row.clear();
    row.resize(y.len() + 1, 0);
    for &cx in x {
        let mut diag = 0;
        for (j, &cy) in y.iter().enumerate() {
            let up = row[j + 1];
            if cx == cy {
                row[j + 1] = diag + 1;
            } else if row[j] > up {
                row[j + 1] = row[j];
            }
            diag = up;
        }
    }
    2.0 * f64::from(row[y.len()]) / (x.len() + y.len()) as f64
}
