//! Discussion catalogs end to end: `guidedog dsc` on a catalog of older Typst records
//! (the workflow, recovery, dates and Typst builds), on new catalogs as users make them,
//! and on the reStructuredText catalog the manual describes. The commands' parts are
//! tested in gd-discussion; these tests run the built programs.

#![allow(clippy::unwrap_used)] // a test failing on unexpected input is the point.

mod dates;
mod dsc;
mod manual;
mod recovery;
mod typst;
mod workflow;

use std::path::{Path, PathBuf};
use std::process::Command;

use gd_discussion as ds;
use guidedog_tests::{Run, Scratch, data_path, read, run_command, write};

/// A catalog of older Typst records, as Guidedog's own was: `docs/gds` holds the GDS
/// process record (number 0001), the record template and an index; `docs/shared` the
/// guide the records import.
struct Legacy {
    scratch: Scratch,
    /// The catalog folder, `<scratch>/docs/gds`.
    dir: PathBuf,
}

impl Legacy {
    /// Writes the catalog and refreshes its index, as a first command would.
    fn new(name: &str) -> Legacy {
        let scratch = Scratch::new(name);
        let dir = scratch.join("docs/gds");
        write(&dir.join("records/0001-gds-process.typ"), &fixture("process.typ"));
        write(&dir.join("template/rfc-template.typ"), &fixture("template.typ"));
        scratch.write("docs/shared/guide.typ", &fixture("guide.typ"));
        write(&dir.join("registry.typ"), "#let gds-reserved-through = 0\n");
        write(&dir.join("index.typ"), "= Fixture discussions\n\nRecords under review.\n");
        let catalog = Legacy { scratch, dir };
        catalog.dsc(0, &["index"]);
        catalog
    }

    /// Runs `guidedog dsc --dir <catalog> args` and expects `status`.
    fn dsc(&self, status: i32, args: &[&str]) -> Run {
        run_command(&mut self.command(args)).expect(status)
    }

    /// The command `guidedog dsc --dir <catalog> args`, in the scratch folder, for a
    /// caller that sets its environment.
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(guidedog_tests::program("guidedog"));
        command.arg("dsc").arg("--dir").arg(&self.dir).args(args);
        command.current_dir(&self.scratch.path).env("NO_COLOR", "1");
        command
    }

    /// A path in the catalog folder.
    fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    /// The text of a catalog file.
    fn read(&self, rel: &str) -> String {
        read(&self.path(rel))
    }

    /// The catalog's record at `rel`, read as the catalog reads it.
    fn record(&self, rel: &str) -> ds::Result<ds::Record> {
        self.parse(rel, &self.read(rel))
    }

    /// `source` read as the catalog would read it at `rel`, without writing it.
    fn parse(&self, rel: &str, source: &str) -> ds::Result<ds::Record> {
        let catalog = ds::load_catalog(&self.dir, &gds())?;
        ds::read_record(&catalog, &self.path(rel), source)
    }
}

/// The text of a fixture of the older Typst catalog.
fn fixture(name: &str) -> String {
    read(&data_path(&format!("gds-legacy/{name}")))
}

/// The prefix of Guidedog's own catalog.
fn gds() -> ds::Prefix {
    ds::Prefix::new("gds").unwrap()
}

/// `source` with the metadata field `key` set to the text `value`.
fn set_field(source: &str, key: &str, value: &str) -> String {
    ds::replace_field(source, &gds(), key, ds::FieldValue::Text(value)).unwrap()
}

/// Whether `path` names a file, a folder or a link.
fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// Today's civil date at `hours` east of UTC, `YYYY-MM-DD`, from the system clock.
fn calendar_date(hours: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    let days = (now.as_secs() as i64 + hours * 3600).div_euclid(86_400);
    civil_from_days(days)
}

/// The proleptic Gregorian date of `days` since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[test]
fn civil_dates_follow_the_calendar() {
    assert_eq!(civil_from_days(0), "1970-01-01");
    assert_eq!(civil_from_days(11_016), "2000-02-29");
    assert_eq!(civil_from_days(-1), "1969-12-31");
}
