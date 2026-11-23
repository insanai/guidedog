use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use super::model::{Bounds, reachable};
use super::*;
use crate::testutil::TempDir;

/// Names the lock file the helper process takes; set only for that process.
const HELPER_VARIABLE: &str = "GUIDEDOG_TEST_LOCK_HELPER";
/// The exit status of the helper process once it holds the lock.
const HELPER_HELD: i32 = 17;

fn is_held(path: &Path) -> bool {
    matches!(lock_exclusive(path), Err(LockError::Held))
}

// The defect, without timing. A duplicate of the descriptor shares the open file, as
// the copy in a child process does. While it lives, a release must still release.
#[test]
fn a_copy_of_the_descriptor_does_not_keep_a_released_lock() {
    let root = TempDir::new("lock-copy");
    let path = root.path().join("state.lock");
    let held = lock_exclusive(&path).unwrap();
    let copy = held.file.try_clone().unwrap();
    assert!(is_held(&path), "a lock that is held is refused");
    drop(held);
    assert!(!is_held(&path), "a released lock is free, whoever still copies its file");
    let again = lock_exclusive(&path).unwrap();
    assert!(is_held(&path));
    drop(copy);
    assert!(is_held(&path), "the old copy's end does not release the new lock");
    drop(again);
    assert!(!is_held(&path));
}

// What we replaced, and the system rule the model assumes: closing a descriptor releases
// the lock only when no copy is left.
#[cfg(unix)]
#[test]
fn closing_alone_leaves_the_lock_with_a_copy() {
    let root = TempDir::new("lock-close");
    let path = root.path().join("state.lock");
    let mut options = std::fs::OpenOptions::new();
    let file = options.read(true).write(true).create(true).truncate(false).open(&path).unwrap();
    file.try_lock().unwrap();
    let copy = file.try_clone().unwrap();
    drop(file);
    assert!(is_held(&path), "nobody holds the lock, and it is refused");
    drop(copy);
    // This test closes without unlocking, so it is open to the very defect it shows:
    // a child that another test starts at this instant holds one more copy until it
    // execs. We therefore wait for the release instead of expecting it at once.
    let started = std::time::Instant::now();
    while is_held(&path) {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the last close releases it"
        );
        std::thread::yield_now();
    }
}

#[test]
fn a_held_lock_is_refused_in_another_thread_and_a_private_file_is_private() {
    let root = TempDir::new("lock-thread");
    let path = root.path().join("state.lock");
    let held = lock_exclusive_private(&path).unwrap();
    let refused = std::thread::scope(|scope| scope.spawn(|| is_held(&path)).join().unwrap());
    assert!(refused);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    drop(held);
    let missing = root.path().join("no-such-folder/state.lock");
    assert!(matches!(lock_exclusive(&missing), Err(LockError::Io(_))));
}

// Run by `a_holder_that_exits_without_releasing_leaves_no_lock` in a process of its own:
// it takes the lock and ends the process there, so nothing unlocks.
#[test]
fn helper_takes_the_lock_and_exits() {
    let Some(path) = std::env::var_os(HELPER_VARIABLE) else { return };
    let lock = lock_exclusive(Path::new(&path));
    std::process::exit(if lock.is_ok() { HELPER_HELD } else { 1 });
}

// Crash recovery (Theorem 5 of GDS 0019): the system closes a dead holder's descriptor.
#[test]
fn a_holder_that_exits_without_releasing_leaves_no_lock() {
    let root = TempDir::new("lock-crash");
    let path = root.path().join("state.lock");
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "files::lock::tests::helper_takes_the_lock_and_exits"])
        .env(HELPER_VARIABLE, &path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(HELPER_HELD), "the helper held the lock when it ended");
    assert!(!is_held(&path));
}

// The race itself: other threads start programs while a thread takes and releases a
// lock nobody else wants. Any refusal is a false one.
#[test]
fn starting_programs_on_other_threads_never_makes_a_free_lock_look_held() {
    let root = TempDir::new("lock-stress");
    let program = std::env::current_exe().unwrap();
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        for _ in 0..3 {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let mut child = Command::new(&program);
                    child.args(["--exact", "no-such-test"]).stdout(Stdio::null());
                    child.status().unwrap();
                }
            });
        }
        let lockers: Vec<_> = (0..2)
            .map(|n| {
                let path = root.path().join(format!("state-{n}.lock"));
                scope.spawn(move || (0..400).filter(|_| is_held(&path)).count())
            })
            .collect();
        let refused: usize = lockers.into_iter().map(|l| l.join().unwrap()).sum();
        stop.store(true, Ordering::Relaxed);
        assert_eq!(refused, 0, "a lock nobody held was refused");
    });
}

/// Checks every reachable state of the algorithm within `bounds` against the invariant
/// and the theorems of GDS 0019, and returns how many states there are.
fn check_every_state(bounds: &Bounds) -> usize {
    let states = reachable(bounds);
    for s in &states {
        // Lemma 1: the invariant, clause by clause.
        assert_eq!(s.broken_clause(), None, "{s:?}");
        // Corollary 2a: with no dead taker, a refusal means a holder.
        assert!(!s.is_busy() || s.has_holder() || s.has_dead(), "{s:?}");
        // Corollary 2b: a held lock is refused.
        assert!(!s.has_holder() || s.is_busy(), "{s:?}");
        // Theorem 4: no update is lost.
        assert!(s.counts_every_update(), "{s:?}");
        // Theorems 5 and 8: a dead taker's lock ends with its copies.
        let mut drained = *s;
        for b in (0..bounds.takers).filter(|b| s.is_dead(*b)) {
            let (after, locked) = drained.drained(b);
            assert!(!locked, "{s:?}");
            drained = after;
        }
        // Theorem 6: then, without a holder, the lock is free.
        assert!(drained.has_holder() || !drained.is_busy(), "{s:?}");
    }
    states.len()
}

#[test]
fn the_algorithm_keeps_the_invariant_in_every_interleaving() {
    let two = Bounds { takers: 2, copies: 2, updates: 2, explicit: true };
    let three = Bounds { takers: 3, copies: 1, updates: 1, explicit: true };
    assert!(check_every_state(&two) > 1000);
    assert!(check_every_state(&three) > 1000);
}

// Theorem 9: the algorithm we replaced reaches a refusal with no holder and no dead
// taker, and the clause it breaks is the one that forbids a false refusal.
#[test]
fn closing_alone_reaches_a_false_refusal() {
    let bounds = Bounds { takers: 2, copies: 1, updates: 0, explicit: false };
    let states = reachable(&bounds);
    let falsely = |s: &&model::State| s.is_busy() && !s.has_holder() && !s.has_dead();
    let found = states.iter().find(falsely).expect("a false refusal is reachable");
    assert_eq!(found.broken_clause(), Some("lockedOwner"));
    let broken: HashSet<_> = states.iter().filter_map(|s| s.broken_clause()).collect();
    assert_eq!(broken, HashSet::from(["lockedOwner"]), "it breaks nothing else");
}
