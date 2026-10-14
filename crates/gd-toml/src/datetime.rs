//! TOML date-times (RFC 3339 with TOML's local forms).

use std::fmt;

/// A calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Date {
    /// The year, 0 to 9999.
    pub year: u16,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day of the month, 1 to 31.
    pub day: u8,
}

/// A time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Time {
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The minute, 0 to 59.
    pub minute: u8,
    /// The second, 0 to 60 (a leap second).
    pub second: u8,
    /// Fractional seconds in nanoseconds; finer digits are truncated.
    pub nanosecond: u32,
}

/// A time zone offset from UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Offset {
    /// UTC, written `Z`.
    Z,
    /// Minutes east of UTC, written `+hh:mm` or `-hh:mm`.
    Minutes(i16),
}

/// A TOML date-time: offset date-time (date, time and offset), local date-time (date
/// and time), local date (date only) or local time (time only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Datetime {
    /// The date, absent for a local time.
    pub date: Option<Date>,
    /// The time, absent for a local date.
    pub time: Option<Time>,
    /// The offset, present only with both a date and a time.
    pub offset: Option<Offset>,
}

impl Datetime {
    /// Parses a whole TOML date-time such as `1979-05-27T07:32:00Z`; `None` when `text` is
    /// not exactly one valid date-time.
    pub fn parse(text: &str) -> Option<Datetime> {
        text.parse().ok()
    }
}

impl fmt::Display for Datetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(d) = self.date {
            write!(f, "{:04}-{:02}-{:02}", d.year, d.month, d.day)?;
            if self.time.is_some() {
                f.write_str("T")?;
            }
        }
        if let Some(t) = self.time {
            write!(f, "{:02}:{:02}:{:02}", t.hour, t.minute, t.second)?;
            if t.nanosecond > 0 {
                let mut frac = format!("{:09}", t.nanosecond);
                while frac.ends_with('0') {
                    frac.pop();
                }
                write!(f, ".{frac}")?;
            }
        }
        match self.offset {
            None => Ok(()),
            Some(Offset::Z) => f.write_str("Z"),
            Some(Offset::Minutes(m)) => {
                let sign = if m < 0 { '-' } else { '+' };
                let m = m.unsigned_abs();
                write!(f, "{sign}{:02}:{:02}", m / 60, m % 60)
            }
        }
    }
}

/// Reports whether `b` starts like a date (`dddd-`) or a time (`dd:`).
pub(crate) fn looks_like(b: &[u8]) -> bool {
    let digit = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    (digit(0) && digit(1) && digit(2) && digit(3) && b.get(4) == Some(&b'-'))
        || (digit(0) && digit(1) && b.get(2) == Some(&b':'))
}

fn digits(b: &[u8], at: usize, n: usize) -> Option<u32> {
    let slice = b.get(at..at + n)?;
    slice
        .iter()
        .try_fold(0u32, |acc, &c| c.is_ascii_digit().then(|| acc * 10 + u32::from(c - b'0')))
}

fn days_in(year: u16, month: u8) -> u8 {
    match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Scans a date-time at the start of `b`; returns it and the bytes used, or a message.
pub(crate) fn scan(b: &[u8]) -> Result<(Datetime, usize), &'static str> {
    let mut out = Datetime::default();
    let mut at = 0;
    if b.get(2) != Some(&b':') {
        out.date = Some(scan_date(b)?);
        at = 10;
        let sep = b.get(at).copied();
        let time_follows = b.get(at + 1).is_some_and(u8::is_ascii_digit);
        if !(matches!(sep, Some(b'T' | b't')) || sep == Some(b' ') && time_follows) {
            return Ok((out, at));
        }
        at += 1;
    }
    let (time, used) = scan_time(&b[at..])?;
    out.time = Some(time);
    at += used;
    if out.date.is_some() {
        match b.get(at) {
            Some(b'Z' | b'z') => {
                out.offset = Some(Offset::Z);
                at += 1;
            }
            Some(&sign @ (b'+' | b'-')) => {
                let (h, m) = (digits(b, at + 1, 2), digits(b, at + 4, 2));
                let (Some(h), Some(m), Some(b':')) = (h, m, b.get(at + 3)) else {
                    return Err("An offset is written +hh:mm or -hh:mm.");
                };
                if h > 23 || m > 59 {
                    return Err("The offset's hours or minutes are out of range.");
                }
                let minutes = (h * 60 + m) as i16;
                out.offset = Some(Offset::Minutes(if sign == b'-' { -minutes } else { minutes }));
                at += 6;
            }
            _ => {}
        }
    }
    Ok((out, at))
}

fn scan_date(b: &[u8]) -> Result<Date, &'static str> {
    let (Some(y), Some(m), Some(d)) = (digits(b, 0, 4), digits(b, 5, 2), digits(b, 8, 2)) else {
        return Err("A date is written YYYY-MM-DD.");
    };
    if b.get(7) != Some(&b'-') {
        return Err("A date is written YYYY-MM-DD.");
    }
    let (year, month, day) = (y as u16, m as u8, d as u8);
    if !(1..=12).contains(&month) {
        return Err("The month is out of range; use 01 to 12.");
    }
    if day == 0 || day > days_in(year, month) {
        return Err("The day does not exist in that month.");
    }
    Ok(Date { year, month, day })
}

fn scan_time(b: &[u8]) -> Result<(Time, usize), &'static str> {
    let (Some(h), Some(m), Some(s)) = (digits(b, 0, 2), digits(b, 3, 2), digits(b, 6, 2)) else {
        return Err("A time is written HH:MM:SS, with seconds.");
    };
    if b.get(2) != Some(&b':') || b.get(5) != Some(&b':') {
        return Err("A time is written HH:MM:SS, with seconds.");
    }
    if h > 23 || m > 59 || s > 60 {
        return Err("The hour, minute or second is out of range.");
    }
    let mut time = Time { hour: h as u8, minute: m as u8, second: s as u8, nanosecond: 0 };
    let mut at = 8;
    if b.get(at) == Some(&b'.') {
        at += 1;
        let start = at;
        let mut scale = 100_000_000;
        while let Some(c) = b.get(at).filter(|c| c.is_ascii_digit()) {
            time.nanosecond += u32::from(c - b'0') * scale;
            scale /= 10;
            at += 1;
        }
        if at == start {
            return Err("Fractional seconds need digits after the point.");
        }
    }
    Ok((time, at))
}

impl std::str::FromStr for Datetime {
    type Err = &'static str;

    /// Parses one whole TOML date-time; the error says what is malformed.
    fn from_str(text: &str) -> Result<Datetime, &'static str> {
        match scan(text.as_bytes())? {
            (d, n) if n == text.len() => Ok(d),
            _ => Err("Unexpected text after the date-time."),
        }
    }
}
