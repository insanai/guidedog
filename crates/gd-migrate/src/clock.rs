//! Today's date from the system clock, in UTC, for callers that do not give one.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current year.
pub(crate) fn current_year() -> i64 {
    today().0
}

/// Today as `YYYY-MM-DD`.
pub(crate) fn today_text() -> String {
    let (year, month, day) = today();
    format!("{year:04}-{month:02}-{day:02}")
}

fn today() -> (i64, u32, u32) {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    civil_from_days(i64::try_from(seconds / 86_400).unwrap_or(0))
}

/// The proleptic Gregorian date `days` after 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, u32::try_from(month).unwrap_or(1), u32::try_from(day).unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn days_become_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_725), (2026, 9, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }
}
