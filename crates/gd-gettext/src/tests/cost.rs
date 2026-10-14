//! What a hostile catalog can cost: reading, merging, and writing one grow with its
//! size, never with its square. Each check counts work where it can, and times it only
//! where counting would need instrumenting the code under test.

use std::time::{Duration, Instant};

use super::Rng;
use crate::PoReader;
use crate::po_read::collect_catalog;
use crate::{
    Catalog, ErrorKind, FUZZY_MIN_WORK, FUZZY_THRESHOLD, FUZZY_WORK_PER_BYTE, MergeOptions,
    Message, WriteOptions, merge_counted, parse_mo, parse_po, similarity, write_mo, write_po,
};

/// A field continued over many lines is gathered once: the parser copies about the
/// text into the catalog, not every longer copy of the field as each line was added.
#[test]
fn po_continuation_lines_cost_their_length() {
    let line = "\"0123456789012345678901234567890123456789012345678901234567890\"\n";
    let mut text = String::from("msgid \"a\"\nmsgid_plural \"b\"\nmsgstr[0] \"\"\n");
    text.push_str(&line.repeat(1000));
    text.push_str("msgstr[1] \"\"\n");
    text.push_str(&line.repeat(1000));
    let reader = PoReader::new(text.as_bytes(), "long.po");
    let (cat, copied) = collect_catalog(reader, "long.po").expect("the text parses");
    let strs = &cat.messages[0].strs;
    assert_eq!(strs[0].len(), 1000 * (line.len() - 3));
    assert_eq!(strs[0], strs[1]);
    assert!(copied < 2 * text.len(), "copied {copied} bytes for {} of text", text.len());
}

fn translation_offset(translations: usize, i: usize) -> usize {
    translations + 8 * i + 4
}

/// A compiled catalog whose table points many entries at the same bytes is refused:
/// each entry would check and copy them again. Strings apart from each other still read.
#[test]
fn mo_strings_that_share_bytes_are_refused() {
    let mut po = Catalog::new("many.po");
    for i in 0..200 {
        let mut m = Message::new(&format!("k{i:03}"));
        m.strs[0] = "v".repeat(4096);
        po.add_message(m);
    }
    let mut bytes = write_mo(&po, false).expect("the catalog compiles");
    let whole = parse_mo(&bytes, "many.mo").expect("a sound file reads");
    assert_eq!(whole.messages.len(), 200);
    // Point every translation at the first one's bytes.
    let translations = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    let first_at = translation_offset(translations, 0);
    let first: [u8; 4] =
        [bytes[first_at], bytes[first_at + 1], bytes[first_at + 2], bytes[first_at + 3]];
    for i in 1..200 {
        let at = translation_offset(translations, i);
        bytes[at..at + 4].copy_from_slice(&first);
    }
    let shared = parse_mo(&bytes, "shared.mo").expect_err("shared bytes are refused");
    assert_eq!(shared.kind, ErrorKind::MoFormat);
    assert!(shared.message.contains("share bytes"));
    // A string starting inside another is refused the same way.
    let offset = u32::from_le_bytes(first);
    for i in 1..200u32 {
        let at = translation_offset(translations, i as usize);
        bytes[at - 4..at].copy_from_slice(&1u32.to_le_bytes());
        bytes[at..at + 4].copy_from_slice(&(offset + i).to_le_bytes());
    }
    let inside = parse_mo(&bytes, "inside.mo").expect_err("overlapping strings are refused");
    assert_eq!(inside.kind, ErrorKind::MoFormat);
}

/// The exhaustive search the fuzzy index replaces: every pair compared.
fn most_similar_by_every_pair(def: &Catalog, id: &str) -> Option<usize> {
    let mut best = None;
    let mut best_score = FUZZY_THRESHOLD;
    for (i, d) in def.messages.iter().enumerate() {
        if d.obsolete || !d.is_translated() || d.id.is_empty() {
            continue;
        }
        let score = similarity(id, &d.id);
        if score >= best_score && (best.is_none() || score > best_score) {
            best = Some(i);
            best_score = score;
        }
    }
    best
}

fn random_msgid(rng: &mut Rng, words: &[&str]) -> String {
    let count = 2 + rng.below(8);
    let picked: Vec<&str> = (0..count).map(|_| *rng.choice(words)).collect();
    picked.join(" ")
}

