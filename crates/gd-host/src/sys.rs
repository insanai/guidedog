//! The crate's only foreign calls: installing the Ctrl+C handler (C's `signal`, which
//! Linux, macOS, and Windows' C runtime all provide) and swapping two paths in one step.
//! The standard library offers neither.

use std::io;
use std::path::Path;

/// C's `int`.
type CInt = std::ffi::c_int;

/// SIGINT is 2 on every platform Guidedog builds for (POSIX and the Windows C runtime).
const SIGINT: CInt = 2;

/// C's `SIG_ERR`: `(void (*)(int)) -1`.
const SIG_ERR: usize = usize::MAX;

unsafe extern "C" {
    /// `signal(sig, handler)`: installs `handler` for `sig` and returns the previous
    /// handler, or `SIG_ERR`.
    fn signal(sig: CInt, handler: usize) -> usize;
}

/// Makes `handler` run on Ctrl+C (SIGINT) for the whole process; false when the system
/// refuses. The handler must be async-signal-safe: it may only store to atomics.
pub(crate) fn on_interrupt(handler: extern "C" fn(CInt)) -> bool {
    // SAFETY: `signal` is the C standard library's; it is given a valid signal number and
    // the address of an `extern "C" fn(c_int)`, the handler type it expects. The handlers
    // this crate passes only store to an atomic, which is async-signal-safe.
    unsafe { signal(SIGINT, handler as usize) != SIG_ERR }
}

/// Swaps `a` and `b`; `Ok(false)` where the system cannot.
#[cfg(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos"))]
pub(crate) fn exchange(a: &Path, b: &Path) -> io::Result<bool> {
    let a = c_path(a)?;
    let b = c_path(b)?;
    if swap_paths(&a, &b) == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::Unsupported || unsupported(error.raw_os_error()) {
        return Ok(false);
    }
    Err(error)
}

/// Swaps `a` and `b`; this system cannot, so nothing changes.
#[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos")))]
pub(crate) fn exchange(a: &Path, b: &Path) -> io::Result<bool> {
    std::fs::symlink_metadata(a)?;
    std::fs::symlink_metadata(b)?;
    Ok(false)
}

#[cfg(any(all(target_os = "linux", target_env = "gnu"), target_os = "macos"))]
fn c_path(path: &Path) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a path holds a NUL byte"))
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod linux {
    use super::CInt;
    use std::ffi::c_char;

    pub(super) const AT_FDCWD: CInt = -100;
    pub(super) const RENAME_EXCHANGE: u32 = 2;
    pub(super) const EINVAL: i32 = 22;
    pub(super) const ENOSYS: i32 = 38;
    pub(super) const EOPNOTSUPP: i32 = 95;

    unsafe extern "C" {
        /// glibc's `renameat2` (2.28 and later).
        pub(super) fn renameat2(
            olddirfd: CInt,
            oldpath: *const c_char,
            newdirfd: CInt,
            newpath: *const c_char,
            flags: u32,
        ) -> CInt;
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn swap_paths(a: &std::ffi::CStr, b: &std::ffi::CStr) -> CInt {
    use linux::{AT_FDCWD, RENAME_EXCHANGE, renameat2};
    // SAFETY: both pointers come from live, NUL-terminated `CStr`s that outlive the call,
    // and AT_FDCWD makes the paths relative to the working directory as `rename` does.
    unsafe { renameat2(AT_FDCWD, a.as_ptr(), AT_FDCWD, b.as_ptr(), RENAME_EXCHANGE) }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn unsupported(code: Option<i32>) -> bool {
    matches!(code, Some(linux::EINVAL | linux::ENOSYS | linux::EOPNOTSUPP))
}

#[cfg(target_os = "macos")]
mod macos {
    use super::CInt;
    use std::ffi::c_char;

    pub(super) const RENAME_SWAP: u32 = 2;
    pub(super) const EINVAL: i32 = 22;
    pub(super) const ENOTSUP: i32 = 45;

    unsafe extern "C" {
        /// `renamex_np` (macOS 10.12 and later).
        pub(super) fn renamex_np(from: *const c_char, to: *const c_char, flags: u32) -> CInt;
    }
}

#[cfg(target_os = "macos")]
fn swap_paths(a: &std::ffi::CStr, b: &std::ffi::CStr) -> CInt {
    // SAFETY: both pointers come from live, NUL-terminated `CStr`s that outlive the call.
    unsafe { macos::renamex_np(a.as_ptr(), b.as_ptr(), macos::RENAME_SWAP) }
}

#[cfg(target_os = "macos")]
fn unsupported(code: Option<i32>) -> bool {
    matches!(code, Some(macos::EINVAL | macos::ENOTSUP))
}
