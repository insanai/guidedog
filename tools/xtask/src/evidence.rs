//! The header facts of a verification record: when and where it was made, by which
//! revision of Guidedog, and with which version of the reference program.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::process;

/// The current time in UTC as `2026-09-30T04:54:32Z`.
pub(crate) fn utc_timestamp() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil_date(days);
    let (hour, minute, second) = (rest / 3600, rest % 3600 / 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// The proleptic Gregorian date `days` days after 1970-01-01 (Howard Hinnant's
/// `civil_from_days`).
fn civil_date(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// The system, its release and the machine, as `uname -srm` prints them, or
/// `unknown` where uname is missing.
pub(crate) fn host() -> String {
    let mut uname = Command::new("uname");
    uname.arg("-srm");
    first_line(&mut uname)
}

/// The revision of Guidedog the record is about: GUIDEDOG_REVISION when set (for a tree
/// that is not a git checkout), else git's HEAD, else `unknown`.
pub(crate) fn revision(root: &Path) -> String {
    if let Some(revision) = std::env::var("GUIDEDOG_REVISION").ok().filter(|r| !r.is_empty()) {
        return revision;
    }
    let mut git = Command::new("git");
    git.arg("-C").arg(root).args(["rev-parse", "HEAD"]);
    first_line(&mut git)
}

/// The first line a program prints for `--version`, on either output.
pub(crate) fn version(program: &Path) -> String {
    let mut command = Command::new(program);
    command.arg("--version");
    first_line(&mut command)
}

/// The first line a command prints, standard output first; `unknown` when it cannot
/// run or prints nothing.
fn first_line(command: &mut Command) -> String {
    let Ok(captured) = process::capture(command) else { return String::from("unknown") };
    let text = if captured.stdout.trim().is_empty() { captured.stderr } else { captured.stdout };
    let line = text.lines().next().unwrap_or_default().trim();
    if line.is_empty() { String::from("unknown") } else { line.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_count_from_the_epoch() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(20_361), (2025, 9, 30));
        assert_eq!(civil_date(11_016), (2000, 2, 29));
        assert_eq!(utc_timestamp().len(), 20);
    }
}
