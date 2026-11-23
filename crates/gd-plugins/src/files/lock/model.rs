//! The model of GDS 0019, enumerated: the same states, steps and invariant as
//! `docs/gds/proofs/Gds/R0019/FileLock.lean`, with a few takers and a bound on the
//! copies, so every reachable state can be visited.
//!
//! Lean proves the invariant for any number of takers and copies. This test checks that
//! the model written there is the one we mean: it visits every interleaving of a small
//! system, checks each clause of the invariant in each state, and finds the false
//! refusal of the algorithm we replaced.

use std::collections::{HashSet, VecDeque};

/// The most takers a run has.
const MAX_TAKERS: usize = 3;

/// Where a taker is in its call (`Phase` in the Lean file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Phase {
    Idle,
    Opened,
    Held,
    Released,
    Done,
    Dead,
}

/// How far a holder is with the file beside the state file (`Stage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Stage {
    Empty,
    Part,
    Whole,
}

/// One taker: its phase, the system's lock of its open file, the copies of its
/// descriptor in children, what it read and how far it wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Taker {
    phase: Phase,
    locked: bool,
    copies: u8,
    seen: Option<u8>,
    stage: Stage,
}

/// A file's contents: a number, and whether every byte is there.
type Contents = (u8, bool);

/// The lock file, its takers, and the state file the lock protects (`State`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct State {
    takers: [Taker; MAX_TAKERS],
    file: Contents,
    next: Option<Contents>,
    /// How many renames happened. The Lean model has no such count; the test compares
    /// it with the state file to see that no update is lost.
    renames: u8,
}

/// The bounds of a run and its algorithm.
#[derive(Debug, Clone, Copy)]
pub(super) struct Bounds {
    /// How many takers act.
    pub takers: usize,
    /// The most copies of one descriptor.
    pub copies: u8,
    /// The most updates of the state file.
    pub updates: u8,
    /// Whether a holder unlocks before it closes (`Step true`), or may only close
    /// (`Step false`).
    pub explicit: bool,
}

/// Nothing has happened (`init`).
pub(super) fn init() -> State {
    let taker =
        Taker { phase: Phase::Idle, locked: false, copies: 0, seen: None, stage: Stage::Empty };
    State { takers: [taker; MAX_TAKERS], file: (0, true), next: None, renames: 0 }
}

impl State {
    fn is_open(&self, a: usize) -> bool {
        matches!(self.takers[a].phase, Phase::Opened | Phase::Held | Phase::Released)
    }

    /// The lock of `a`'s open file after `a`'s own descriptor closes (`afterClose`).
    fn after_close(&self, a: usize) -> bool {
        self.takers[a].copies != 0 && self.takers[a].locked
    }

    /// The lock of `a`'s open file after one copy closes (`afterExec`).
    fn after_exec(&self, a: usize) -> bool {
        let t = &self.takers[a];
        let last = t.copies == 1 && matches!(t.phase, Phase::Done | Phase::Dead);
        !last && t.locked
    }

    /// Whether some open file holds the lock: the system refuses a taker.
    pub(super) fn is_busy(&self) -> bool {
        self.takers.iter().any(|t| t.locked)
    }

    /// The state with `change` applied to the taker `a`.
    fn with(&self, a: usize, change: impl FnOnce(&mut Taker)) -> State {
        let mut next = *self;
        change(&mut next.takers[a]);
        next
    }

    /// The steps of the lock itself that the taker `a` or a child can take.
    fn lock_steps(&self, a: usize, bounds: &Bounds, out: &mut Vec<State>) {
        let t = self.takers[a];
        let closed = self.after_close(a);
        match t.phase {
            // `openFile`.
            Phase::Idle => out.push(self.with(a, |t| t.phase = Phase::Opened)),
            // `acquire`.
            Phase::Opened if !self.is_busy() => out.push(self.with(a, |t| {
                (t.phase, t.locked, t.seen, t.stage) = (Phase::Held, true, None, Stage::Empty);
            })),
            // `refuse`.
            Phase::Opened => out.push(self.with(a, |t| {
                (t.phase, t.locked) = (Phase::Done, closed);
            })),
            Phase::Held => {
                // `unlock`.
                out.push(self.with(a, |t| (t.phase, t.locked) = (Phase::Released, false)));
                // `closeHeld`, the replaced algorithm.
                if !bounds.explicit {
                    out.push(self.with(a, |t| (t.phase, t.locked) = (Phase::Done, closed)));
                }
            }
            // `close`.
            Phase::Released => out.push(self.with(a, |t| {
                (t.phase, t.locked) = (Phase::Done, closed);
            })),
            Phase::Done | Phase::Dead => {}
        }
        if self.is_open(a) {
            // `fork`, bounded here and unbounded in Lean.
            if t.copies < bounds.copies {
                out.push(self.with(a, |t| t.copies += 1));
            }
            // `crash`.
            out.push(self.with(a, |t| (t.phase, t.locked) = (Phase::Dead, closed)));
        }
        if t.copies > 0 {
            // `exec`.
            let kept = self.after_exec(a);
            out.push(self.with(a, |t| (t.copies, t.locked) = (t.copies - 1, kept)));
        }
    }

