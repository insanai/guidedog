//! The model of GDS 0020 enumerated: every interleaving of small plans, with every
//! reachable state checked against the invariant and the theorems.

use std::collections::HashSet;

use super::*;

/// A plan: weights, capacity, slots, and which jobs fail.
struct Plan {
    weights: Vec<u64>,
    capacity: u64,
    slots: usize,
    fails: Vec<bool>,
}

/// What the enumeration of one plan found.
#[derive(Default)]
struct Found {
    states: usize,
    ended: usize,
    longest: usize,
}

/// Visits every state reachable from the initial one. `room` only removes starts, so
/// every run under any `room` is a run where `room` always consents; checking those
/// states checks them all.
fn explore(plan: &Plan) -> Found {
    let mut found = Found::default();
    let mut seen = HashSet::new();
    let start = Schedule::new(plan.weights.clone(), plan.capacity, plan.slots);
    let mut stack = vec![(start, 0usize)];
    while let Some((schedule, depth)) = stack.pop() {
        if !seen.insert(schedule.phases.clone()) {
            continue;
        }
        found.states += 1;
        found.longest = found.longest.max(depth);
        check_state(plan, &schedule);
        let mut moved = false;
        for (job, &fails) in plan.fails.iter().enumerate() {
            if schedule.may_start(job, |_| true) {
                // A job above a failed job never starts, as in a sequential run.
                let failed = (0..job).any(|k| schedule.phase(k) == JobPhase::Failed);
                assert!(!failed, "job {job} starts above a failure: {:?}", schedule.phases);
                let mut next = schedule.clone();
                next.start(job);
                stack.push((next, depth + 1));
                moved = true;
            }
            if schedule.phase(job) == JobPhase::Running {
                let mut next = schedule.clone();
                next.finish(job, !fails);
                stack.push((next, depth + 1));
                moved = true;
            }
        }
        assert_eq!(moved, !schedule.is_ended(), "ended means no step: {:?}", schedule.phases);
        if !moved {
            found.ended += 1;
            check_outcome(plan, &schedule);
        }
    }
    found
}

/// The invariant J1 to J5 and Theorem 6 in one state.
fn check_state(plan: &Plan, s: &Schedule) {
    let running: Vec<usize> =
        (0..s.job_count()).filter(|&j| s.phase(j) == JobPhase::Running).collect();
    let load: u64 = running.iter().map(|&j| plan.weights[j]).sum();
    assert_eq!(s.load(), load);
    assert_eq!(s.running_count(), running.len());
    // J1 and Theorem 1.
    assert!(load <= plan.capacity || running.len() == 1, "memory: {:?}", s.phases);
    // Corollary 1a.
    if plan.weights.iter().all(|&w| w <= plan.capacity) {
        assert!(load <= plan.capacity);
    }
    // J2 and Theorem 2.
    assert!(running.len() <= plan.slots.max(1));
    // J4 and J5.
    for (j, &fails) in plan.fails.iter().enumerate() {
        match s.phase(j) {
            JobPhase::Succeeded => assert!(!fails),
            JobPhase::Failed => assert!(fails),
            JobPhase::Waiting | JobPhase::Running => {}
        }
    }
    // Theorem 6: when nothing runs, the first waiting job without a failure before it
    // may start, whatever its weight and whatever `room` says.
    if running.is_empty() {
        let first = (0..s.job_count()).find(|&j| s.phase(j) == JobPhase::Waiting);
        if let Some(j) = first.filter(|&j| !s.failed_before(j)) {
            assert!(s.may_start(j, |_| false));
        }
    }
    // `next` picks a job that may start, and the heaviest of them.
    if let Some(j) = s.next(|_| true) {
        assert!(s.may_start(j, |_| true));
        let heaviest = (0..s.job_count())
            .filter(|&k| s.may_start(k, |_| true))
            .map(|k| plan.weights[k])
            .max();
        assert_eq!(Some(plan.weights[j]), heaviest);
    }
}

/// Theorems 4 and 5: an ended run has the outcome of the sequential run.
fn check_outcome(plan: &Plan, s: &Schedule) {
    // The sequential run stops at the first job that fails, or runs them all.
    let sequential = plan.fails.iter().position(|&f| f);
    assert_eq!(s.first_failure(), sequential, "first failure: {:?}", s.phases);
    let succeeded_before = sequential.unwrap_or(plan.weights.len());
    for j in 0..succeeded_before {
        assert_eq!(s.phase(j), JobPhase::Succeeded, "job {j}: {:?}", s.phases);
    }
    if let Some(f) = sequential {
        assert_eq!(s.phase(f), JobPhase::Failed);
    }
}

/// Every plan of `n` jobs with weights from `weights`, each failing or not.
fn plans(n: usize, weights: &[u64], capacity: u64, slots: usize) -> Vec<Plan> {
    let mut out = Vec::new();
    let combinations = weights.len().pow(n as u32);
    for w in 0..combinations {
        for f in 0..(1usize << n) {
            let mut rest = w;
            let mut ws = Vec::with_capacity(n);
            for _ in 0..n {
                ws.push(weights[rest % weights.len()]);
                rest /= weights.len();
            }
            let fails = (0..n).map(|j| f >> j & 1 == 1).collect();
            out.push(Plan { weights: ws, capacity, slots, fails });
        }
    }
    out
}

#[test]
fn the_schedule_keeps_the_invariant_in_every_interleaving() {
    let mut total = Found::default();
    for (n, slots) in [(1, 1), (2, 1), (3, 2), (4, 2), (4, 3), (4, 8)] {
        // Capacity 4 with weights up to 5: some jobs fit together, one never fits.
        for plan in plans(n, &[0, 1, 2, 3, 5], 4, slots) {
            let found = explore(&plan);
            assert!(found.ended >= 1);
            // Theorem 3: every run has at most 2 n steps.
            assert!(found.longest <= 2 * n);
            total.states += found.states;
            total.ended += found.ended;
        }
    }
    assert!(total.states > 100_000, "explored {} states", total.states);
}

#[test]
fn a_job_heavier_than_the_capacity_runs_alone() {
    let mut s = Schedule::new(vec![1, 9, 1], 4, 4);
    s.start(1);
    assert_eq!(s.next(|_| true), None);
    s.finish(1, true);
    assert_eq!(s.next(|_| true), Some(0));
    s.start(0);
    assert!(!s.may_start(1, |_| true), "a job that ended does not start again");
    assert!(s.may_start(2, |_| true));
}

#[test]
fn room_holds_back_a_job_only_while_others_run() {
    let mut s = Schedule::new(vec![1, 1], 4, 4);
    assert!(s.may_start(0, |_| false));
    s.start(0);
    assert!(!s.may_start(1, |_| false));
    assert!(s.may_start(1, |_| true));
}

#[test]
fn no_job_above_a_failure_starts() {
    let mut s = Schedule::new(vec![1, 1, 1], 4, 4);
    s.start(1);
    s.finish(1, false);
    assert!(s.may_start(0, |_| true), "a job before the failure still runs");
    assert!(!s.may_start(2, |_| true));
    s.start(0);
    s.finish(0, true);
    assert!(s.is_ended());
    assert_eq!(s.first_failure(), Some(1));
}

#[test]
#[should_panic(expected = "may not start")]
fn starting_a_job_that_may_not_start_panics() {
    let mut s = Schedule::new(vec![1, 1], 4, 1);
    s.start(0);
    s.start(1);
}
