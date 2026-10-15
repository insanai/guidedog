//! Unit tests of the internals: budget waiting, sizes as text, crash report parts,
//! lexical path cleaning, stage names, and stack budgets.

use super::*;
use crate::confine::clean;
use crate::crash::crash_parts;
use crate::problem::human_bytes;
use std::path::{Path, PathBuf};

const MIB: usize = 1 << 20;

#[test]
fn sizes_read_as_text_never_round_down_to_the_budget() {
    assert_eq!(human_bytes(512), "512 bytes");
    assert_eq!(human_bytes(4096), "4 KiB");
    assert_eq!(human_bytes(MIB), "1 MiB");
    assert_eq!(human_bytes(MIB + 1), "1.1 MiB");
    assert_eq!(human_bytes(2 * MIB - 1), "2 MiB");
}

// Units that run at once wait for each other's memory, but never all at once: when every
// other unit waits too, a reservation is refused instead of waiting forever.
#[test]
fn waiting_units_never_deadlock() {
    let budget = Budget::new(100);
    let mine = budget.join();
    let theirs = budget.join();
    assert!(budget.reserve(80));
    std::thread::scope(|s| {
        let waiter = s.spawn(|| budget.reserve(50));
        while budget.waiting_count() != 1 {
            std::thread::yield_now();
        }
        // Both units would wait now: this one is refused, the waiter goes on waiting.
        assert!(!budget.reserve(50));
        budget.release(80);
        assert!(waiter.join().unwrap_or(false));
    });
    assert_eq!(budget.used(), 50);
    drop(theirs);
    // A unit alone never waits for itself.
    assert!(!budget.reserve(60));
    drop(mine);
    assert_eq!(budget.peak(), 80);
}

#[test]
fn ledgers_release_what_they_hold_when_dropped() {
    let budget = Budget::new(8 * MIB);
    {
        let ledger = Ledger::new(&budget);
        assert!(ledger.charge(4096));
        assert!(!ledger.charge(8 * MIB));
        assert_eq!(ledger.refused(), 8 * MIB);
        assert_eq!(budget.used(), 4096);
        ledger.discharge(96);
        assert_eq!(ledger.held(), 4000);
    }
    assert_eq!(budget.used(), 0);
    let free = Ledger::unbounded();
    assert!(free.charge(usize::MAX));
}

// A reservation the budget cannot hold asks its policy: raising the limit lets it
// through; stopping refuses it and every later one without asking again.
#[test]
fn policies_raise_or_stop() {
    use std::sync::{Arc, Mutex};
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&asked);
    let raise = Budget::with_policy(4096, move |r| {
        seen.lock().map(|mut v| v.push(*r)).ok();
        RefusalChoice::Raise(8192)
    });
    let l = Ledger::new(&raise);
    assert!(l.charge(3000));
    assert!(l.charge(3000));
    assert_eq!(raise.limit(), 8192);
    let asked = asked.lock().map(|v| v.clone()).unwrap_or_default();
    assert_eq!(asked, vec![Refusal { need: 3000, used: 3000, limit: 4096 }]);

    let count = Arc::new(Mutex::new(0));
    let counted = Arc::clone(&count);
    let stop = Budget::with_policy(4096, move |_| {
        counted.lock().map(|mut n| *n += 1).ok();
        RefusalChoice::Stop
    });
    let m = Ledger::new(&stop);
    assert!(!m.charge(5000));
    assert!(!m.charge(6000));
    assert_eq!(count.lock().map(|n| *n).unwrap_or(0), 1);
    assert_eq!(stop.last_refusal(), Some(Refusal { need: 6000, used: 0, limit: 4096 }));
    let p = budget_problem("Testing", "x", 5000, Some(&stop));
    assert_eq!(p.code, "host.budget");
    assert!(p.hint.contains("--budget=") && !p.hint.contains("--budget=MIB"), "{}", p.hint);
}

// The hint names the budget the step fits in: what is held and the request four times
// over, or twice the old budget, in steps of 64 MiB, within the free memory when what is
// needed fits there. When it does not, the hint says so.
#[test]
fn refusal_hints_name_a_budget_that_fits() {
    let cases = [
        (1000 * MIB, 100 * MIB, 1024 * MIB, None, 2048 * MIB, true),
        (1000 * MIB, 100 * MIB, 1024 * MIB, Some(1500 * MIB), 1408 * MIB, true),
        (1000 * MIB, 100 * MIB, 1024 * MIB, Some(900 * MIB), 1408 * MIB, false),
        (3 * MIB, 60 * MIB, 4 * MIB, None, 256 * MIB, true), // the review's template.
        (3 * MIB, 60 * MIB, 4 * MIB, Some(4096 * MIB), 256 * MIB, true),
    ];
    for (used, need, limit, available, budget, fits) in cases {
        assert_eq!(suggest_budget(used, need, limit, available), (budget, fits));
    }
    let (huge, fits) = suggest_budget(usize::MAX, usize::MAX, usize::MAX, None);
    assert!(fits && huge >= usize::MAX / 8);
    assert_eq!(
        budget_advice(3 * MIB, 60 * MIB, 4 * MIB, None, "split the document"),
        "Run again with --budget=256, or split the document."
    );
    assert_eq!(
        budget_advice(3 * MIB, 60 * MIB, 4 * MIB, Some(32 * MIB), "split the document"),
        "Free memory and run again with --budget=256: the step needs about 256 MiB, more \
         than the 32 MiB this machine has free. Or split the document."
    );
}

