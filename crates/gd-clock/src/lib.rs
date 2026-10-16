//! The build's moment for Guidedog's crates: the time now or the one
//! `SOURCE_DATE_EPOCH` fixes, civil dates and times from Unix seconds, and the local
//! time zone's offset from UTC.
//!
//! ```
//! let moment = gd_clock::build_time(); // SOURCE_DATE_EPOCH, or now
//! let today = moment.local().date; // the local calendar date
//! assert!((1..=12).contains(&today.month));
//! assert_eq!(gd_clock::Date::from_days(11_016).to_string(), "2000-02-29");
//! ```
//!
//! Everything is plain arithmetic except the local offset, which asks the operating
//! system (see `offset.rs`, the only module with unsafe code).

#![deny(unsafe_code)]

mod civil;
mod epoch;
mod parse;
// localtime_r (Unix) and SystemTimeToTzSpecificLocalTime (Windows): FFI.
#[allow(unsafe_code)]
mod offset;

use std::time::{SystemTime, UNIX_EPOCH};

pub use civil::{Date, DateTime};
pub use epoch::{EpochError, SOURCE_DATE_EPOCH, parse_source_date_epoch, source_date_epoch};
pub use offset::local_offset;
pub use parse::{ParseTimeError, parse_date, parse_timestamp};

/// A moment and the local zone's offset at it. `Default` is 1970-01-01 00:00:00 UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Moment {
    /// Seconds since 1970-01-01 00:00:00 UTC.
    pub unix: i64,
    /// The local zone's offset from UTC in seconds (east positive): 0 where the zone is
    /// unknown, and for a moment `SOURCE_DATE_EPOCH` fixed, which is read in UTC.
    pub offset_seconds: i64,
    /// Whether `SOURCE_DATE_EPOCH` fixed the moment.
    pub is_fixed: bool,
}

impl Moment {
    /// Returns the moment's date and time in UTC.
    pub fn utc(&self) -> DateTime {
        DateTime::from_unix(self.unix)
    }

    /// Returns the moment's date and time in the local zone.
    pub fn local(&self) -> DateTime {
        DateTime::from_unix(self.unix.saturating_add(self.offset_seconds))
    }
}

/// Returns the time now from the system clock, with the local zone's offset (0 when the
/// system cannot tell). Ignores `SOURCE_DATE_EPOCH`; [`build_time`] honours it.
pub fn now() -> Moment {
    let unix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |s| -s),
    };
    Moment { unix, offset_seconds: local_offset(unix).unwrap_or(0), is_fixed: false }
}

/// Returns the moment a build stamps on its output: `SOURCE_DATE_EPOCH`, read in UTC,
/// when it is set to a whole number of seconds in the years 1 to 9999; otherwise
/// [`now`]. A refused value falls back to the clock rather than failing the build;
/// [`parse_source_date_epoch`] says why it was refused.
pub fn build_time() -> Moment {
    match source_date_epoch() {
        Some(unix) => Moment { unix, offset_seconds: 0, is_fixed: true },
        None => now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_reads_in_utc_and_in_the_local_zone() {
        let moment = Moment { unix: 1_790_000_000, offset_seconds: 12 * 3600, is_fixed: false };
        assert_eq!(moment.utc().date.to_string(), "2026-09-21");
        assert_eq!(moment.local().date.to_string(), "2026-09-22");
        assert_eq!(Moment::default().local(), DateTime::default());
    }

    #[test]
    fn now_is_after_the_epoch_and_in_a_sane_zone() {
        let moment = now();
        assert!(moment.unix > 1_700_000_000 && !moment.is_fixed);
        assert!(moment.offset_seconds.abs() <= 86_400);
        let fixed = build_time();
        assert!(fixed.is_fixed || fixed.unix >= moment.unix);
    }
}
