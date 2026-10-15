//! Windows: the console's echo of typed input, for reading a token. This is an FFI
//! module: every foreign call is here, each `unsafe` block with the reason it is sound.

use std::ffi::c_void;

/// `STD_INPUT_HANDLE`, `(DWORD)-10`.
const STD_INPUT_HANDLE: u32 = -10i32 as u32;
const ENABLE_ECHO_INPUT: u32 = 0x0004;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(which: u32) -> *mut c_void;
    fn GetConsoleMode(console: *mut c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(console: *mut c_void, mode: u32) -> i32;
}

fn input() -> *mut c_void {
    // SAFETY: GetStdHandle takes a constant and returns a handle (or null/invalid), which
    // the console functions below check themselves.
    unsafe { GetStdHandle(STD_INPUT_HANDLE) }
}

/// Turns the echo off; returns the mode to restore, or `None` for no console.
pub(super) fn hide_input() -> Option<u32> {
    let console = input();
    let mut mode = 0u32;
    // SAFETY: `mode` is a valid place for the result; an invalid handle makes the call
    // fail, which is checked.
    if unsafe { GetConsoleMode(console, &mut mode) } == 0 {
        return None;
    }
    // SAFETY: the handle was accepted by GetConsoleMode just above.
    unsafe { SetConsoleMode(console, mode & !ENABLE_ECHO_INPUT) };
    Some(mode)
}

/// Restores the console mode `mode`.
pub(super) fn restore_input(mode: u32) {
    // SAFETY: setting a mode the console reported before; a failure changes nothing.
    unsafe { SetConsoleMode(input(), mode) };
}
