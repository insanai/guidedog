//! The zero-copy MO reader against GNU msgfmt's files, GNU msgunfmt against ours, and
//! every search path against each other and against hostile files.

use std::path::PathBuf;
use std::process::Command;

use super::Rng;
use crate::{
    Catalog, ErrorKind, LoadOptions, Lookup, Message, MoCatalog, MoSearch, PoReader, parse_mo,
    parse_po, write_mo,
};

const MSGFMT_PO: &str = include_str!("fixtures/msgfmt.po");
const MSGFMT_MO: &[u8] = include_bytes!("fixtures/msgfmt.mo");
const MSGFMT_BIG: &[u8] = include_bytes!("fixtures/msgfmt-big.mo");
const MSGFMT_NO_HASH: &[u8] = include_bytes!("fixtures/msgfmt-nohash.mo");

#[track_caller]
fn check_russian(mo: &MoCatalog) {
    assert_eq!(mo.text("Next"), "Далее");
    assert_eq!(mo.with_context("verb", "Open"), "Открыть");
    assert_eq!(mo.with_context("noun", "Open"), "Открытое");
    assert_eq!(mo.text("Open"), "Open");
    assert_eq!(mo.plural("%d file", "%d files", 21), "%d файл");
    assert_eq!(mo.plural("%d file", "%d files", 3), "%d файла");
    assert_eq!(mo.plural("%d file", "%d files", 11), "%d файлов");
    assert_eq!(mo.text("Line one\nline two"), "Строка один\nстрока два");
    assert_eq!(mo.text("Previous"), "Previous"); // msgfmt leaves fuzzy messages out.
    assert!(mo.header_field("Plural-Forms").is_some());
}

#[test]
fn msgfmt_files_are_read() {
    let little = MoCatalog::from_bytes(MSGFMT_MO.to_vec(), "msgfmt.mo").expect("reads");
    assert!(matches!(little.search(), MoSearch::Hash { .. }), "{:?}", little.search());
    check_russian(&little);
    let big = MoCatalog::from_bytes(MSGFMT_BIG.to_vec(), "msgfmt-big.mo").expect("reads");
    assert!(matches!(big.search(), MoSearch::Hash { .. }), "{:?}", big.search());
    check_russian(&big);
    let sorted = MoCatalog::from_bytes(MSGFMT_NO_HASH.to_vec(), "nohash.mo").expect("reads");
    assert_eq!(sorted.search(), &MoSearch::Sorted);
    check_russian(&sorted);
}

/// We compile the fixture's PO to the very bytes msgfmt wrote.
#[test]
fn we_write_what_msgfmt_writes() {
    let po = parse_po(MSGFMT_PO, "msgfmt.po").expect("the fixture parses");
    let ours = write_mo(&po, false).expect("compiles");
    assert_eq!(ours, MSGFMT_MO);
}

/// A scratch file under the system's temporary directory, removed when dropped.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temp_file(tag: &str) -> TempFile {
    TempFile(std::env::temp_dir().join(format!("gd-gettext-{tag}-{}", std::process::id())))
}

