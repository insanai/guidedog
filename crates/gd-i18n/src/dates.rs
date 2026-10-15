//! Dates formatted as Sphinx's `format_date` does: `today_fmt` and
//! `html_last_updated_fmt` are strftime formats whose directives Sphinx maps to Babel's
//! CLDR patterns, so names follow the language ("%b" is "Sept." in German). Guidedog
//! carries the CLDR data of the languages it has catalogs for; another language uses
//! English, as Sphinx does when Babel does not know the locale. Directives Sphinx does
//! not map stay as written.

use std::fmt::Write as _;

use gd_gettext::language_candidates;

use crate::date_names::{DATE_NAMES, DateNames};

/// A moment as a wall-clock reading: the date and time where it is observed, and that
/// place's offset from UTC and zone abbreviation. `Default` is 1970-01-01 00:00:00 UTC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moment {
    /// The year, e.g. 2026.
    pub year: i64,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day of the month, 1 to 31.
    pub day: u8,
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The minute, 0 to 59.
    pub minute: u8,
    /// The second, 0 to 60.
    pub second: u8,
    /// The offset from UTC in seconds, e.g. 19800 for +05:30.
    pub offset_seconds: i32,
    /// The zone's abbreviation, e.g. "IST"; "" shows "UTC" at offset zero, else the
    /// offset.
    pub zone: String,
}

impl Default for Moment {
    fn default() -> Moment {
        Moment::from_unix_seconds(0)
    }
}

impl Moment {
    /// Returns the UTC moment at `seconds` since 1970-01-01 00:00:00 UTC.
    pub fn from_unix_seconds(seconds: i64) -> Moment {
        Moment::from_unix_seconds_at(seconds, 0, "UTC")
    }

    /// Returns the moment at `seconds` since the Unix epoch as read in a zone `offset_seconds`
    /// ahead of UTC and abbreviated `zone`.
    pub fn from_unix_seconds_at(seconds: i64, offset_seconds: i32, zone: &str) -> Moment {
        let local = seconds.saturating_add(i64::from(offset_seconds));
        let days = local.div_euclid(86_400);
        let time = local.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Moment {
            year,
            month,
            day,
            hour: (time / 3600) as u8,
            minute: (time / 60 % 60) as u8,
            second: (time % 60) as u8,
            offset_seconds,
            zone: zone.to_owned(),
        }
    }

    /// Returns the UTC moment of a date and time.
    pub fn utc(year: i64, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Moment {
        Moment { year, month, day, hour, minute, second, ..Moment::default() }
    }

    /// Returns the moment's date as days since 1970-01-01.
    fn days(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }
}

/// Formats `moment` with a strftime `format` as Sphinx's `format_date` does, with the
/// month and day names of `language` (or its base language, or English).
pub fn format_date(format: &str, moment: &Moment, language: &str) -> String {
    let mut out = String::with_capacity(format.len() + 16);
    write_date(&mut out, format, moment, date_names(language), 0);
    out
}

/// Returns the names for a language, trying it, then its base language, then English.
fn date_names(language: &str) -> &'static DateNames {
    let candidates = language_candidates(language);
    candidates
        .iter()
        .find_map(|c| DATE_NAMES.iter().find(|n| n.language == c))
        .unwrap_or(&DATE_NAMES[0])
}

/// Writes `format` with its directives replaced; `depth` bounds the nesting of %c, %x,
/// and %X, whose patterns are formats too.
fn write_date(out: &mut String, format: &str, m: &Moment, n: &DateNames, depth: u8) {
    let mut rest = format;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let mut chars = after.chars();
        let Some(first) = chars.next() else {
            out.push('%');
            return;
        };
        let mut code_len = first.len_utf8();
        if first == '-'
            && let Some(second) = chars.next()
        {
            code_len += second.len_utf8();
        }
        let code = &after[..code_len];
        if !write_directive(out, code, m, n, depth) {
            out.push('%'); // not a directive Sphinx maps: keep it as written.
            out.push_str(code);
        }
        rest = &after[code_len..];
    }
    out.push_str(rest);
}

