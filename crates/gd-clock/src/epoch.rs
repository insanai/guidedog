//! `SOURCE_DATE_EPOCH`, the reproducible-builds convention: seconds since 1970-01-01
//! 00:00:00 UTC that stand in for the time of the build.

use std::fmt;

use crate::civil::DateTime;

/// The environment variable reproducible builds set.
pub const SOURCE_DATE_EPOCH: &str = "SOURCE_DATE_EPOCH";

/// The years a `SOURCE_DATE_EPOCH` may fall in: the ones a `YYYY-MM-DD` date can hold.
const YEARS: std::ops::RangeInclusive<i64> = 1..=9999;

/// Why a `SOURCE_DATE_EPOCH` value was refused. Each kind has a code, a title, a message
/// and a hint; `Display` writes them in the project's Elm style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochError {
    /// The text is not a whole number of seconds.
    NotANumber {
        /// The text given (without surrounding white space).
        text: String,
    },
    /// The number is a moment outside the years 1 to 9999.
    OutOfRange {
        /// The seconds given.
        seconds: i64,
    },
}

impl EpochError {
    /// A short, stable code for the error kind.
    pub fn code(&self) -> &'static str {
        match self {
            EpochError::NotANumber { .. } => "clock.epoch-number",
            EpochError::OutOfRange { .. } => "clock.epoch-range",
        }
    }

    /// The title of the error, in capitals.
    pub fn title(&self) -> &'static str {
        match self {
            EpochError::NotANumber { .. } => "SOURCE_DATE_EPOCH IS NOT A NUMBER",
            EpochError::OutOfRange { .. } => "SOURCE_DATE_EPOCH IS OUT OF RANGE",
        }
    }

    /// What went wrong, in a sentence.
    pub fn message(&self) -> String {
        match self {
            EpochError::NotANumber { text } => {
                format!("SOURCE_DATE_EPOCH is `{text}`, which is not a whole number of seconds.")
            }
            EpochError::OutOfRange { seconds } => format!(
                "SOURCE_DATE_EPOCH is {seconds} seconds, a moment outside the years 1 to 9999."
            ),
        }
    }

    /// How to fix it.
    pub fn hint(&self) -> &'static str {
        "Set SOURCE_DATE_EPOCH to seconds since 1970-01-01 00:00:00 UTC, such as the \
         output of `git log -1 --format=%ct`, or unset it to use the clock."
    }
}

impl fmt::Display for EpochError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.title();
        let rule = "-".repeat(80usize.saturating_sub(title.len() + 4));
        write!(f, "-- {title} {rule}\n\n{}\n\nHint: {}", self.message(), self.hint())
    }
}

impl std::error::Error for EpochError {}

/// Parses a `SOURCE_DATE_EPOCH` value: a whole number of seconds (white space around it
/// is ignored) whose moment falls in the years 1 to 9999.
///
/// Fails with [`EpochError::NotANumber`] or [`EpochError::OutOfRange`]. The range check
/// comes before any arithmetic, so a huge value never wraps into a plausible date.
pub fn parse_source_date_epoch(text: &str) -> Result<i64, EpochError> {
    let text = text.trim();
    let seconds: i64 =
        text.parse().map_err(|_| EpochError::NotANumber { text: text.to_owned() })?;
    if YEARS.contains(&DateTime::from_unix(seconds).date.year) {
        Ok(seconds)
    } else {
        Err(EpochError::OutOfRange { seconds })
    }
}

/// Reads `SOURCE_DATE_EPOCH` from the environment. Returns `None` when it is unset or
/// refused by [`parse_source_date_epoch`]: then the clock decides, as in other
/// reproducible builds.
pub fn source_date_epoch() -> Option<i64> {
    let text = std::env::var(SOURCE_DATE_EPOCH).ok()?;
    parse_source_date_epoch(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_numbers_in_range_are_accepted() {
        assert_eq!(parse_source_date_epoch("946684800"), Ok(946_684_800));
        assert_eq!(parse_source_date_epoch(" 0 \n"), Ok(0));
        assert_eq!(parse_source_date_epoch("-1"), Ok(-1));
        assert_eq!(parse_source_date_epoch("253402300799"), Ok(253_402_300_799)); // 9999-12-31
        assert_eq!(parse_source_date_epoch("-62135596800"), Ok(-62_135_596_800)); // 0001-01-01
    }

    #[test]
    fn other_values_are_refused() {
        for text in ["", "soon", "1.5", "0x10", "99999999999999999999"] {
            let code = parse_source_date_epoch(text).map_err(|e| e.code());
            assert_eq!(code, Err("clock.epoch-number"), "{text:?}");
        }
        for text in ["253402300800", "-62135596801", "9223372036854775807", "-9223372036854775808"]
        {
            let code = parse_source_date_epoch(text).map_err(|e| e.code());
            assert_eq!(code, Err("clock.epoch-range"), "{text}");
        }
    }

    #[test]
    fn errors_read_in_the_elm_style() {
        let text = parse_source_date_epoch(" soon ").err().map(|e| e.to_string());
        let text = text.unwrap_or_default();
        assert!(text.starts_with("-- SOURCE_DATE_EPOCH IS NOT A NUMBER ---"), "{text}");
        assert!(text.contains("\n\nSOURCE_DATE_EPOCH is `soon`, which"), "{text}");
        assert!(text.contains("\n\nHint: Set SOURCE_DATE_EPOCH"), "{text}");
        assert_eq!(text.lines().next().map(str::len), Some(80));
    }
}