/// GNU msgunfmt reads what we write (skipped when it is not installed).
#[test]
fn msgunfmt_reads_our_files() {
    if Command::new("msgunfmt").arg("--version").output().is_err() {
        eprintln!("msgunfmt is not installed; skipping");
        return;
    }
    let po = parse_po(MSGFMT_PO, "msgfmt.po").expect("the fixture parses");
    let file = temp_file("unfmt.mo");
    std::fs::write(&file.0, write_mo(&po, false).expect("compiles")).expect("writes");
    let out = Command::new("msgunfmt").arg(&file.0).output().expect("msgunfmt runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).expect("msgunfmt writes UTF-8");
    let back = parse_po(&text, "unfmt.po").expect("msgunfmt's output parses");
    for m in po.messages.iter().filter(|m| !m.fuzzy) {
        let found = back.messages.iter().find(|b| b.same_key(m)).expect("each message");
        assert_eq!(found.strs, m.strs);
    }
}

/// A random catalog with contexts, plurals and a header, as a PO catalog.
fn random_catalog(rng: &mut Rng, n: usize) -> Catalog {
    let mut cat = Catalog::new("random.po");
    cat.set_header_field("Content-Type", "text/plain; charset=UTF-8");
    cat.set_header_field("Plural-Forms", "nplurals=3; plural=(n==1 ? 0 : n<5 ? 1 : 2);");
    for i in 0..n {
        let mut m = Message::new(&format!("msg {} {i}", rng.below(1000)));
        if rng.below(4) == 0 {
            m.ctx = Some(["menu", "", "verb"][rng.below(3)].to_string());
        }
        if rng.below(5) == 0 {
            m.id_plural = Some(format!("msgs {i}"));
            m.strs = vec![format!("a{i}"), format!("b{i}"), format!("c{i}")];
        } else if rng.below(6) != 0 {
            m.strs = vec![format!("t{i} ü")];
        }
        cat.add_message(m);
    }
    cat
}

/// Hash table, binary search, a permuted order, and the editable catalog's index
/// answer every lookup alike, for keys present and absent.
#[test]
fn every_search_agrees() {
    let mut rng = Rng::new(42);
    for n in [0, 1, 2, 3, 10, 300] {
        let cat = random_catalog(&mut rng, n);
        let bytes = write_mo(&cat, false).expect("compiles");
        let mo = MoCatalog::from_bytes(bytes.clone(), "r.mo").expect("reads");
        let MoSearch::Hash { probes, .. } = *mo.search() else {
            panic!("n={n}: {:?}", mo.search());
        };
        let unsorted = MoCatalog::from_bytes(swap_entries(&bytes, 1, 2), "u.mo");
        let mut queries: Vec<(Option<String>, String)> =
            cat.messages.iter().map(|m| (m.ctx.clone(), m.id.clone())).collect();
        queries.extend((0..50).map(|i| (None, format!("missing {i}"))));
        for (ctx, id) in &queries {
            let ctx = ctx.as_deref();
            let by_hash = mo.find_by_hash(probes, id, ctx);
            let by_sort = mo.find_sorted(id, ctx, |i| i);
            assert_eq!(by_hash, by_sort, "n={n} {ctx:?} {id:?}");
            let want: Vec<&str> =
                cat.find(id, ctx).map(|f| f.iter().collect()).unwrap_or_default();
            let got: Vec<&str> = mo.find(id, ctx).map(|f| f.iter().collect()).unwrap_or_default();
            assert_eq!(got, want, "n={n} {ctx:?} {id:?}");
            if let Ok(unsorted) = &unsorted {
                assert_eq!(unsorted.translation(id, ctx), mo.translation(id, ctx));
            }
        }
    }
}

/// Swaps entries `a` and `b` of both string tables, unsorting the file and breaking
/// its hash table.
fn swap_entries(bytes: &[u8], a: usize, b: usize) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let count = u32::from_le_bytes(bytes[8..12].try_into().expect("a word")) as usize;
    if a >= count || b >= count {
        return out;
    }
    for table_word in [12, 16] {
        let table =
            u32::from_le_bytes(bytes[table_word..table_word + 4].try_into().expect("a word"));
        let (x, y) = (table as usize + 8 * a, table as usize + 8 * b);
        out[x..x + 8].copy_from_slice(&bytes[y..y + 8]);
        out[y..y + 8].copy_from_slice(&bytes[x..x + 8]);
    }
    out
}

#[test]
fn unsorted_files_are_searched_through_an_order() {
    let cat = random_catalog(&mut Rng::new(5), 50);
    let bytes = swap_entries(&write_mo(&cat, false).expect("compiles"), 3, 30);
    let mo = MoCatalog::from_bytes(bytes, "unsorted.mo").expect("an unsorted file reads");
    assert!(matches!(mo.search(), MoSearch::Permuted(_)), "{:?}", mo.search());
    for m in cat.messages.iter().filter(|m| m.is_translated()) {
        assert_eq!(mo.translation(&m.id, m.ctx.as_deref()), Some(m.strs[0].as_str()));
    }
}

#[test]
fn a_broken_hash_table_is_not_trusted() {
    let mut bytes = MSGFMT_MO.to_vec();
    let size = u32::from_le_bytes(bytes[20..24].try_into().expect("a word")) as usize;
    let at = u32::from_le_bytes(bytes[24..28].try_into().expect("a word")) as usize;
    bytes[at..at + 4 * size].fill(0); // every slot empty: no key would be found.
    let mo = MoCatalog::from_bytes(bytes, "nohash.mo").expect("still reads");
    assert_eq!(mo.search(), &MoSearch::Sorted);
    check_russian(&mo);
}

#[track_caller]
fn expect_mo_error(bytes: Vec<u8>, fragment: &str) {
    let err = MoCatalog::from_bytes(bytes, "bad.mo").expect_err("a damaged file is refused");
    assert_eq!(err.kind, ErrorKind::MoFormat);
    assert!(err.message.contains(fragment), "{err}");
    assert!(err.to_string().contains("Hint: "));
}