/// Writes one directive as Sphinx's mapping to Babel shows it, and returns false for a
/// directive Sphinx leaves as written.
fn write_directive(out: &mut String, code: &str, m: &Moment, n: &DateNames, depth: u8) -> bool {
    let days = m.days();
    let weekday = monday_first(days);
    let month = usize::from(m.month.clamp(1, 12) - 1);
    // Writing to a String cannot fail.
    let _ = match code {
        "a" => write!(out, "{}", n.days_abbr[weekday as usize]),
        "A" => write!(out, "{}", n.days_wide[weekday as usize]),
        "b" => write!(out, "{}", n.months_abbr[month]),
        "B" => write!(out, "{}", n.months_wide[month]),
        "d" => write!(out, "{:02}", m.day),
        "-d" => write!(out, "{}", m.day),
        "H" => write!(out, "{:02}", m.hour),
        "-H" => write!(out, "{}", m.hour),
        "I" => write!(out, "{:02}", twelve_hour(m.hour)),
        "-I" => write!(out, "{}", twelve_hour(m.hour)),
        "j" => write!(out, "{:03}", day_of_year(days)),
        "-j" => write!(out, "{}", day_of_year(days)),
        "m" => write!(out, "{:02}", m.month),
        "-m" => write!(out, "{}", m.month),
        "M" => write!(out, "{:02}", m.minute),
        "-M" => write!(out, "{}", m.minute),
        "S" => write!(out, "{:02}", m.second),
        "-S" => write!(out, "{}", m.second),
        "p" => write!(out, "{}", n.am_pm[usize::from(m.hour >= 12)]),
        // Sphinx maps %U, %W, and %-W to Babel's W, the week of the month.
        "U" | "W" | "-W" => write!(out, "{}", week_of_month(days, n)),
        // Babel's e, the local day of the week.
        "w" => write!(out, "{}", (weekday - n.first_week_day).rem_euclid(7) + 1),
        // Sphinx maps %y to Babel's YY, the two-digit week-based year.
        "y" => write!(out, "{:02}", week_year(days, n).rem_euclid(100)),
        "Y" => write!(out, "{:04}", m.year),
        "Z" => write_zone(out, m),
        "z" => write_offset(out, m.offset_seconds),
        "%" => write!(out, "%"),
        // The medium patterns hold no %c, and %c's holds only %x and %X.
        "x" | "X" if depth <= 1 => {
            let pattern = if code == "x" { n.medium_date } else { n.medium_time };
            write_date(out, pattern, m, n, 2);
            Ok(())
        }
        "c" if depth == 0 => {
            write_date(out, n.medium_joined, m, n, 1);
            Ok(())
        }
        _ => return false,
    };
    true
}

fn twelve_hour(hour: u8) -> u8 {
    match hour % 12 {
        0 => 12,
        h => h,
    }
}

/// Writes the zone's abbreviation, "UTC" for an unnamed zero offset, or the offset.
fn write_zone(out: &mut String, m: &Moment) -> std::fmt::Result {
    if !m.zone.is_empty() {
        out.push_str(&m.zone);
        Ok(())
    } else if m.offset_seconds == 0 {
        out.push_str("UTC");
        Ok(())
    } else {
        write_offset(out, m.offset_seconds)
    }
}

/// Writes Babel's ZZZ, the offset from UTC, as +0530.
fn write_offset(out: &mut String, offset_seconds: i32) -> std::fmt::Result {
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let minutes = offset_seconds.unsigned_abs() / 60;
    write!(out, "{sign}{:02}{:02}", minutes / 60, minutes % 60)
}

/// Returns the days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: u8, day: u8) -> i64 {
    let (month, day) = (i64::from(month.clamp(1, 12)), i64::from(day));
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Returns the date `days` after 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Returns the day of the week, counted from Monday (0) as CLDR's tables are.
fn monday_first(days: i64) -> i64 {
    // 1970-01-01 was a Thursday.
    (days + 3).rem_euclid(7)
}

/// Returns the 1-based day of the year.
fn day_of_year(days: i64) -> i64 {
    let (year, _, _) = civil_from_days(days);
    days - days_from_civil(year, 1, 1) + 1
}

/// Babel's get_week_number: the week of a period holding `day_of_period`, which falls on
/// `weekday`, under the language's week rules. It may be 0.
fn week_number(day_of_period: i64, weekday: i64, n: &DateNames) -> i64 {
    let first_day = (weekday - n.first_week_day - day_of_period + 1).rem_euclid(7);
    let mut week = (day_of_period + first_day - 1) / 7;
    if 7 - first_day >= n.min_week_days {
        week += 1;
    }
    week
}

