//! The crate's only foreign calls: installing the Ctrl+C handler (C's `signal`, which
//! Linux, macOS, and Windows' C runtime all provide), swapping two paths in one step,
//! and asking macOS and Windows for memory figures that Linux gives in `/proc`
//! (GDS 0020). The standard library offers none of them.

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

/// The peak resident memory of this process in bytes, from `getrusage`, whose
/// `ru_maxrss` macOS gives in bytes (Linux gives kilobytes, and reads `/proc` instead);
/// `None` when the call fails.
#[cfg(target_os = "macos")]
pub(crate) fn peak_resident() -> Option<u64> {
    /// `struct timeval` on 64-bit macOS: `time_t` and `suseconds_t`, padded to 16 bytes.
    #[repr(C)]
    struct Timeval {
        seconds: i64,
        microseconds: i32,
    }
    /// `struct rusage`: two times, then fourteen `long` counters.
    #[repr(C)]
    struct Rusage {
        user: Timeval,
        system: Timeval,
        counters: [std::ffi::c_long; 14],
    }
    const RUSAGE_SELF: CInt = 0;
    unsafe extern "C" {
        fn getrusage(who: CInt, usage: *mut Rusage) -> CInt;
    }
    let mut usage = Rusage {
        user: Timeval { seconds: 0, microseconds: 0 },
        system: Timeval { seconds: 0, microseconds: 0 },
        counters: [0; 14],
    };
    // SAFETY: `usage` is a live, writable `struct rusage` with the C layout macOS
    // declares in <sys/resource.h>; `getrusage` writes it and reads nothing else.
    let status = unsafe { getrusage(RUSAGE_SELF, &mut usage) };
    // The first counter is `ru_maxrss`.
    (status == 0).then(|| u64::try_from(usage.counters[0]).unwrap_or(0))
}

/// Windows' answers about memory, from kernel32 (which every Rust program on Windows
/// links).
#[cfg(windows)]
mod windows {
    use std::ffi::c_void;

    /// `MEMORYSTATUSEX`: its size first, then the figures in bytes.
    #[repr(C)]
    pub(super) struct MemoryStatus {
        pub length: u32,
        pub memory_load: u32,
        pub total_physical: u64,
        pub available_physical: u64,
        pub total_page_file: u64,
        pub available_page_file: u64,
        pub total_virtual: u64,
        pub available_virtual: u64,
        pub available_extended_virtual: u64,
    }

    /// `PROCESS_MEMORY_COUNTERS`: its size first, then the counters.
    #[repr(C)]
    pub(super) struct MemoryCounters {
        pub size: u32,
        pub page_faults: u32,
        pub peak_working_set: usize,
        pub working_set: usize,
        pub quota_peak_paged_pool: usize,
        pub quota_paged_pool: usize,
        pub quota_peak_non_paged_pool: usize,
        pub quota_non_paged_pool: usize,
        pub page_file: usize,
        pub peak_page_file: usize,
    }

    unsafe extern "system" {
        pub(super) fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        pub(super) fn GetCurrentProcess() -> *mut c_void;
        pub(super) fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut MemoryCounters,
            size: u32,
        ) -> i32;
    }
}

/// The physical memory available now, in bytes (`ullAvailPhys`); `None` when the call
/// fails.
#[cfg(windows)]
pub(crate) fn available_physical() -> Option<u64> {
    let mut status = windows::MemoryStatus {
        length: std::mem::size_of::<windows::MemoryStatus>() as u32,
        memory_load: 0,
        total_physical: 0,
        available_physical: 0,
        total_page_file: 0,
        available_page_file: 0,
        total_virtual: 0,
        available_virtual: 0,
        available_extended_virtual: 0,
    };
    // SAFETY: `status` is a live, writable MEMORYSTATUSEX whose `dwLength` holds its
    // size, as the function requires; it writes the structure and nothing else.
    let ok = unsafe { windows::GlobalMemoryStatusEx(&mut status) };
    (ok != 0).then_some(status.available_physical)
}

/// The peak working set of this process, in bytes (`PeakWorkingSetSize`); `None` when
/// the call fails.
#[cfg(windows)]
pub(crate) fn peak_working_set() -> Option<u64> {
    let size = std::mem::size_of::<windows::MemoryCounters>();
    let mut counters = windows::MemoryCounters {
        size: size as u32,
        page_faults: 0,
        peak_working_set: 0,
        working_set: 0,
        quota_peak_paged_pool: 0,
        quota_paged_pool: 0,
        quota_peak_non_paged_pool: 0,
        quota_non_paged_pool: 0,
        page_file: 0,
        peak_page_file: 0,
    };
    // SAFETY: GetCurrentProcess returns a pseudo handle that needs no closing; `counters`
    // is a live, writable PROCESS_MEMORY_COUNTERS of the size passed, which the function
    // writes and nothing else.
    let ok = unsafe {
        windows::K32GetProcessMemoryInfo(windows::GetCurrentProcess(), &mut counters, size as u32)
    };
    (ok != 0).then_some(counters.peak_working_set as u64)
}
