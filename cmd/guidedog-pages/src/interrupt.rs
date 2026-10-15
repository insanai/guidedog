//! Ctrl+C sets a flag instead of ending the process. This is the program's only FFI
//! module: C's `signal`, which Linux, macOS, and Windows' C runtime all provide, and
//! which the standard library does not offer.

use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

/// SIGINT is 2 on every platform Guidedog builds for (POSIX and the Windows C runtime).
const SIGINT: c_int = 2;
/// C's `SIG_ERR`: `(void (*)(int)) -1`.
const SIG_ERR: usize = usize::MAX;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    /// `signal(sig, handler)`: installs `handler` for `sig` and returns the previous
    /// handler, or `SIG_ERR`.
    fn signal(sig: c_int, handler: usize) -> usize;
}

extern "C" fn on_interrupt(_signal: c_int) {
    // Only an atomic store: the one thing a signal handler may safely do here.
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Makes Ctrl+C set the interrupt flag for the whole process; false when the system
/// refused.
pub fn catch_interrupts() -> bool {
    let handler: extern "C" fn(c_int) = on_interrupt;
    // SAFETY: `signal` is the C standard library's; it is given a valid signal number and
    // the address of an `extern "C" fn(c_int)`, the handler type it expects. The handler
    // only stores to an atomic, which is async-signal-safe.
    unsafe { signal(SIGINT, handler as usize) != SIG_ERR }
}

/// Whether Ctrl+C was pressed since the start.
pub fn was_interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}