/// Builds an old translation and a new template of random msgids from a few words.
fn random_catalogs(rng: &mut Rng) -> (Catalog, Catalog) {
    let words = [
        "install",
        "the",
        "package",
        "with",
        "a",
        "new",
        "installer",
        "build",
        "docs",
        "run",
        "page",
        "index",
        "is",
        "not",
        "found",
        "x",
        "y",
    ];
    let (mut def, mut reference) = (Catalog::new("de.po"), Catalog::new("x.pot"));
    for _ in 0..400 {
        let mut m = Message::new(&random_msgid(rng, &words));
        if rng.below(5) != 0 {
            m.strs[0] = "t".into();
        }
        m.obsolete = rng.below(10) == 0;
        def.add_message(m);
        reference.add_message(Message::new(&random_msgid(rng, &words)));
    }
    (def, reference)
}

/// The fuzzy index finds what comparing every pair finds, ties included. (The seed is
/// one whose searches all stay within their share of the work bound: with these
/// catalogs, where every msgid is made of the same 17 words, most seeds have a search
/// or two that the bound cuts short, which then keeps the best it had found.)
#[test]
fn fuzzy_search_finds_what_every_pair_finds() {
    let (def, reference) = random_catalogs(&mut Rng::new(11));
    let (out, stats) = merge_counted(&def, &reference, &MergeOptions::default());
    assert_eq!(stats.cut_short, 0);
    let mut exact = 0;
    for (q, m) in reference.messages.iter().zip(&out.messages) {
        if def.messages.iter().any(|d| d.same_key(q)) {
            exact += 1;
            continue;
        }
        let found = m.previous.as_ref().map(|p| p.id.as_str());
        match most_similar_by_every_pair(&def, &q.id) {
            None => assert!(!m.fuzzy, "{:?} matched nothing", q.id),
            Some(want) => assert_eq!(found, Some(def.messages[want].id.as_str()), "{:?}", q.id),
        }
    }
    assert!(exact < reference.messages.len());
    assert!(stats.comparisons < reference.messages.len() * def.messages.len() / 4);
}

/// Messages that all score alike make every pair worth comparing. The search stays
/// within its work bound, which grows with the catalogs, and still finds each message's
/// twin first.
#[test]
fn fuzzy_search_work_grows_with_the_catalogs() {
    let n = 2000;
    let (mut def, mut reference) = (Catalog::new("de.po"), Catalog::new("x.pot"));
    let mut bytes = 0;
    for i in 0..n {
        let mut m =
            Message::new(&format!("The quick brown fox jumps over the lazy dog, number {i:06}."));
        m.strs[0] = "t".into();
        let q =
            Message::new(&format!("The quick brown fox leaps over the lazy cat, number {i:06}!"));
        bytes += m.id.len() + q.id.len();
        def.add_message(m);
        reference.add_message(q);
    }
    let (out, stats) = merge_counted(&def, &reference, &MergeOptions::default());
    assert_eq!(stats.searches, n);
    let bound = (FUZZY_WORK_PER_BYTE * bytes).max(FUZZY_MIN_WORK);
    assert!(stats.work <= bound, "work {} for {bytes} bytes", stats.work);
    assert!(stats.comparisons < 64 * n, "{} comparisons", stats.comparisons);
    let twins = out
        .messages
        .iter()
        .zip(&def.messages)
        .filter(|(m, d)| m.previous.as_ref().is_some_and(|p| p.id == d.id))
        .count();
    assert_eq!(twins, n, "{twins} of {n} messages found their twin");
}

/// Wrapping a long line looks only at the next piece, so its cost is its length. The
/// bound is generous: the quadratic rescan took seconds here, the linear one
/// milliseconds (in a debug build, tens of them).
#[test]
fn wrapping_a_long_line_costs_its_length() {
    let mut cat = Catalog::new("w.po");
    let id = "ab ".repeat(400_000);
    cat.add_message(Message::new(&id));
    let start = Instant::now();
    let text = write_po(&cat, &WriteOptions::default());
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(2), "wrapping took {elapsed:?}");
    let lines = text.matches('\n').count();
    assert!(lines > 400_000 * 3 / 76 && lines < 400_000 * 3 / 60, "{lines} lines");
    let back = parse_po(&text, "w.po").expect("the wrapped text parses");
    assert_eq!(back.messages[0].id, id);
}
