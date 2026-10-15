//! Threads and their stacks. Every thread Guidedog starts for parallel work runs
//! through `run_workers`, with a stack of `WORKER_STACK_BYTES`, so a conversion's stack
//! budget (`Limits::stack_bytes`) is known to fit; `run_with_stack` gives the same to
//! work the main thread would otherwise run on a stack the system chose.

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;
use std::thread;

/// The stack of every thread `run_workers` and `run_with_stack` start (8 MiB, the main
/// thread's usual stack on Linux and macOS).
pub const WORKER_STACK_BYTES: usize = 8 << 20;

/// What a thread keeps besides a conversion's stack budget: the frames above the guard
/// and `guidedoc::STACK_RESERVE` below the deepest one, with room for a debug build.
const STACK_HEADROOM: usize = 2 << 20;

/// The largest stack budget a thread with `stack` bytes can hold: all but 2 MiB of
/// headroom, and never less than half the stack. An 8 MiB thread holds 6 MiB.
pub fn stack_budget_limit(stack: usize) -> usize {
    stack.saturating_sub(STACK_HEADROOM).max(stack / 2)
}

/// The budget to use when none is asked for: `guidedoc::DEFAULT_STACK_BYTES`, or less on
/// a thread too small for it.
pub fn default_stack_budget(stack: usize) -> usize {
    guidedoc::DEFAULT_STACK_BYTES.min(stack_budget_limit(stack))
}

/// Runs `work(i)` on `count` threads at once (`i` from 0), each with a stack of
/// `WORKER_STACK_BYTES`, and returns when all have ended; `work` takes its share itself
/// (an index taken atomically, for one). A thread that cannot start is left out; when
/// none can, `work(0)` runs on the calling thread, so the work is always done. A panic
/// in a worker is raised again on the caller, once every worker has ended, with its
/// original payload.
pub fn run_workers<F>(count: usize, work: F)
where
    F: Fn(usize) + Sync,
{
    let panicked: Mutex<Option<Box<dyn std::any::Any + Send>>> = Mutex::new(None);
    let started = thread::scope(|scope| {
        let mut started = 0;
        for i in 0..count {
            let (work, panicked) = (&work, &panicked);
            let spawned = worker(i).spawn_scoped(scope, move || {
                if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| work(i))) {
                    let mut first = panicked.lock().unwrap_or_else(|p| p.into_inner());
                    first.get_or_insert(payload);
                }
            });
            if spawned.is_ok() {
                started += 1;
            }
        }
        started
    });
    if let Some(payload) = panicked.into_inner().unwrap_or_else(|p| p.into_inner()) {
        // Raised without calling the panic hook again: the worker's panic already did.
        panic::resume_unwind(payload);
    }
    if started == 0 && count > 0 {
        work(0);
    }
}

fn worker(i: usize) -> thread::Builder {
    thread::Builder::new().name(format!("guidedog-worker-{i}")).stack_size(WORKER_STACK_BYTES)
}

/// Runs `work` on a thread with a stack of `bytes` and returns its result, so a command
/// knows the stack its conversions run on; on the calling thread when no thread can be
/// started. A panic in `work` is raised again on the caller.
pub fn run_with_stack<R: Send>(bytes: usize, work: impl FnOnce() -> R + Send) -> R {
    let mut work = Some(work);
    let result = thread::scope(|scope| {
        let builder = thread::Builder::new().name("guidedog-main".into()).stack_size(bytes);
        let slot = &mut work;
        let handle = builder.spawn_scoped(scope, move || slot.take().map(|w| w()));
        handle.ok().map(|h| h.join())
    });
    match result {
        Some(Ok(Some(value))) => value,
        Some(Err(payload)) => panic::resume_unwind(payload),
        // The thread did not start: `work` is still here.
        _ => match work.take() {
            Some(w) => w(),
            None => unreachable!("a thread that ran the work returned its value"),
        },
    }
}
