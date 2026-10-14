//! A benchmark of large catalogs, ignored by default. Run it with
//! `cargo test --release -p gd-gettext --test large -- --ignored --nocapture`.
//!
//! It counts the bytes allocated (total and peak live) with a counting global
//! allocator, which is why it lives in its own test binary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use gd_gettext::{
    Catalog, LoadOptions, Lookup, Message, MoCatalog, PoReader, Stats, WriteOptions, parse_mo,
    parse_po, write_mo, write_po,
};

/// Counts what the program allocates: the bytes live now, the most live at once since
/// the last reset, and the total allocated.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is forwarded unchanged to the system allocator; the counters only
// observe sizes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        TOTAL.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's layout is passed through as `GlobalAlloc::alloc` requires.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: `ptr` was allocated by `System` with this layout, in `alloc` above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// What one step cost: time, bytes allocated in all, and the most held at once above
/// what was live before it.
struct Cost {
    time: Duration,
    total: usize,
    peak: usize,
}

fn measure<T>(step: impl FnOnce() -> T) -> (T, Cost) {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let total = TOTAL.load(Ordering::Relaxed);
    let start = Instant::now();
    let value = step();
    let time = start.elapsed();
    let cost = Cost {
        time,
        total: TOTAL.load(Ordering::Relaxed) - total,
        peak: PEAK.load(Ordering::Relaxed) - base,
    };
    (value, cost)
}

fn report(what: &str, cost: &Cost) {
    let mb = |b: usize| b as f64 / 1e6;
    println!(
        "{what:<44} {:>9.1} ms  {:>8.1} MB allocated  {:>8.1} MB peak",
        cost.time.as_secs_f64() * 1e3,
        mb(cost.total),
        mb(cost.peak)
    );
}

const MESSAGES: usize = 200_000;

fn large_catalog() -> Catalog {
    let mut cat = Catalog::new("large.po");
    cat.set_header_field("Content-Type", "text/plain; charset=UTF-8");
    cat.set_header_field("Plural-Forms", "nplurals=2; plural=(n != 1);");
    let filler = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod. ";
    for i in 0..MESSAGES {
        let mut m = Message::new(&format!("Message number {i:07} of the large catalog"));
        if i % 7 == 0 {
            m.ctx = Some("menu".into());
        }
        m.strs[0] = format!("Übersetzung {i:07}: {}", filler.repeat(3));
        cat.add_message(m);
    }
    cat
}

/// Times `rounds` lookups of present and missing keys; returns lookups per second.
fn lookups_per_second(cat: &dyn Lookup, rounds: usize) -> f64 {
    let keys: Vec<String> = (0..1000)
        .map(|k| format!("Message number {:07} of the large catalog", (k * 7919 + 1) % MESSAGES))
        .collect();
    let start = Instant::now();
    let mut found = 0;
    for r in 0..rounds {
        let key = &keys[r % keys.len()];
        if cat.translation(key, None).is_some() {
            found += 1;
        }
        found += usize::from(cat.translation("Missing key", None).is_some());
    }
    assert!(found > 0);
    (2 * rounds) as f64 / start.elapsed().as_secs_f64()
}

#[test]
#[ignore = "a benchmark: run with --release -- --ignored --nocapture"]
fn large_catalogs() {
    let (cat, cost) = measure(large_catalog);
    report("build a 200k-message Catalog", &cost);
    let (bytes, cost) = measure(|| write_mo(&cat, false).expect("compiles"));
    report(&format!("write_mo ({:.1} MB)", bytes.len() as f64 / 1e6), &cost);
    let (text, cost) = measure(|| write_po(&cat, &WriteOptions::default()));
    report(&format!("write_po ({:.1} MB)", text.len() as f64 / 1e6), &cost);
    drop(cat);

    let (copied, cost) = measure(|| parse_mo(&bytes, "large.mo").expect("reads"));
    report("before: parse_mo into a Catalog (copies)", &cost);
    let (_, cost) = measure(|| copied.translation("x", None));
    report("before: first lookup builds the index", &cost);
    let (mo, cost) = measure(|| MoCatalog::from_bytes(bytes.clone(), "large.mo").expect("reads"));
    report("after: MoCatalog::from_bytes (clone counted)", &cost);
    println!("  search: {}", search_name(&mo));
    let file = std::env::temp_dir().join(format!("gd-gettext-large-{}.mo", std::process::id()));
    std::fs::write(&file, &bytes).expect("writes");
    let (loaded, cost) = measure(|| MoCatalog::load(&file, &LoadOptions::default()));
    report("after: MoCatalog::load from a file", &cost);
    drop(loaded);
    let _ = std::fs::remove_file(&file);

    let rounds = 1_000_000;
    println!("lookups/s, Catalog index:   {:>12.0}", lookups_per_second(&copied, rounds));
    println!("lookups/s, MoCatalog:       {:>12.0}", lookups_per_second(&mo, rounds));
    drop(copied);

    let (parsed, cost) = measure(|| parse_po(&text, "large.po").expect("parses"));
    report("parse_po into a Catalog", &cost);
    drop(parsed);
    let (stats, cost) = measure(|| {
        let mut stats = Stats::default();
        for m in PoReader::new(text.as_bytes(), "large.po") {
            stats.add(&m.expect("parses"));
        }
        stats
    });
    report("PoReader: stats without a Catalog", &cost);
    assert_eq!(stats.translated, MESSAGES);
}

fn search_name(mo: &MoCatalog) -> String {
    match mo.search() {
        gd_gettext::MoSearch::Hash { probes, .. } => {
            format!("hash table, at most {probes} probes")
        }
        other => format!("{other:?}"),
    }
}
