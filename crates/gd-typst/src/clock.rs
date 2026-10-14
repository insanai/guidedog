//! One instant for the whole compile. Explicit offsets in `datetime.today(offset: ..)` use
//! its UTC time; `auto` uses the local date. `SOURCE_DATE_EPOCH` fixes both to UTC, as
//! Typst's CLI does. Keeping the time of day matters: a timezone offset can cross midnight.

#![cfg_attr(not(feature = "embedded"), allow(dead_code))]

use std::time::{SystemTime, UNIX_EPOCH};

/// A civil date and time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Civil {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// The compile's instant in UTC, and the local date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Clock {
    pub utc: Civil,
    pub local: Civil,
}

/// Captures now, or `SOURCE_DATE_EPOCH` when it is set and in range.
pub(crate) fn compiler_clock() -> Clock {
    let fixed = std::env::var("SOURCE_DATE_EPOCH").ok().and_then(|t| compiler_epoch(&t));
    let seconds = fixed.unwrap_or_else(|| match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
    });
    let utc = civil_from_unix(seconds);
    let local = match fixed {
        Some(_) => utc,
        None => local::offset_seconds(seconds)
            .map_or(utc, |offset| civil_from_unix(seconds.saturating_add(offset))),
    };
    Clock { utc, local }
}

/// Parses a `SOURCE_DATE_EPOCH`, rejecting a timestamp outside a nanosecond clock's range
/// (as the Odin host did) before anything multiplies it.
pub(crate) fn compiler_epoch(text: &str) -> Option<i64> {
    const SCALE: i64 = 1_000_000_000;
    let epoch: i64 = text.parse().ok()?;
    (i64::MIN / SCALE..=i64::MAX / SCALE).contains(&epoch).then_some(epoch)
}

/// The civil date and time of a Unix timestamp (Howard Hinnant's days-from-civil inverse).
pub(crate) fn civil_from_unix(seconds: i64) -> Civil {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Civil {
        year: i32::try_from(year).unwrap_or(if year < 0 { i32::MIN } else { i32::MAX }),
        month: month as u8,
        day: day as u8,
        hour: (rest / 3600) as u8,
        minute: (rest / 60 % 60) as u8,
        second: (rest % 60) as u8,
    }
}

/// The local timezone's offset from UTC, read through the C library's `localtime_r`.
#[cfg(all(unix, target_pointer_width = "64"))]
mod local {
    /// `struct tm` as glibc, musl and the BSDs lay it out on 64-bit targets.
    #[repr(C)]
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
        tm_zone: *const u8,
    }

    unsafe extern "C" {
        fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
    }

    /// The offset in seconds at `seconds`, or `None` when the C library cannot tell.
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
        // SAFETY: both pointers refer to live, properly aligned locals of the types the C
        // function expects (time_t is 64-bit here); it writes only into `tm` and returns
        // null on failure. `tm_zone` is never dereferenced.
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
    fn compiler_epoch_is_bounded_before_nanosecond_conversion() {
        assert_eq!(compiler_epoch("946684800"), Some(946_684_800));
        for text in ["", "bad", "9223372036854775807", "-9223372036854775808"] {
            assert_eq!(compiler_epoch(text), None, "{text}");
        }
        assert_eq!(compiler_epoch("-1"), Some(-1));
    }

    #[test]
    fn unix_seconds_become_civil_dates() {
        let c = civil_from_unix(946_684_800);
        assert_eq!((c.year, c.month, c.day, c.hour), (2000, 1, 1, 0));
        let c = civil_from_unix(-1);
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute, c.second),
            (1969, 12, 31, 23, 59, 59)
        );
        let c = civil_from_unix(1_709_210_096); // 2024-02-29T12:34:56Z
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute, c.second),
            (2024, 2, 29, 12, 34, 56)
        );
    }

    #[test]
    fn the_local_offset_is_within_a_day() {
        if let Some(offset) = local::offset_seconds(0) {
            assert!(offset.abs() <= 86_400);
        }
        let clock = compiler_clock();
        assert!((1..=12).contains(&clock.local.month));
    }
}
