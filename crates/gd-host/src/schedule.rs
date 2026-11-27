//! Which jobs may run at once within a memory capacity (GDS 0020).
//!
//! A [`Schedule`] knows each job's weight (the memory reserved for it while it runs), the
//! capacity all running jobs may reserve together, and the slots (the most jobs that run
//! at once). It decides which job starts next; the caller runs the jobs and reports their
//! ends. The rules are the transitions of the model in GDS 0020 (`start`, `succeed`,
//! `fail`), which `docs/gds/proofs/Gds/R0020/Schedule.lean` proves correct and the tests
//! of this module enumerate:
//!
//! * a job above a failed job never starts, as a sequential run stops at its first
//!   failure;
//! * a job starts at once when nothing runs, whatever its weight;
//! * otherwise a job starts only when its weight fits beside the running weights, a slot
//!   is free, and the caller's `room` consents.
//!
//! ```
//! use gd_host::{JobPhase, Schedule};
//!
//! let mut schedule = Schedule::new(vec![3, 2, 2], 4, 8);
//! let first = schedule.next(|_| true);
//! assert_eq!(first, Some(0)); // the heaviest job first
//! schedule.start(0);
//! assert_eq!(schedule.next(|_| true), None); // 3 + 2 exceeds the capacity of 4
//! schedule.finish(0, true);
//! schedule.start(1);
//! schedule.start(2);
//! schedule.finish(1, true);
//! schedule.finish(2, true);
//! assert!(schedule.is_ended());
//! assert_eq!(schedule.phase(2), JobPhase::Succeeded);
//! ```

/// Where a job is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobPhase {
    /// Not started.
    Waiting,
    /// Started and not ended.
    Running,
    /// Ended with success.
    Succeeded,
    /// Ended with failure.
    Failed,
}

/// The jobs of a parallel run, numbered in the order a sequential run takes them, and the
/// memory they may reserve together. Plain data; one schedule per run.
#[derive(Debug, Clone)]
pub struct Schedule {
    weights: Box<[u64]>,
    phases: Vec<JobPhase>,
    capacity: u64,
    slots: usize,
    load: u64,
    running: usize,
}

impl Schedule {
    /// A schedule of `weights.len()` waiting jobs, which may reserve `capacity` bytes
    /// together and run at most `slots` at once (0 counts as 1).
    pub fn new(weights: Vec<u64>, capacity: u64, slots: usize) -> Schedule {
        let phases = vec![JobPhase::Waiting; weights.len()];
        Schedule {
            weights: weights.into_boxed_slice(),
            phases,
            capacity,
            slots: slots.max(1),
            load: 0,
            running: 0,
        }
    }

    /// The number of jobs.
    pub fn job_count(&self) -> usize {
        self.phases.len()
    }

    /// Where `job` is. Panics when `job` is not a job of the schedule.
    pub fn phase(&self, job: usize) -> JobPhase {
        self.phases[job]
    }

    /// The memory the running jobs reserve.
    pub fn load(&self) -> u64 {
        self.load
    }

    /// How many jobs run.
    pub fn running_count(&self) -> usize {
        self.running
    }

    /// Whether `job` may start now: the guard of `start`. `room` is asked about the
    /// job's weight only when other jobs run.
    pub fn may_start(&self, job: usize, room: impl Fn(u64) -> bool) -> bool {
        if self.phases.get(job) != Some(&JobPhase::Waiting) || self.failed_before(job) {
            return false;
        }
        if self.running == 0 {
            return true;
        }
        let weight = self.weights[job];
        self.load.saturating_add(weight) <= self.capacity
            && self.running < self.slots
            && room(weight)
    }

    /// The job to start next, if one may start: the heaviest, and of equal weights the
    /// earliest, so that light jobs fill the gaps at the end.
    pub fn next(&self, room: impl Fn(u64) -> bool) -> Option<usize> {
        let mut best: Option<usize> = None;
        for job in 0..self.phases.len() {
            let heavier = best.is_none_or(|b| self.weights[job] > self.weights[b]);
            if heavier && self.may_start(job, &room) {
                best = Some(job);
            }
        }
        best
    }

    /// Marks `job` as running. Panics when it may not start, whatever `room` says: the
    /// caller asks [`Schedule::next`] or [`Schedule::may_start`] first.
    pub fn start(&mut self, job: usize) {
        assert!(self.may_start(job, |_| true), "job {job} may not start");
        self.phases[job] = JobPhase::Running;
        self.load = self.load.saturating_add(self.weights[job]);
        self.running += 1;
    }

    /// Marks the running `job` as ended. Panics when it does not run.
    pub fn finish(&mut self, job: usize, succeeded: bool) {
        assert_eq!(self.phases[job], JobPhase::Running, "job {job} does not run");
        self.phases[job] = if succeeded { JobPhase::Succeeded } else { JobPhase::Failed };
        self.load = self.load.saturating_sub(self.weights[job]);
        self.running -= 1;
    }

    /// Whether no job runs and none may start: the run has ended. A sequential run would
    /// have ended in the same outcome (GDS 0020, Theorems 4 and 5).
    pub fn is_ended(&self) -> bool {
        self.running == 0 && (0..self.phases.len()).all(|job| !self.may_start(job, |_| true))
    }

    /// The first job that failed, in plan order.
    pub fn first_failure(&self) -> Option<usize> {
        self.phases.iter().position(|p| *p == JobPhase::Failed)
    }

    fn failed_before(&self, job: usize) -> bool {
        self.phases[..job].contains(&JobPhase::Failed)
    }
}

#[cfg(test)]
mod tests;
