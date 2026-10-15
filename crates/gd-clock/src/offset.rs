//! The local time zone's offset from UTC, the one part of the clock the standard library
//! cannot give: it has no time zones. This is the crate's FFI module; every foreign call
//! is here, each `unsafe` block with the reason it is sound.
//!
//! - Unix (Linux, Android, the Apple systems and the BSDs, 64-bit): the C library's
//!   `localtime_r`, which reads `TZ` and the system zone database.
//! - Windows: `SystemTimeToTzSpecificLocalTime`, which applies the zone's rules
//!   (daylight saving included) for the moment asked about.
//! - Elsewhere (32-bit targets, whose `time_t` width varies, and systems whose
//!   `struct tm` has no `tm_gmtoff`, such as illumos): no offset, so local time is UTC.

/// The largest offset accepted; real zones stay within 14 hours.
const MAX_OFFSET: i64 = 86_400;

/// Returns the local zone's offset from UTC in seconds (east positive) at `seconds`
/// after 1970-01-01 00:00:00 UTC, or `None` when the system cannot tell.
pub fn local_offset(seconds: i64) -> Option<i64> {
    system::offset_seconds(seconds).filter(|offset| offset.abs() <= MAX_OFFSET)
}

#[cfg(all(
    target_pointer_width = "64",
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
    )
))]
mod system {
    use std::ffi::{c_char, c_int, c_long};

    /// `struct tm` as glibc, musl, Bionic, Apple's libc and the BSDs declare it: nine
    /// `int`s, then the BSD extensions `tm_gmtoff` (a `long`) and `tm_zone`. On these
    /// 64-bit targets `time_t` is 64 bits wide.
    #[repr(C)]
    struct Tm {
        tm_sec: c_int,
        tm_min: c_int,
        tm_hour: c_int,
        tm_mday: c_int,
        tm_mon: c_int,
        tm_year: c_int,
        tm_wday: c_int,
        tm_yday: c_int,
        tm_isdst: c_int,
        tm_gmtoff: c_long,
        tm_zone: *const c_char,
    }

    unsafe extern "C" {
        fn tzset();
        fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
    }

    pub(super) fn offset_seconds(seconds: i64) -> Option<i64> {
        let mut tm = Tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: std::ptr::null(),
        };
        // SAFETY: tzset takes no arguments; it reads TZ, which safe Rust cannot change
        // while this runs (setting the environment is `unsafe` in this edition and the
        // workspace denies unsafe code outside its FFI modules).
        unsafe { tzset() };
        // SAFETY: both pointers refer to live, aligned locals of the types the function
        // expects (`time_t` is i64 on these targets, and `Tm` matches `struct tm`); it
        // writes only into `tm` and returns null on failure. `tm_zone` is never read.
        let result = unsafe { localtime_r(&seconds, &mut tm) };
        if result.is_null() {
            return None;
        }
        // `long` is i64 on these targets.
        Some(tm.tm_gmtoff)
    }
}

#[cfg(windows)]
mod system {
    use std::ffi::c_void;

    use crate::civil::{Date, DateTime};

    /// Windows `FILETIME`: 100-nanosecond ticks since 1601-01-01 00:00:00 UTC.
    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    /// Windows `SYSTEMTIME`, eight `WORD`s.
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FileTimeToSystemTime(file_time: *const FileTime, out: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(
            zone: *const c_void,
            universal: *const SystemTime,
            local: *mut SystemTime,
        ) -> i32;
    }

    /// Seconds from 1601-01-01 to 1970-01-01.
    const WINDOWS_EPOCH_SHIFT: i64 = 11_644_473_600;

    pub(super) fn offset_seconds(seconds: i64) -> Option<i64> {
        let ticks = u64::try_from(seconds.checked_add(WINDOWS_EPOCH_SHIFT)?)
            .ok()?
            .checked_mul(10_000_000)?;
        let file_time = FileTime { low: ticks as u32, high: (ticks >> 32) as u32 };
        let mut universal = SystemTime::default();
        // SAFETY: both pointers refer to live, aligned locals of the `#[repr(C)]` types
        // the function expects; it writes only into `universal` and returns 0 on failure
        // (a time past the year 30827).
        if unsafe { FileTimeToSystemTime(&file_time, &mut universal) } == 0 {
            return None;
        }
        let mut local = SystemTime::default();
        // SAFETY: a null zone means the current zone; the other pointers refer to live,
        // aligned locals of the expected type, and the function writes only `local` and
        // returns 0 on failure.
        let ok =
            unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &universal, &mut local) };
        if ok == 0 {
            return None;
        }
        Some(unix_of(&local) - unix_of(&universal))
    }

    fn unix_of(t: &SystemTime) -> i64 {
        let date = Date { year: i64::from(t.year), month: t.month as u8, day: t.day as u8 };
        DateTime { date, hour: t.hour as u8, minute: t.minute as u8, second: t.second as u8 }
            .unix()
    }
}

#[cfg(not(any(
    windows,
    all(
        target_pointer_width = "64",
        any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "dragonfly",
        )
    )
)))]
mod system {
    /// No zone information here: the local time is UTC.
    pub(super) fn offset_seconds(_seconds: i64) -> Option<i64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_offset_is_within_a_day() {
        for seconds in [0, 951_782_400, 1_790_000_000, -2_000_000_000] {
            if let Some(offset) = local_offset(seconds) {
                assert!(offset.abs() <= MAX_OFFSET, "{offset}");
            }
        }
    }
}
