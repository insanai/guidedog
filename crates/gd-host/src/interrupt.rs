//! Ctrl+C sets a flag instead of ending the process, so staged files are removed and
//! the command exits with status 130. Conversions poll the flag through
//! `Request::cancel`; a Typst program in the same process group gets the signal itself.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::sys;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_interrupt(_signal: std::ffi::c_int) {
    // Only an atomic store: the one thing a signal handler may safely do here.
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Makes Ctrl+C (SIGINT) set the interrupt flag instead of ending the process, for the
/// whole process. A program calls it once, at the start; false when the system refused.
pub fn catch_interrupts() -> bool {
    sys::on_interrupt(on_interrupt)
}

/// Reports whether Ctrl+C was pressed (or `request_interrupt` called) since the start.
pub fn was_interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

/// Sets the interrupt flag as Ctrl+C does, for an embedder that cancels by other means
/// (a server's stop button, a test).
pub fn request_interrupt() {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Clears the interrupt flag, for a long-running embedder that goes on after a cancel.
pub fn clear_interrupt() {
    INTERRUPTED.store(false, Ordering::SeqCst);
}

/// The probe a `guidedoc::Request` polls (`request.cancel`) to stop at Ctrl+C; it holds
/// no state of its own.
pub fn cancel_probe() -> &'static (dyn Fn() -> bool + Sync) {
    &was_interrupted
}