#[test]
fn no_budget_is_claimed_to_fit_zero_free_memory() {
    let (budget, fits) = suggest_budget(0, 60 * MIB, 4 * MIB, Some(0));
    assert!(budget > 0 && !fits);
    assert_eq!(fitting_budget(0), 0);
    assert_eq!(fitting_budget(MIB), 0);
    let hint = budget_advice(0, 60 * MIB, 4 * MIB, Some(0), "split the document");
    assert!(hint.contains("0 MiB"), "{hint}");
}

#[test]
fn limits_are_lowered_to_the_budget() {
    let limits = limits_within(guidedoc::default_limits(), 4 * MIB);
    assert_eq!(limits.max_output_bytes, 4 * MIB);
    assert_eq!(limits.max_text_bytes, 4 * MIB);
    assert_eq!(limits.max_nodes, guidedoc::default_limits().max_nodes);
    assert_eq!(limits_within(limits, 0), limits);
}

// A crash report names an issue tracker only when the build was given one; without,
// it sends the user to whoever provided the build, and either way it carries what
// `guidedog --version` prints.
#[test]
fn a_crash_report_names_no_tracker_it_was_not_given() {
    let version = "guidedog 1.2.3\nguidedoc 4.5.6 (document schema 7)\n";
    let args: Vec<String> = ["guidedog", "build", "my docs"].map(String::from).to_vec();
    let (head, tail) = crash_parts(&args, version, "");
    assert!(head.contains("guidedog build 'my docs'"));
    assert!(head.contains("GUIDEDOG CRASHED") && head.contains("host.crash"));
    assert!(tail.contains("report it to whoever provided\nyour Guidedog build"));
    assert!(tail.contains("guidedog 1.2.3\nguidedoc 4.5.6 (document schema 7)\nplatform "));
    assert!(!tail.contains("http"));
    let (_, tail) = crash_parts(&args[..1], version, "https://example.org/issues");
    assert!(tail.contains("report it at\nhttps://example.org/issues\nwith this whole message"));
    assert_eq!(ISSUE_TRACKER, "");
}

#[test]
fn a_panic_is_reported_as_a_crash() {
    let status = catch_crash(|| panic!("broken invariant"));
    assert_eq!(status, CRASH_EXIT);
    assert_eq!(catch_crash(|| 3), 3);
}

#[test]
fn paths_are_cleaned_lexically() {
    assert_eq!(clean(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
    assert_eq!(clean(Path::new("/../x")), PathBuf::from("/x"));
    assert_eq!(clean(Path::new("../a/../../b")), PathBuf::from("../../b"));
}

#[test]
fn stage_names_are_recognized_exactly() {
    assert!(is_stage_name(".guidedog-stage-0123456789"));
    assert!(is_stage_name(".guidedog-stage-0123456789.html"));
    for other in [
        ".guidedog-stage-012345678.next",
        ".guidedog-stage-0123456789x",
        ".guidedog-stage-0123456789.tar.gz",
        ".guidedog-stage-notes.txt",
        ".guidedog-stage-",
        "notes.next",
    ] {
        assert!(!is_stage_name(other), "{other}");
    }
}

// A stack budget leaves the thread room for the frames around the guarded recursion:
// 6 MiB of an 8 MiB thread; a smaller thread gets less than the default.
#[test]
fn stack_budgets_fit_their_threads() {
    assert_eq!(stack_budget_limit(8 * MIB), 6 * MIB);
    assert_eq!(stack_budget_limit(MIB), MIB / 2);
    assert_eq!(default_stack_budget(WORKER_STACK_BYTES), guidedoc::DEFAULT_STACK_BYTES);
    assert_eq!(default_stack_budget(512 << 10), 256 << 10);
    const { assert!(guidedoc::DEFAULT_STACK_BYTES + guidedoc::STACK_RESERVE < MIB) };
}

#[test]
fn workers_share_the_work_and_raise_panics_on_the_caller() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    run_workers(4, |_| {
        while next.fetch_add(1, Ordering::Relaxed) < 100 {
            done.fetch_add(1, Ordering::Relaxed);
        }
    });
    assert_eq!(done.load(Ordering::Relaxed), 100);
    let caught = std::panic::catch_unwind(|| run_workers(2, |i| assert!(i > 5, "worker {i}")));
    assert!(caught.is_err());
    assert_eq!(run_with_stack(WORKER_STACK_BYTES, || 6 * 7), 42);
}
