//! Strict civil dates and RFC 3339 instants for publication boundaries.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{Date, DateTime};

/// An invalid calendar date or RFC 3339 instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseTimeError;

impl fmt::Display for ParseTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a valid YYYY-MM-DD date or RFC 3339 instant with a time zone")
    }
}

impl std::error::Error for ParseTimeError {}

/// Parses a calendar date in years 1 through 9999; refuses impossible dates.
pub fn parse_date(text: &str) -> Result<Date, ParseTimeError> {
    if text.len() != 10 || !text.is_ascii() || &text[4..5] != "-" || &text[7..8] != "-" {
        return Err(ParseTimeError);
    }
    let date = Date {
        year: number(&text[..4])?,
        month: number(&text[5..7])? as u8,
        day: number(&text[8..])? as u8,
    };
    if date.year == 0 || Date::from_days(date.days()) != date {
        return Err(ParseTimeError);
    }
    Ok(date)
}

/// Parses an RFC 3339 instant, including offsets and up to nine fractional digits.
/// Leap seconds and the unknown local offset `-00:00` are refused.
pub fn parse_timestamp(text: &str) -> Result<SystemTime, ParseTimeError> {
    if !text.is_ascii() || text.len() < 20 {
        return Err(ParseTimeError);
    }
    let date = parse_date(&text[..10])?;
    if !matches!(&text[10..11], "T" | "t") || &text[13..14] != ":" || &text[16..17] != ":" {
        return Err(ParseTimeError);
    }
    let (hour, minute, second) =
        (number(&text[11..13])?, number(&text[14..16])?, number(&text[17..19])?);
    if hour > 23 || minute > 59 || second > 59 {
        return Err(ParseTimeError);
    }
    let tail = &text[19..];
    let zone = tail.find(['Z', 'z', '+', '-']).ok_or(ParseTimeError)?;
    let fraction = &tail[..zone];
    let nanos = if fraction.is_empty() {
        0
    } else {
        let digits = fraction.strip_prefix('.').ok_or(ParseTimeError)?;
        if digits.is_empty() || digits.len() > 9 {
            return Err(ParseTimeError);
        }
        number(digits)? as u32 * 10u32.pow(9 - digits.len() as u32)
    };
    let seconds = DateTime { date, hour: hour as u8, minute: minute as u8, second: second as u8 }
        .unix()
        - offset(&tail[zone..])?;
    let instant = if seconds < 0 {
        UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs()))
    } else {
        UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))
    };
    instant
        .and_then(|t| t.checked_add(Duration::from_nanos(u64::from(nanos))))
        .ok_or(ParseTimeError)
}

fn number(text: &str) -> Result<i64, ParseTimeError> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ParseTimeError);
    }
    text.parse().map_err(|_| ParseTimeError)
}

fn offset(text: &str) -> Result<i64, ParseTimeError> {
    if matches!(text, "Z" | "z") {
        return Ok(0);
    }
    if text.len() != 6 || !matches!(&text[..1], "+" | "-") || &text[3..4] != ":" {
        return Err(ParseTimeError);
    }
    let (hours, minutes) = (number(&text[1..3])?, number(&text[4..])?);
    if hours > 23 || minutes > 59 || text == "-00:00" {
        return Err(ParseTimeError);
    }
    Ok((hours * 3600 + minutes * 60) * if text.starts_with('-') { -1 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_offsets_are_checked() {
        assert!(parse_date("2024-02-29").is_ok());
        for text in ["2026-02-29", "0000-01-01", "2026-13-01", "2026-01-00"] {
            assert!(parse_date(text).is_err(), "{text}");
        }
        assert_eq!(parse_timestamp("1970-01-01T01:00:00+01:00"), Ok(UNIX_EPOCH));
        assert_eq!(
            parse_timestamp("1969-12-31T23:59:59.5Z"),
            UNIX_EPOCH.checked_sub(Duration::from_millis(500)).ok_or(ParseTimeError)
        );
        for text in [
            "2026-01-01",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:00:00-00:00",
            "2026-01-01T00:00:00.Z",
            "2026-01-01T00:00:00Zjunk",
        ] {
            assert!(parse_timestamp(text).is_err(), "{text}");
        }
    }
}
