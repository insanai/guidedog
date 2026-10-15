//! Record dates: kept through promotion and decisions, checked with an explanation, and
//! taken from the local calendar or a reproducible `SOURCE_DATE_EPOCH`.

use super::{Legacy, calendar_date, fixture, set_field};
use guidedog_tests::{has, run_command, write};

#[test]
fn promotion_preserves_later_recorded_dates() {
    let c = Legacy::new("gds-dates");
    let mut source = fixture("cli.typ");
    for (key, value) in [
        ("number", "XXXXX"),
        ("state", "prediscussion"),
        ("created", "2099-01-01"),
        ("updated", "2099-01-02"),
    ] {
        source = set_field(&source, key, value);
    }
    let draft = "records/XXXXX-guidedog-cli.typ";
    write(&c.path(draft), &source);
    c.dsc(0, &["index"]);
    c.dsc(0, &["promote", "guidedog-cli", "--dry-run"]);
    assert_eq!(c.read(draft), source);
    c.dsc(0, &["promote", "guidedog-cli"]);
    let promoted = "records/0002-guidedog-cli.typ";
    let record = c.record(promoted).unwrap();
    assert_eq!((record.created.as_str(), record.updated.as_str()), ("2099-01-01", "2099-01-02"));
    c.dsc(0, &["state", "2", "--to=accepted", "--reason=Reviewed"]);
    assert_eq!(c.record(promoted).unwrap().updated, "2099-01-02");
    c.dsc(0, &["check"]);
}

#[test]
fn date_errors_explain_the_actual_problem() {
    let c = Legacy::new("gds-date-errors");
    let path = "records/0001-gds-process.typ";
    let source = set_field(&fixture("process.typ"), "created", "2026-09-29");
    let source = set_field(&source, "updated", "2026-09-28");
    let problem = c.parse(path, &source).unwrap_err();
    assert_eq!(problem.code, "gds.date-order");
    has(&problem.message, "2026-09-28");
    has(&problem.message, "2026-09-29");
    let problem = c.parse(path, &set_field(&source, "created", "2026-02-30")).unwrap_err();
    assert_eq!(problem.code, "gds.date");
    has(&problem.message, "gds-created");
    has(&problem.hint, "YYYY-MM-DD");
}

#[cfg(unix)]
#[test]
fn creation_uses_local_calendar_date() {
    let c = Legacy::new("gds-zones");
    // POSIX zone names invert the sign: Etc/GMT-14 is fourteen hours east of UTC.
    for (zone, hours) in [("Etc/GMT-14", 14_i64), ("Etc/GMT+12", -12)] {
        let slug = format!("zone-{}", hours.abs());
        let before = calendar_date(hours);
        let mut command = c.command(&["new", &slug, "--author=Ada"]);
        run_command(command.env("TZ", zone).env_remove("SOURCE_DATE_EPOCH")).expect(0);
        let after = calendar_date(hours);
        let record = c.record(&format!("records/XXXXX-{slug}.typ")).unwrap();
        assert!(record.created == before || record.created == after, "{zone}: {record:?}");
        assert_eq!(record.updated, record.created);
    }
}

/// An unrepresentable environment timestamp must not wrap into a plausible old date.
#[test]
fn out_of_range_source_epoch_uses_the_clock_without_overflow() {
    let c = Legacy::new("gds-epoch");
    let before = calendar_date(0);
    let mut command = c.command(&["new", "Epoch overflow", "--author=Ada"]);
    command.env("SOURCE_DATE_EPOCH", "9223372036854775807").env("TZ", "UTC");
    run_command(&mut command).expect(0);
    let after = calendar_date(0);
    let record = c.record("records/XXXXX-epoch-overflow.typ").unwrap();
    assert!(record.created == before || record.created == after, "{record:?}");
}
