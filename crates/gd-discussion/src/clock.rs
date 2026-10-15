//! The date a change is stamped with. As in other reproducible builds,
//! `SOURCE_DATE_EPOCH` (seconds since 1970, UTC) fixes it; otherwise it is the local date.

use std::time::{SystemTime, UNIX_EPOCH};

/// A civil date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Date {
    pub year: i64,
    pub month: u32,
    pub day: u32,
}

/// Today's date as `YYYY-MM-DD`: `SOURCE_DATE_EPOCH`'s when it is set and a whole number,
/// otherwise the local date (the UTC date where the local time zone cannot be read).
pub fn today() -> String {
    let d = now();
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

/// Today's date, as [`today`] chooses it.
pub(crate) fn now() -> Date {
    let fixed = std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|t| t.trim().parse().ok());
    if let Some(seconds) = fixed {
        return date_from_unix(seconds);
    }
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
    };
    let offset = local::offset_seconds(seconds).unwrap_or(0);
    date_from_unix(seconds.saturating_add(offset))
}

/// The civil date of a Unix timestamp (Howard Hinnant's days-from-civil inverse).
pub(crate) fn date_from_unix(seconds: i64) -> Date {
    let z = seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Date { year, month: month as u32, day: day as u32 }
}

/// The local time zone's offset from UTC, read through the C library's `localtime_r`;
/// Rust's standard library has no time zones.
#[cfg(all(unix, target_pointer_width = "64"))]
mod local {
    /// `struct tm` as glibc, musl and the BSDs lay it out on 64-bit targets.
    #[repr(C)]
    #[derive(Default)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: usize,
    }

    unsafe extern "C" {
        fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
    }

    /// The offset in seconds at `seconds`, or `None` when the C library cannot tell.
    pub(super) fn offset_seconds(seconds: i64) -> Option<i64> {
        let mut tm = Tm::default();
        // SAFETY: both pointers refer to live, properly aligned locals of the types the C
        // function expects (time_t is 64-bit here); it writes only into `tm` and returns
        // null on failure. `tm_zone` is a pointer-sized integer that is never used.
        let result = unsafe { localtime_r(&seconds, &mut tm) };
        if result.is_null() || tm.tm_gmtoff.abs() > 86_400 {
            return None;
        }
        Some(tm.tm_gmtoff)
    }
}

/// Elsewhere the local date is the UTC date.
#[cfg(not(all(unix, target_pointer_width = "64")))]
mod local {
    pub(super) fn offset_seconds(_seconds: i64) -> Option<i64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_timestamps_become_civil_dates() {
        assert_eq!(date_from_unix(0), Date { year: 1970, month: 1, day: 1 });
        assert_eq!(date_from_unix(951_782_400), Date { year: 2000, month: 2, day: 29 });
        assert_eq!(date_from_unix(-86_400), Date { year: 1969, month: 12, day: 31 });
    }
}