fn week_of_month(days: i64, n: &DateNames) -> i64 {
    let (_, _, day) = civil_from_days(days);
    let week = week_number(i64::from(day), monday_first(days), n);
    if week != 0 {
        return week;
    }
    // Babel then counts in the month before, from its last day.
    let before = days - i64::from(day);
    let (_, _, last) = civil_from_days(before);
    week_number(i64::from(last), monday_first(before), n)
}

fn week_of_year(days: i64, n: &DateNames) -> i64 {
    let (year, _, day) = civil_from_days(days);
    let week = week_number(day_of_year(days), monday_first(days), n);
    if week == 0 {
        let year_end = days_from_civil(year - 1, 12, 31);
        return week_number(day_of_year(year_end), monday_first(year_end), n);
    }
    if week > 52 {
        let next = monday_first(days_from_civil(year + 1, 1, 1));
        if week_number(1, next, n) == 1
            && 32 - (next - n.first_week_day).rem_euclid(7) <= i64::from(day)
        {
            return 1;
        }
    }
    week
}

/// Babel's Y: the year a date's week belongs to.
fn week_year(days: i64, n: &DateNames) -> i64 {
    let (year, month, day) = civil_from_days(days);
    if month == 1 && day < 7 && week_of_year(days, n) >= 52 {
        return year - 1;
    }
    if month == 12 && day > 25 && week_of_year(days, n) <= 2 {
        return year + 1;
    }
    year
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect(format: &str, m: &Moment, language: &str, want: &str) {
        assert_eq!(format_date(format, m, language), want, "{format:?} in {language}");
    }

    fn tuesday() -> Moment {
        Moment::utc(2026, 9, 29, 6, 5, 9)
    }

    #[test]
    fn dates_follow_the_language() {
        let m = tuesday();
        expect("%b %d, %Y", &m, "en", "Sep 29, 2026");
        expect("%b %d, %Y", &m, "de", "Sept. 29, 2026");
        expect("%A %-d %B", &m, "fr", "mardi 29 septembre");
        expect("%-d %B %Y", &m, "ru", "29 сентября 2026");
        expect("%Y年%m月%d日 (%a)", &m, "ja", "2026年09月29日 (火)");
        // A regional language uses its base language, an unknown one English.
        expect("%B", &m, "pt_BR", "setembro");
        expect("%B", &m, "tlh", "September");
    }

    #[test]
    fn dates_have_sphinxs_directives() {
        let m = tuesday();
        expect("%H:%M:%S %I %p %-H %-M %-S", &m, "en", "06:05:09 06 AM 6 5 9");
        expect("%j %-j %m %-m %%", &m, "en", "272 272 09 9 %");
        expect("%x | %X", &m, "en", "Sep 29, 2026 | 6:05:09 AM");
        expect("%c", &m, "de", "29.09.2026, 06:05:09");
        expect("%z %Z", &m, "en", "+0000 UTC");
        // Directives Sphinx does not map stay as written.
        expect("%q %e %", &m, "en", "%q %e %");
        expect("%é%-", &m, "en", "%é%-");
    }

    // A moment in a time zone shows the zone's offset and abbreviation.
    #[test]
    fn dates_show_their_time_zone() {
        let noon = days_from_civil(2026, 1, 1) * 86_400 + 12 * 3600;
        let local = Moment::from_unix_seconds_at(noon, 19_800, "IST");
        expect("%H:%M %z %Z", &local, "en", "17:30 +0530 IST");
        let west = Moment::from_unix_seconds_at(noon, -12_600, "");
        expect("%H:%M %z %Z", &west, "en", "08:30 -0330 -0330");
    }

    // Sphinx maps %U and %W to Babel's week of the month, %w to the local day of the
    // week, and %y to the week-based year, so results follow the language's week rules.
    #[test]
    fn dates_keep_babels_week_rules() {
        let m = Moment::utc(2026, 9, 29, 0, 0, 0);
        expect("%U %W %-W %w", &m, "en", "5 5 5 3");
        expect("%w", &m, "de", "2");
        let new_year = Moment::utc(2021, 1, 1, 0, 0, 0);
        expect("%y %Y", &new_year, "en", "21 2021");
        expect("%y %Y", &new_year, "de", "20 2021");
        expect("%W", &Moment::utc(2026, 11, 1, 0, 0, 0), "de", "5");
    }

    #[test]
    fn civil_days_round_trip() {
        for days in [-800_000, -1, 0, 59, 365, 11_016, 20_725, 2_932_896] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(Moment::default().zone, "UTC");
        assert_eq!(Moment::from_unix_seconds(86_399).hour, 23);
    }
}