#[test]
fn damaged_files_are_refused() {
    let words = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().expect("w"));
    // A string whose NUL is overwritten.
    let mut no_nul = MSGFMT_MO.to_vec();
    let (len, off) = (words(&no_nul, 28 + 8) as usize, words(&no_nul, 28 + 12) as usize);
    no_nul[off + len] = b'x';
    expect_mo_error(no_nul, "NUL");
    // Two entries with the same key.
    let mut twice = MSGFMT_MO.to_vec();
    twice.copy_within(28 + 8..28 + 16, 28 + 16);
    expect_mo_error(twice, "share bytes");
    // A count larger than the file.
    let mut count = MSGFMT_MO.to_vec();
    count[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    expect_mo_error(count, "more messages");
    // A table offset past the end.
    let mut table = MSGFMT_MO.to_vec();
    table[12..16].copy_from_slice(&(MSGFMT_MO.len() as u32).to_le_bytes());
    expect_mo_error(table, "outside the file");
    // An unknown major revision.
    let mut revision = MSGFMT_MO.to_vec();
    revision[4..8].copy_from_slice(&(2u32 << 16).to_le_bytes());
    expect_mo_error(revision, "revision 2");
    expect_mo_error(MSGFMT_MO[..20].to_vec(), "truncated");
}

#[test]
fn equal_keys_are_refused() {
    // Two strings apart from each other but equal: make entry 2's key a copy of entry 1's.
    let cat = parse_po("msgid \"aa\"\nmsgstr \"x\"\n\nmsgid \"ab\"\nmsgstr \"y\"\n", "k.po")
        .expect("parses");
    let mut bytes = write_mo(&cat, false).expect("compiles");
    let word = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().expect("w"));
    let second_key = word(&bytes, 28 + 8 + 4) as usize;
    bytes[second_key + 1] = b'a';
    let err = MoCatalog::from_bytes(bytes, "k.mo").expect_err("equal keys are refused");
    assert!(err.message.contains("same key"), "{err}");
}

#[test]
fn files_past_the_limit_are_refused() {
    let file = temp_file("limit.mo");
    std::fs::write(&file.0, MSGFMT_MO).expect("writes");
    let options = LoadOptions { max_bytes: 100, ..LoadOptions::default() };
    let err = MoCatalog::load(&file.0, &options).expect_err("too large");
    assert_eq!(err.kind, ErrorKind::TooLarge);
    assert!(err.to_string().contains("Hint: Raise LoadOptions::max_bytes"));
    let po = temp_file("limit.po");
    std::fs::write(&po.0, MSGFMT_PO).expect("writes");
    let err = PoReader::open(&po.0, &options).expect_err("too large");
    assert_eq!(err.kind, ErrorKind::TooLarge);
    assert!(MoCatalog::load(&file.0, &LoadOptions::default()).is_ok());
}

/// The editable copy of an MO file holds what the zero-copy one serves.
#[test]
fn parse_mo_copies_every_entry() {
    let cat = parse_mo(MSGFMT_MO, "msgfmt.mo").expect("reads");
    assert!(cat.header.is_some());
    assert_eq!(cat.messages.len(), 5);
    assert_eq!(cat.with_context("noun", "Open"), "Открытое");
    assert_eq!(cat.plural("%d file", "%d files", 5), "%d файлов");
}

/// A file whose string region holds bytes that are not UTF-8 between strings still
/// reads: the strings are then copied into a new pool instead of reusing the buffer.
#[test]
fn a_gap_that_is_not_utf8_is_copied_around() {
    let mut bytes = MSGFMT_MO.to_vec();
    let word = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().expect("w"));
    let translations = word(&bytes, 16) as usize;
    let last = translations + 8 * (word(&bytes, 8) as usize - 1);
    let (len, off) = (word(&bytes, last) as usize, word(&bytes, last + 4) as usize);
    let moved = bytes[off..off + len].to_vec();
    bytes.extend_from_slice(b"\xff\xfe");
    let new_off = bytes.len() as u32;
    bytes.extend_from_slice(&moved);
    bytes.push(0);
    bytes[last + 4..last + 8].copy_from_slice(&new_off.to_le_bytes());
    let mo = MoCatalog::from_bytes(bytes, "gap.mo").expect("a file with a gap reads");
    check_russian(&mo);
}
