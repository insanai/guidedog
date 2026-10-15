//! Civil (proleptic Gregorian) dates and times from Unix seconds and back, with Howard
//! Hinnant's `days_from_civil` and `civil_from_days` algorithms. Pure arithmetic: no time
//! zones and no operating system.

use std::fmt;

/// Seconds in a day; Unix time has no leap seconds.
const DAY: i64 = 86_400;

/// Days from 0000-03-01 (the algorithms' epoch) to 1970-01-01.
const EPOCH_SHIFT: i64 = 719_468;

/// Days in a 400-year era of the Gregorian calendar.
const ERA_DAYS: i64 = 146_097;

/// A civil date. `Default` is 1970-01-01. `Display` writes `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    /// The year, e.g. 2026 (0 is 1 BC, as in ISO 8601).
    pub year: i64,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day of the month, 1 to 31.
    pub day: u8,
}

impl Default for Date {
    fn default() -> Date {
        Date { year: 1970, month: 1, day: 1 }
    }
}

impl Date {
    /// Returns the date `days` after 1970-01-01 (before it when negative).
    pub fn from_days(days: i64) -> Date {
        let z = days + EPOCH_SHIFT;
        let era = z.div_euclid(ERA_DAYS);
        let day_of_era = z.rem_euclid(ERA_DAYS);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let mp = (5 * day_of_year + 2) / 153;
        // The ranges are exact: day 1..=31 and month 1..=12 fit in a u8.
        let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Date { year, month, day }
    }

    /// Returns the days from 1970-01-01 to this date (negative before it). A month
    /// outside 1 to 12 counts as the nearest valid one; the day is not checked, so
    /// 2026-02-30 is 2026-03-02.
    pub fn days(self) -> i64 {
        let month = i64::from(self.month.clamp(1, 12));
        let year = if month <= 2 { self.year - 1 } else { self.year };
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * ERA_DAYS + day_of_era - EPOCH_SHIFT
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A civil date and time of day, without a zone. `Default` is 1970-01-01 00:00:00.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DateTime {
    /// The date.
    pub date: Date,
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The minute, 0 to 59.
    pub minute: u8,
    /// The second, 0 to 59 (Unix time has no leap seconds).
    pub second: u8,
}

impl DateTime {
    /// Returns the date and time `seconds` after 1970-01-01 00:00:00.
    pub fn from_unix(seconds: i64) -> DateTime {
        let time = seconds.rem_euclid(DAY);
        DateTime {
            date: Date::from_days(seconds.div_euclid(DAY)),
            hour: (time / 3600) as u8,
            minute: (time / 60 % 60) as u8,
            second: (time % 60) as u8,
        }
    }

    /// Returns the seconds from 1970-01-01 00:00:00 to this date and time, saturating
    /// at the ends of `i64`.
    pub fn unix(self) -> i64 {
        let time = i64::from(self.hour) * 3600 + i64::from(self.minute) * 60;
        self.date.days().saturating_mul(DAY).saturating_add(time + i64::from(self.second))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i64, month: u8, day: u8) -> Date {
        Date { year, month, day }
    }

    #[test]
    fn known_days_become_dates() {
        assert_eq!(Date::from_days(0), date(1970, 1, 1));
        assert_eq!(Date::from_days(-1), date(1969, 12, 31));
        assert_eq!(Date::from_days(11_016), date(2000, 2, 29));
        assert_eq!(Date::from_days(20_725), date(2026, 9, 29));
        assert_eq!(Date::from_days(date(1, 1, 1).days()), date(1, 1, 1));
        assert_eq!(date(1600, 3, 1).days() - date(1600, 2, 28).days(), 2);
        assert_eq!(date(1900, 3, 1).days() - date(1900, 2, 28).days(), 1);
        assert_eq!(date(2024, 3, 1).days() - date(2024, 2, 28).days(), 2);
        assert_eq!(Date::from_days(date(2026, 2, 30).days()), date(2026, 3, 2));
        assert_eq!(date(2026, 13, 1).days(), date(2026, 12, 1).days());
    }

    #[test]
    fn days_round_trip_over_eight_thousand_years() {
        let start = date(-2000, 1, 1).days();
        let end = date(6000, 12, 31).days();
        let mut previous = Date::from_days(start - 1);
        for days in start..=end {
            let d = Date::from_days(days);
            assert_eq!(d.days(), days, "{d}");
            assert!(d > previous, "{d} follows {previous}");
            assert!((1..=12).contains(&d.month) && (1..=31).contains(&d.day), "{d}");
            previous = d;
        }
    }

    #[test]
    fn leap_years_have_february_29() {
        for year in 1590..=2410 {
            let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
            let after_28 = Date::from_days(date(year, 2, 28).days() + 1);
            assert_eq!(after_28 == date(year, 2, 29), leap, "{year}");
            assert_eq!(
                date(year + 1, 1, 1).days() - date(year, 1, 1).days(),
                365 + i64::from(leap)
            );
        }
    }

    #[test]
    fn unix_seconds_become_date_times_and_back() {
        let t = DateTime::from_unix(1_709_210_096); // 2024-02-29T12:34:56Z
        assert_eq!((t.date, t.hour, t.minute, t.second), (date(2024, 2, 29), 12, 34, 56));
        let t = DateTime::from_unix(-1);
        assert_eq!((t.date, t.hour, t.minute, t.second), (date(1969, 12, 31), 23, 59, 59));
        for seconds in [-62_135_596_800, -1, 0, 86_399, 951_782_400, 253_402_300_799] {
            assert_eq!(DateTime::from_unix(seconds).unix(), seconds);
        }
        assert_eq!(DateTime::from_unix(i64::MAX).date.month, 12);
        assert_eq!(date(2026, 10, 5).to_string(), "2026-10-05");
        assert_eq!(date(1, 1, 1).to_string(), "0001-01-01");
    }
}