    /// The steps of a holder `a` on the state file: `read`, `draft`, `sync`, `rename`.
    fn file_steps(&self, a: usize, bounds: &Bounds, out: &mut Vec<State>) {
        let t = self.takers[a];
        if t.phase != Phase::Held {
            return;
        }
        out.push(self.with(a, |t| t.seen = Some(self.file.0)));
        if let Some(read) = t.seen.filter(|read| *read < bounds.updates) {
            // The update we model adds one, so a lost update shows as a lost count. Lean
            // drafts any value, any number of times.
            let mut next = self.with(a, |t| t.stage = Stage::Part);
            next.next = Some((read + 1, false));
            out.push(next);
        }
        if let (Stage::Part, Some((value, false))) = (t.stage, self.next) {
            let mut next = self.with(a, |t| t.stage = Stage::Whole);
            next.next = Some((value, true));
            out.push(next);
        }
        if let (Stage::Whole, Some(contents)) = (t.stage, self.next) {
            let mut next = self.with(a, |t| (t.stage, t.seen) = (Stage::Empty, None));
            (next.file, next.next) = (contents, None);
            next.renames += 1;
            out.push(next);
        }
    }

    /// Every state one step away (`Step`).
    pub(super) fn steps(&self, bounds: &Bounds) -> Vec<State> {
        let mut out = Vec::new();
        for a in 0..bounds.takers {
            self.lock_steps(a, bounds, &mut out);
            self.file_steps(a, bounds, &mut out);
        }
        out
    }

    /// The clause of the invariant (`Inv`) that fails in this state, by its Lean name.
    pub(super) fn broken_clause(&self) -> Option<&'static str> {
        let held = |t: &Taker| t.phase == Phase::Held;
        let takers = &self.takers;
        if takers.iter().filter(|t| t.locked).count() > 1 {
            return Some("one");
        }
        if takers.iter().any(|t| held(t) && !t.locked) {
            return Some("heldLocked");
        }
        let owner = |t: &Taker| held(t) || (t.phase == Phase::Dead && t.copies > 0);
        if takers.iter().any(|t| t.locked && !owner(t)) {
            return Some("lockedOwner");
        }
        if !self.file.1 {
            return Some("fileWhole");
        }
        let staged = matches!(self.next, Some((_, true)));
        if takers.iter().any(|t| held(t) && t.stage == Stage::Whole && !staged) {
            return Some("staged");
        }
        if takers.iter().any(|t| held(t) && t.seen.is_some_and(|v| v != self.file.0)) {
            return Some("current");
        }
        if takers.iter().any(|t| held(t) && t.stage != Stage::Empty && t.seen.is_none()) {
            return Some("hasRead");
        }
        None
    }

    /// Whether some taker is in its critical section.
    pub(super) fn has_holder(&self) -> bool {
        self.takers.iter().any(|t| t.phase == Phase::Held)
    }

    /// Whether some taker has died.
    pub(super) fn has_dead(&self) -> bool {
        self.takers.iter().any(|t| t.phase == Phase::Dead)
    }

    /// Whether the taker `b` is dead.
    pub(super) fn is_dead(&self, b: usize) -> bool {
        self.takers[b].phase == Phase::Dead
    }

    /// The state after every child that copies `b`'s descriptor closed its copy
    /// (`execs s b (s.copies b)`), and whether `b`'s open file then holds the lock.
    pub(super) fn drained(&self, b: usize) -> (State, bool) {
        let mut state = *self;
        while state.takers[b].copies > 0 {
            let kept = state.after_exec(b);
            state = state.with(b, |t| (t.copies, t.locked) = (t.copies - 1, kept));
        }
        (state, state.takers[b].locked)
    }

    /// Whether the state file counts every rename: no update was lost.
    pub(super) fn counts_every_update(&self) -> bool {
        self.file.0 == self.renames
    }
}

/// Every state a run within `bounds` reaches, in the order a breadth-first search finds
/// them (`Reachable`).
pub(super) fn reachable(bounds: &Bounds) -> Vec<State> {
    let start = init();
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    let mut order = Vec::new();
    while let Some(state) = queue.pop_front() {
        order.push(state);
        for next in state.steps(bounds) {
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    order
}
